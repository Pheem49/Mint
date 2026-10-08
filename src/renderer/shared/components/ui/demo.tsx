"use client";

import { useState } from 'react'
import { VoicePoweredOrb } from './voice-powered-orb'
import { Button } from './button'
import { Mic, MicOff } from 'lucide-react'

/** Standalone example from the supplied component. The app uses GeminiLiveOverlay. */
export default function VoicePoweredOrbPage() {
  const [isRecording, setIsRecording] = useState(false)
  const [voiceDetected, setVoiceDetected] = useState(false)
  return (
    <div className="min-h-screen flex items-center justify-center p-8">
      <div className="flex flex-col items-center space-y-8">
        <div className="w-96 h-96 relative" style={{ width: 'min(384px, 80vw)', height: 'min(384px, 80vw)' }}>
          <VoicePoweredOrb enableVoiceControl={isRecording} className="rounded-xl overflow-hidden shadow-2xl" onVoiceDetected={setVoiceDetected} />
        </div>
        <Button onClick={() => setIsRecording(current => !current)} variant={isRecording ? 'destructive' : 'default'} size="lg" className="px-8 py-3">
          {isRecording ? <><MicOff className="w-5 h-5 mr-3" />Stop Recording</> : <><Mic className="w-5 h-5 mr-3" />Start Recording</>}
        </Button>
        <p className="text-muted-foreground text-center max-w-md">Click the button to enable voice control. Speak to see the orb respond to your voice with subtle movements.</p>
        <span role="status" className="text-muted-foreground">{voiceDetected ? 'Voice detected' : 'No voice detected'}</span>
      </div>
    </div>
  )
}
