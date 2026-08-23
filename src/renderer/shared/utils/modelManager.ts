/**
 * shared/utils/modelManager.ts
 * Centralized Model Manager & Event Bus for Mint CLI, Desktop UI, and Web UI.
 * 
 * Acts as the single source of truth for:
 * 1. Default model resolution per provider/category
 * 2. Bi-directional model state persistence across Settings and Studio panels
 * 3. Reactive event listeners for real-time model change synchronization across UI components.
 */

let getProfileValueFn: ((key: string) => Promise<string>) | null = null
let setProfileValueFn: ((key: string, value: string) => Promise<boolean>) | null = null

async function resolveProfileApis() {
  if (!getProfileValueFn || !setProfileValueFn) {
    try {
      // '@' resolves to src/tauri (desktop) or src-web/tauri (web) per build — see vite.config*.ts
      const mod = await import('@/tauri')
      getProfileValueFn = mod.getProfileValue
      setProfileValueFn = mod.setProfileValue
    } catch (e) {
      console.warn('[ModelManager] Dynamic tauri import failed:', e)
    }
  }
}

export type ModelCategory = 'llm' | 'image' | 'video'

export const DEFAULT_CATEGORY_MODELS: Record<string, string> = {
  gemini: 'gemini-2.5-flash',
  openai: 'gpt-5.6-luna',
  anthropic: 'claude-sonnet-5',
  openrouter: 'openai/gpt-5.6-terra',
  deepseek: 'deepseek-v4-flash',
  huggingface: 'Qwen/Qwen3.6-27B',
  
  // Image Generation Providers
  nanobanana: 'gemini-3.1-flash-image',
  bfl: 'flux-pro-1.1',
  dalle: 'gpt-image-1',
  stability: 'ultra',
  ideogram: 'V_3',
  replicate: 'black-forest-labs/flux-1.1-pro',

  // Video Generation Provider
  veo: 'veo-3.1-generate-preview',
}

const MODEL_EVENT_NAME = 'mint:model-changed'

export interface ModelChangeEventDetail {
  category: ModelCategory
  provider?: string
  model: string
}

/**
 * Get the active model for a specific key/category.
 * Fallbacks to DEFAULT_CATEGORY_MODELS if no saved value exists.
 */
export async function getActiveModel(key: string): Promise<string> {
  try {
    await resolveProfileApis()
    if (getProfileValueFn) {
      const saved = await getProfileValueFn(key)
      if (saved && saved.trim() !== '') {
        return saved.trim()
      }
    }
  } catch (e) {
    console.warn(`[ModelManager] Failed to load saved model for ${key}:`, e)
  }
  return DEFAULT_CATEGORY_MODELS[key] ?? ''
}

/**
 * Set the active model for a key/category centrally.
 * Persists to profile config and broadcasts an event to all subscribers.
 */
export async function setActiveModel(key: string, model: string, category: ModelCategory = 'llm'): Promise<boolean> {
  try {
    await resolveProfileApis()
    if (setProfileValueFn) {
      await setProfileValueFn(key, model)
    }
    
    // Broadcast event across UI components
    if (typeof window !== 'undefined') {
      window.dispatchEvent(
        new CustomEvent<ModelChangeEventDetail>(MODEL_EVENT_NAME, {
          detail: { category, provider: key, model },
        })
      )
    }
    return true
  } catch (e) {
    console.error(`[ModelManager] Failed to set active model for ${key}:`, e)
    return false
  }
}

/**
 * Subscribe to model change events across the application.
 * Returns an unsubscribe function.
 */
export function subscribeModelChange(
  callback: (detail: ModelChangeEventDetail) => void
): () => void {
  if (typeof window === 'undefined') return () => {}

  const handler = (event: Event) => {
    const customEv = event as CustomEvent<ModelChangeEventDetail>
    if (customEv.detail) {
      callback(customEv.detail)
    }
  }

  window.addEventListener(MODEL_EVENT_NAME, handler)
  return () => {
    window.removeEventListener(MODEL_EVENT_NAME, handler)
  }
}
