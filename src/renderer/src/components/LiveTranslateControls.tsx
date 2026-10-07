import { useState } from 'react'
import { invoke } from '@tauri-apps/api/core'
import { emitTo } from '@tauri-apps/api/event'
import { LANGUAGE_KEY, LANGUAGES } from './liveTranslateLanguage'
import './ScreenPicker.css'

type ControlAction = { type: 'pause' | 'resume' | 'change-area' | 'language'; languageChoice?: string; customLanguage?: string }

export default function LiveTranslateControls() {
  const [savedLanguage] = useState(() => window.localStorage.getItem(LANGUAGE_KEY) || 'Thai')
  const [languageChoice, setLanguageChoice] = useState(() => LANGUAGES.includes(savedLanguage) ? savedLanguage : 'custom')
  const [customLanguage, setCustomLanguage] = useState(() => LANGUAGES.includes(savedLanguage) ? '' : savedLanguage)
  const [paused, setPaused] = useState(false)
  const [error, setError] = useState('')

  const send = async (action: ControlAction) => {
    try {
      await emitTo('screen-picker', 'live-translate-control', action)
      setError('')
      return true
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
      return false
    }
  }

  const chooseLanguage = (choice: string, custom = customLanguage) => {
    setLanguageChoice(choice)
    if (choice === 'custom') setCustomLanguage(custom)
    const language = choice === 'custom' ? custom.trim() : choice
    if (language) window.localStorage.setItem(LANGUAGE_KEY, language)
    void send({ type: 'language', languageChoice: choice, customLanguage: custom })
  }

  const close = async () => {
    try {
      await invoke('close_desktop_window', { label: 'screen-picker' })
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
    }
  }

  return (
    <div className="live-translate-controls">
      <div className="live-translate-toolbar">
        <div className="live-translate-brand" data-tauri-drag-region>
          <span aria-hidden="true">◈</span>
          <div><strong>Live translate</strong><small>Click through the frame</small></div>
        </div>
        <label className="live-translate-language">To
          <select value={languageChoice} onChange={(event) => chooseLanguage(event.target.value)} aria-label="Target language">
            {LANGUAGES.map((language) => <option key={language} value={language}>{language}</option>)}
            <option value="custom">Other language…</option>
          </select>
        </label>
        {languageChoice === 'custom' && <input className="live-translate-custom-language" value={customLanguage} onChange={(event) => { setCustomLanguage(event.target.value); chooseLanguage('custom', event.target.value) }} maxLength={64} placeholder="Language name" aria-label="Custom target language" />}
        <button type="button" onClick={() => { void send({ type: paused ? 'resume' : 'pause' }).then((sent) => { if (sent) setPaused(!paused) }) }}>{paused ? 'Resume' : 'Pause'}</button>
        <button type="button" onClick={() => { void send({ type: 'change-area' }) }}>Change area</button>
        <button type="button" className="live-translate-close" onClick={() => void close()} aria-label="Close live translate">×</button>
      </div>
      {error && <div className="live-translate-controls-error" role="alert">{error}</div>}
    </div>
  )
}
