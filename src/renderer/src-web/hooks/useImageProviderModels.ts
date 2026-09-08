/**
 * src-web/hooks/useImageProviderModels.ts
 */
import { useState, useEffect, useRef } from 'react'
import { IMAGE_STUDIO_MODELS } from '../../shared/constants/models'
import { fetchImageProviderModels } from '../tauri'

export interface ImageModelOption {
  value: string
  label: string
}

function getPresets(provider: string): ImageModelOption[] {
  const lower = provider.trim().toLowerCase()
  const canonical = lower === 'gemini' || lower === 'google' ? 'nanobanana' : lower === 'flux' ? 'bfl' : lower === 'openai' ? 'dalle' : lower
  const presets = (IMAGE_STUDIO_MODELS as Record<string, ImageModelOption[]>)[canonical]
  return presets ? [...presets] : []
}

export interface UseImageProviderModelsResult {
  options: ImageModelOption[]
  models: string[]
  loading: boolean
}

export function useImageProviderModels(
  provider: string,
  apiKey?: string,
): UseImageProviderModelsResult {
  const [options, setOptions] = useState<ImageModelOption[]>(() => getPresets(provider))
  const [loading, setLoading] = useState(false)
  const reqId = useRef(0)

  useEffect(() => {
    const presets = getPresets(provider)
    setOptions(presets)

    const id = ++reqId.current
    setLoading(true)

    fetchImageProviderModels(provider, apiKey)
      .then((live) => {
        if (id !== reqId.current) return
        if (live && live.length > 0) {
          const presetMap = new Map<string, string>()
          for (const p of presets) {
            presetMap.set(p.value, p.label)
          }

          const merged: ImageModelOption[] = []
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
