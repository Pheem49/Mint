//! Native push-to-talk microphone capture (via `cpal`) and transcription through
//! whichever chat provider the user already has configured for `MintConfig`.
//!
//! This is deliberately *not* the same as `speech::transcribe` (which is
//! Whisper-API/CLI specific and used for transcribing existing audio/video
//! files via the `speech_transcribe` agent action). This module always routes
//! through the provider the user already picked for chat, reusing the
//! multimodal audio support `chat.rs` has for Gemini and OpenAI, so no second
//! API key is needed. Providers that don't accept audio surface a specific,
//! actionable error instead of silently falling back to a different provider.

use std::io::Cursor;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;

use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use hound::{SampleFormat, WavSpec, WavWriter};
use thiserror::Error;

use crate::MintConfig;
use crate::chat::{ChatError, ChatRequest, send_chat};

/// Defensive cap so a forgotten/stuck recording doesn't grow memory unbounded.
const MAX_RECORDING_SECS: u32 = 120;

#[derive(Debug, Error)]
pub enum MicTranscribeError {
    #[error("no default input (microphone) device found")]
    NoInputDevice,
    #[error("failed to configure microphone input stream: {0}")]
    StreamConfig(String),
    #[error("microphone recording failed: {0}")]
    RecordingFailed(String),
    #[error("no audio was recorded (recording was empty or too short)")]
    EmptyRecording,
    #[error("a recording is already in progress")]
    AlreadyRecording,
    #[error("no recording is in progress")]
    NotRecording,
    #[error(
        "provider '{0}' does not support audio input for voice transcription — switch to a provider that does (Gemini or OpenAI) in Settings, or type your message instead"
    )]
    ProviderDoesNotSupportAudio(String),
    #[error("transcription failed: {0}")]
    Chat(#[from] ChatError),
}

/// Handle to an in-progress recording, returned by [`start_recording`]. Holds only
/// `Send`-safe pieces — the `cpal::Stream` itself lives on its own OS thread for
/// the whole recording (it isn't `Send` on every backend, e.g. ALSA) and never
/// crosses this boundary.
pub struct MicRecordingHandle {
    stop_tx: mpsc::Sender<()>,
    join: JoinHandle<Result<Vec<u8>, MicTranscribeError>>,
}

/// Starts recording from the default microphone on a dedicated OS thread and
/// blocks briefly until the input stream is confirmed open (so device/permission
/// errors surface immediately rather than only on [`stop_recording`]). Call
/// [`stop_recording`] to stop and get back the recorded audio as WAV bytes.
pub fn start_recording() -> Result<MicRecordingHandle, MicTranscribeError> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::channel::<Result<(), MicTranscribeError>>();

    let join = std::thread::spawn(move || record_thread(stop_rx, ready_tx));

    match ready_rx.recv() {
        Ok(Ok(())) => Ok(MicRecordingHandle { stop_tx, join }),
        Ok(Err(e)) => {
            let _ = join.join();
            Err(e)
        }
        Err(_) => Err(MicTranscribeError::RecordingFailed(
            "recording thread exited unexpectedly before starting".into(),
        )),
    }
}

/// Signals the recorder thread to stop, waits for it to finish encoding, and
/// returns the recorded audio as WAV bytes.
pub fn stop_recording(handle: MicRecordingHandle) -> Result<Vec<u8>, MicTranscribeError> {
    let _ = handle.stop_tx.send(());
    handle
        .join
        .join()
        .map_err(|_| MicTranscribeError::RecordingFailed("recording thread panicked".into()))?
}

/// Sends recorded WAV audio to whichever provider `config.ai_provider` points
/// at and returns the transcript. Providers that don't support audio input
/// return [`MicTranscribeError::ProviderDoesNotSupportAudio`] rather than
/// silently trying a different provider.
pub async fn transcribe_recording(
    config: &MintConfig,
    wav_bytes: Vec<u8>,
) -> Result<String, MicTranscribeError> {
    if wav_bytes.is_empty() {
        return Err(MicTranscribeError::EmptyRecording);
    }

    let audio_data_uri = format!(
        "data:audio/wav;base64,{}",
        BASE64_STANDARD.encode(&wav_bytes)
    );

    let request = ChatRequest {
        message: "Transcribe the attached audio verbatim. Respond with ONLY the transcript \
                   text, in the language spoken. No commentary, no quotation marks, no \
                   \"Transcript:\" prefix, and no translation."
            .to_string(),
        system_instruction: "You are a speech-to-text transcription engine. Output only the \
                   raw transcript of the attached audio, nothing else."
            .to_string(),
        chat_id: None,
        image_data_uri: None,
        audio_data_uri: Some(audio_data_uri),
        video_data_uri: None,
        document_attachment: None,
        workspace_path: None,
        agent_id: None,
        plan_mode: false,
        pinned_mcp_server: None,
        messages: None,
        tools: None,
    };

    // Deliberately `send_chat`, not `send_chat_with_fallback` — the fallback path
    // treats `UnsupportedAttachments` as recoverable and would silently retry a
    // different provider, which defeats the point of surfacing a clear error here.
    match send_chat(config, &request).await {
        Ok(response) => Ok(response.text.trim().to_string()),
        Err(ChatError::UnsupportedAttachments(provider)) => {
            Err(MicTranscribeError::ProviderDoesNotSupportAudio(provider))
        }
        Err(other) => Err(MicTranscribeError::Chat(other)),
    }
}

fn record_thread(
    stop_rx: mpsc::Receiver<()>,
    ready_tx: mpsc::Sender<Result<(), MicTranscribeError>>,
) -> Result<Vec<u8>, MicTranscribeError> {
    let host = cpal::default_host();

    let device = match host.default_input_device() {
        Some(d) => d,
        None => {
            let _ = ready_tx.send(Err(MicTranscribeError::NoInputDevice));
            return Err(MicTranscribeError::NoInputDevice);
        }
    };

    let supported_config = match device.default_input_config() {
        Ok(c) => c,
        Err(e) => return fail_to_start(&ready_tx, e.to_string()),
    };

    let sample_format = supported_config.sample_format();
    let channels = supported_config.channels();
    let sample_rate = supported_config.sample_rate().0;
    let stream_config: cpal::StreamConfig = supported_config.into();

    // A WAV file's header self-describes its sample rate/channel count, and both
    // Gemini's inlineData and OpenAI's input_audio parts accept that — so we
    // record at the device's native default rate/format instead of forcing a
    // fixed rate (e.g. 16kHz) and resampling.
    let max_samples = (MAX_RECORDING_SECS as usize)
        .saturating_mul(sample_rate as usize)
        .saturating_mul(channels as usize);

    let buffer: Arc<Mutex<Vec<f32>>> = Arc::new(Mutex::new(Vec::new()));
    let err_flag: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let stream_buffer = buffer.clone();
    let err_fn = {
        let err_flag = err_flag.clone();
        move |err: cpal::StreamError| {
            if let Ok(mut slot) = err_flag.lock() {
                *slot = Some(err.to_string());
            }
        }
    };

    let build_result = match sample_format {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                push_samples(&stream_buffer, data.iter().copied(), max_samples);
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &stream_config,
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                push_samples(
                    &stream_buffer,
                    data.iter().map(|s| *s as f32 / i16::MAX as f32),
                    max_samples,
                );
            },
            err_fn,
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &stream_config,
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                let half = u16::MAX as f32 / 2.0;
                push_samples(
                    &stream_buffer,
                    data.iter().map(move |s| (*s as f32 - half) / half),
                    max_samples,
                );
            },
            err_fn,
            None,
        ),
        other => {
            return fail_to_start(
                &ready_tx,
                format!("unsupported input sample format: {other:?}"),
            );
        }
    };

    let stream = match build_result {
        Ok(s) => s,
        Err(e) => return fail_to_start(&ready_tx, e.to_string()),
    };

    if let Err(e) = stream.play() {
        return fail_to_start(&ready_tx, e.to_string());
    }

    let _ = ready_tx.send(Ok(()));

    // Block until stop_recording() signals us (or the handle is dropped, which
    // closes the sender and unblocks recv() the same way).
    let _ = stop_rx.recv();
    drop(stream);

    if let Some(msg) = err_flag.lock().ok().and_then(|g| g.clone()) {
        return Err(MicTranscribeError::RecordingFailed(msg));
    }

    let samples = buffer
        .lock()
        .map_err(|_| MicTranscribeError::RecordingFailed("recording buffer poisoned".into()))?
        .clone();

    if samples.is_empty() {
        return Err(MicTranscribeError::EmptyRecording);
    }

    encode_wav(&samples, channels, sample_rate)
}

fn fail_to_start(
    ready_tx: &mpsc::Sender<Result<(), MicTranscribeError>>,
    message: String,
) -> Result<Vec<u8>, MicTranscribeError> {
    let _ = ready_tx.send(Err(MicTranscribeError::StreamConfig(message.clone())));
    Err(MicTranscribeError::StreamConfig(message))
}

fn push_samples(
    buffer: &Arc<Mutex<Vec<f32>>>,
    samples: impl Iterator<Item = f32>,
    max_samples: usize,
) {
    if let Ok(mut buf) = buffer.lock() {
        if buf.len() >= max_samples {
            return;
        }
        for s in samples {
            if buf.len() >= max_samples {
                break;
            }
            buf.push(s);
        }
    }
}

fn encode_wav(
    samples: &[f32],
    channels: u16,
    sample_rate: u32,
) -> Result<Vec<u8>, MicTranscribeError> {
    let spec = WavSpec {
        channels,
        sample_rate,
        bits_per_sample: 16,
        sample_format: SampleFormat::Int,
    };

    let mut cursor = Cursor::new(Vec::new());
    {
        let mut writer = WavWriter::new(&mut cursor, spec).map_err(|e| {
            MicTranscribeError::RecordingFailed(format!("WAV encode setup failed: {e}"))
        })?;
        for &sample in samples {
            let clamped = sample.clamp(-1.0, 1.0);
            let value = (clamped * i16::MAX as f32) as i16;
            writer.write_sample(value).map_err(|e| {
                MicTranscribeError::RecordingFailed(format!("WAV encode failed: {e}"))
            })?;
        }
        writer.finalize().map_err(|e| {
            MicTranscribeError::RecordingFailed(format!("WAV finalize failed: {e}"))
        })?;
    }

    Ok(cursor.into_inner())
}
