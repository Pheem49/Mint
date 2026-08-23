// Resolves ChatPanel's voice-input hook to the desktop implementation (native
// mic recording via cpal, see useNativeVoiceInput's own docs). Web's copy of
// this file (src-web/voiceInput.ts) resolves the same import to the browser
// Web Speech API implementation instead — see @/voiceInput in ChatPanel.tsx.
import { useNativeVoiceInput } from '../shared/utils/useNativeVoiceInput'
import type { SpeechToTextOptions } from '../shared/utils/speech'
import { startMicRecording, stopMicRecordingAndTranscribe } from './tauri'

export type VoiceInputOptions = SpeechToTextOptions

export function useVoiceInput(options: VoiceInputOptions) {
  return useNativeVoiceInput({
    onSendVoiceMessage: options.onSendVoiceMessage,
    startRecording: startMicRecording,
    stopRecordingAndTranscribe: stopMicRecordingAndTranscribe,
  })
}
