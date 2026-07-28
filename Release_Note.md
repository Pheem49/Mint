# Release Notes - Mint Agent v1.10.0

We are excited to release **Mint Agent v1.10.0**! This version introduces major enhancements across CLI, Desktop, Web, Multimodal Video, Remote Messaging Bridges, Browser Automation, LaTeX Sanitization, **AI Image-to-Image & Inpainting Editing Capabilities**, and the all-new **AI Video Editor Core & Auto Shorts Generator**.

---

## 🎬 AI Video Editor & Auto Shorts Generator (Milestones 1–4 Complete)

Mint Agent now includes a full-featured **AI Video Editor Core** & **Auto Shorts Generator** available across CLI, Desktop UI, and Web UI! Every video operation is built as a first-class tool callable by both the user and the AI Agent.

### Features Added
- **Core FFmpeg Video Operations (`crates/mint-core/src/video_edit.rs`)**:
  - `video_trim`: Precise start and end timestamp clipping.
  - `video_resize`: Change aspect ratio and resolution (e.g. 1080p, 4K, 9:16 vertical).
  - `video_merge`: Concatenate multiple video clips cleanly via FFmpeg concat filter.
  - `video_extract_audio`: Extract high-fidelity audio tracks as WAV files.
  - `video_remove_silence`: Auto-detect audio quietness via `silencedetect` and trim out dead air.
  - `render_timeline`: Multi-clip non-linear JSON Timeline Engine.
- **Speech & Subtitles Engine (`crates/mint-core/src/speech.rs` & `subtitle.rs`)**:
  - Speech-to-Text (STT) transcription with OpenAI Whisper API integration, local `whisper` CLI fallback, or heuristic chunking.
  - LLM-based Subtitle Translation preserving SRT timing codes.
  - Styled Subtitle Burning with customizable ASS presets (`🔥 TikTok Bold`, `✨ Minimal White`, `📺 Standard`).
- **⚡ Make Auto Shorts (`crates/mint-core/src/auto_shorts.rs`)**:
  - Automatically analyze long video transcripts via LLM to extract top viral highlight moments.
  - Auto-crop and resize to 9:16 vertical (1080x1920) format.
  - Auto-generate and burn yellow TikTok-style subtitles onto output clips.
- **🖼️ & 🎬 Direct Image & Video Generation in Chat (Agent Mode)**:
  - Added **Generate image** and **Generate video** action options directly into the Chat plus (`+`) attachment menu dropdown.
  - Integrated `generate_image` (`DALL-E`, `Stability`, `NanoBanana`, `Replicate`) and `generate_video` (`Google Veo`) tool dispatch actions into `crates/mint-core/src/orchestration.rs`.
  - Added inline media rendering in `src/renderer/shared/utils/markdown.tsx` so when the AI Agent generates images or videos in Chat, the generated media (`![Image](...)` & `<video controls src="..."></video>`) renders directly inside the chat conversation bubble.
- **🎬 Fully Interactive Veo Studio AI Video Editor UI**:
  - Maintained header title **Veo Studio** with **AI Editor** badge across both Desktop UI (`src/renderer/src`) and Web UI (`src/renderer/src-web`).
  - Expanded video preview canvas Viewport (`.capcut-video-viewport`) to dynamically fill 100% available height and width of the center workspace stage, maximizing video editing screen area.
  - Integrated Tauri native system file picker (`dialog.open()`) into the **Browse** button so picking a video file automatically captures the full absolute system path (`/path/to/video.mp4`), preventing `ffmpeg` file-not-found errors.
  - Fully integrated dynamic CSS application theme variables (`var(--accent)`, `var(--bg-color)`, `var(--panel-bg)`, `var(--input-bg)`, `var(--border)`) with a clean neutral dark (`#09090b`) video canvas stage, eliminating unwanted blue theme tinting around player preview.
  - Replaced all text emojis with high-quality, resolution-independent SVG vector icons across all control buttons, tool options, and timeline cards.
  - Interactive range scrubber & storyboard transcript cards carousel replacing multi-track lines.
  - Real-time player canvas controls (Play, Pause, Stop, Skip -5s/+5s) with formatted timecode.
  - Full Manual Tools control panel (Trim with playhead timestamp capture, Resize, Remove Silence, Extract Audio, Subtitles STT & burning, Auto Shorts, Export).
  - Instant Output Result vs Source Video toggle preview.
- **AI Agent Tool Registration & Platform Parity**:
  - Registered all video tools (`video_trim`, `video_resize`, `video_merge`, `video_extract_audio`, `speech_transcribe`, `subtitle_burn`, `make_shorts`) into `orchestration.rs` system prompt and dispatch arms so the AI Agent can execute video editing requests automatically from chat prompts.
  - CLI: `mint video load|trim|merge|resize|extract-audio|export|transcribe|subtitle|translate-subtitle|make-shorts` subcommands.
  - Full UI parity across Desktop (`src/renderer/src/components/VeoStudioPanel.tsx`) and Web (`src/renderer/src-web/components/VeoStudioPanel.tsx`).

- **🎨 Image Studio & Chat Image Generation Parity**:
  - Fixed `generate_image` tool dispatch in `crates/mint-core/src/orchestration.rs` to automatically invoke `crate::pictures::save_chat_images`, persisting generated images directly to physical disk (`~/.config/mint/Pictures/`) and indexing them into `pictures.json`.
  - Updated `generate_image` tool system prompt format by removing hardcoded `"provider": "dalle"`, allowing the AI Agent to respect the user's default selected provider in Image Studio / Settings (`NanoBanana (Gemini)`).
  - Added smart fallback from DALL-E to `call_nanobanana` (Gemini) when OpenAI API key is missing.
  - Updated `ImageStudioPanel.tsx` (Desktop UI & Web UI) gallery filtering to seamlessly display all AI-generated images from Chat (`chat`, `image_gen`, `cli`) while filtering out non-AI chat attachment uploads.
  - Implemented `resolveMediaUrl` in `src/renderer/shared/utils/markdown.tsx` to automatically resolve `/api/pictures/` URLs to origin server (`http://localhost:3000`), ensuring 100% image card rendering parity on Web UI (port 9000).
  - Added automatic media & model feedback attribution appending (`![Generated Image]`, `✓ Image generated successfully with model...`, and `Saved to: ...`) in `orchestration.rs` to guarantee complete model feedback display in chat bubbles.
- **💻 CLI Interactive Mode Slash Command Parity**:
  - Added `/generate-image <prompt>` and `/gen-image <prompt>` slash commands to CLI interactive `/help` menu and `AUTOCOMPLETE_COMMANDS` autocompletion list.
  - Connected `/generate-image` slash command directly to `SlashResult::ForwardToAgent`, sharing 100% of the native Agent Thinking animation engine (moon phase spinner frames, wave effect, real-time elapsed timer, and `Esc to interrupt`), central image persistence, and model attribution.

---

