/**
 * src-web/hooks/useGeminiLiveModels.ts  (Web)
 *
 * Fetches the list of Gemini Live–capable models (BidiGenerateContent /
 * native-audio) via the /api/live-models endpoint, falling back to the
 * static GEMINI_LIVE_MODELS preset list on any error or when no API key
 * is set.
 */
import { useState, useEffect, useRef } from 'react'
import { GEMINI_LIVE_MODELS } from '../../shared/constants/models'
import { fetchGeminiLiveModels } from '../tauri'

const PRESETS = [...GEMINI_LIVE_MODELS] as string[]

export interface UseGeminiLiveModelsResult {
  models: string[]
  loading: boolean
}

export function useGeminiLiveModels(apiKey: string): UseGeminiLiveModelsResult {
  const [models, setModels] = useState<string[]>(PRESETS)
  const [loading, setLoading] = useState(false)
  const reqId = useRef(0)

  useEffect(() => {
    // Reset to presets immediately so the select is never empty.
    setModels(PRESETS)

    // No key → skip fetch, presets already shown.
    if (!apiKey.trim()) return

    const id = ++reqId.current
    setLoading(true)

    fetchGeminiLiveModels(apiKey)
      .then((live) => {
        if (id !== reqId.current) return
        if (live && live.length > 0) {
          // Merge: live models first, then any presets not already in the list.
          const merged = [...live]
          for (const p of PRESETS) {
            if (!merged.includes(p)) merged.push(p)
          }
          setModels(merged)
        }
      })
      .catch(() => { /* network error — presets already in state */ })
      .finally(() => { if (id === reqId.current) setLoading(false) })
  }, [apiKey])

  return { models, loading }
}
