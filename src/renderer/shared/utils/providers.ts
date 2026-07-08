/**
 * shared/utils/providers.ts
 * Provider label helpers — shared by both Desktop and Web ChatPanel.
 */

/** Minimal shape needed — matches the full ChatResponse in both tauri.ts files. */
interface ProviderResponse {
  provider: string
  fallbackProvider?: string | null
}

export function badge(provider: string, model: string): string {
  return [provider, model].filter(Boolean).join(' / ')
}

export function providerLabel(provider: string): string {
  switch (provider) {
    case 'gemini':       return 'Gemini'
    case 'openai':       return 'OpenAI'
    case 'openrouter':   return 'OpenRouter'
    case 'deepseek':     return 'DeepSeek'
    case 'anthropic':    return 'Claude'
    case 'huggingface':  return 'Hugging Face'
    case 'local_openai': return 'Local OpenAI'
    case 'ollama':       return 'Ollama'
    default:             return provider || 'Primary provider'
  }
}

export function fallbackNotice(
  response: ProviderResponse | null | undefined,
): string {
  if (!response?.fallbackProvider) return ''
  return `${providerLabel(response.fallbackProvider)} unavailable, fell back to ${providerLabel(response.provider)}.`
}

