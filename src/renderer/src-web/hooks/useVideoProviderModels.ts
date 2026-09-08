/**
 * src-web/hooks/useVideoProviderModels.ts
 */
import { useState, useEffect, useRef } from 'react'
import { VEO_STUDIO_MODELS } from '../../shared/constants/models'
import { fetchVideoProviderModels } from '../tauri'

export interface VideoModelOption {
  value: string
  label: string
}

function getPresets(provider: string): VideoModelOption[] {
  const lower = provider.trim().toLowerCase()
  const canonical = lower === 'gemini' || lower === 'google' ? 'veo' : lower
  const presets = (VEO_STUDIO_MODELS as Record<string, VideoModelOption[]>)[canonical]
  return presets ? [...presets] : []
}

export interface UseVideoProviderModelsResult {
  options: VideoModelOption[]
  models: string[]
  loading: boolean
}

export function useVideoProviderModels(
  provider: string,
  apiKey?: string,
): UseVideoProviderModelsResult {
  const [options, setOptions] = useState<VideoModelOption[]>(() => getPresets(provider))
  const [loading, setLoading] = useState(false)
  const reqId = useRef(0)

  useEffect(() => {
    const presets = getPresets(provider)
    setOptions(presets)

    const id = ++reqId.current
    setLoading(true)

    fetchVideoProviderModels(provider, apiKey)
      .then((live) => {
        if (id !== reqId.current) return
        if (live && live.length > 0) {
          const presetMap = new Map<string, string>()
          for (const p of presets) {
            presetMap.set(p.value, p.label)
          }

          const merged: VideoModelOption[] = []
          const seen = new Set<string>()

          for (const modelId of live) {
            if (!seen.has(modelId)) {
              seen.add(modelId)
              merged.push({
                value: modelId,
                label: presetMap.get(modelId) ?? modelId,
              })
            }
          }

          for (const p of presets) {
            if (!seen.has(p.value)) {
              seen.add(p.value)
              merged.push(p)
            }
          }

          setOptions(merged)
        }
      })
      .catch(() => { /* fallback: presets already set */ })
      .finally(() => {
        if (id === reqId.current) setLoading(false)
      })
  }, [provider, apiKey])

  return {
    options,
    models: options.map((o) => o.value),
    loading,
  }
}
