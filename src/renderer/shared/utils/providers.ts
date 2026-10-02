/**
 * shared/utils/providers.ts
 * Provider label helpers — shared by both Desktop and Web ChatPanel.
 */

/** Minimal shape needed — matches the full ChatResponse in both tauri.ts files. */
interface ProviderResponse {
  provider: string
  fallbackProvider?: string | null
  fallbackReason?: string | null
}

export function badge(provider: string, model: string): string {
  return [provider, model].filter(Boolean).join(' / ')
}

export function providerLabel(provider: string): string {
  if (!provider) return 'Primary provider'
  const normalized = provider.startsWith('custom:') ? provider.replace(/^custom:/, '') : provider
  switch (normalized.toLowerCase()) {
    case 'gemini':       return 'Gemini'
    case 'openai':       return 'OpenAI'
    case 'openrouter':   return 'OpenRouter'
    case 'deepseek':     return 'DeepSeek'
    case 'anthropic':    return 'Claude'
    case 'huggingface':  return 'Hugging Face'
    case 'local_openai': return 'Local OpenAI'
    case 'ollama':       return 'Ollama'
    case 'groq':         return 'Groq'
    default:             return normalized.charAt(0).toUpperCase() + normalized.slice(1)
  }
}

export function fallbackNotice(
  response: ProviderResponse | null | undefined,
): string {
  if (!response?.fallbackProvider) return ''
  const reasonText = response.fallbackReason ? ` (${response.fallbackReason})` : ''
  return `${providerLabel(response.fallbackProvider)} unavailable${reasonText}, fell back to ${providerLabel(response.provider)}.`
}

