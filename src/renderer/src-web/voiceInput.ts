// Resolves ChatPanel's voice-input hook to the web implementation (browser
// Web Speech API). Desktop's copy of this file (src/voiceInput.ts) resolves
// the same import to the native cpal-backed mic recording hook instead —
// see @/voiceInput in ChatPanel.tsx.
export { useSpeechToText as useVoiceInput, type SpeechToTextOptions as VoiceInputOptions } from '../shared/utils/speech'
