/**
 * hooks/useProviderModels.ts
 */
import { useState, useEffect, useRef } from 'react'
import { PROVIDER_MODELS } from '../../shared/constants/models'
import { fetchProviderModels } from '../tauri'

function getPresets(provider: string): string[] {
  return [...((PROVIDER_MODELS as Record<string, readonly string[]>)[provider] ?? [])]
}

export interface UseProviderModelsResult {
  models: string[]
  loading: boolean
}

export function useProviderModels(
  provider: string,
  apiKey: string,
  baseUrl?: string,
): UseProviderModelsResult {
  const [models, setModels] = useState<string[]>(getPresets(provider))
  const [loading, setLoading] = useState(false)
  const reqId = useRef(0)

  useEffect(() => {
    setModels(getPresets(provider))
    if (provider === 'ollama' || provider === 'huggingface') return

    const id = ++reqId.current
    setLoading(true)

    fetchProviderModels(provider, apiKey, baseUrl)
      .then((live) => {
        if (id !== reqId.current) return
        if (live && live.length > 0) {
          const presets = getPresets(provider)
          const merged = [...live]
          for (const p of presets) {
            if (!merged.includes(p)) merged.push(p)
          }
          setModels(merged)
        }
      })
      .catch(() => { /* fallback: presets already set */ })
      .finally(() => { if (id === reqId.current) setLoading(false) })
  }, [provider, apiKey, baseUrl])

  return { models, loading }
}
