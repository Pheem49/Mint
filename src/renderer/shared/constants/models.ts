/**
 * shared/constants/models.ts
 * Canonical AI model lists — single source of truth for both Desktop and Web renderers.
 *
 * To add or update models, edit only this file.
 * Both src/components/ChatPanel.tsx and src-web/components/ChatPanel.tsx
 * import from here via their respective re-export files.
 */

export const GEMINI_MODELS = [
  'gemini-2.5-flash',
  'gemini-2.5-pro',
  'gemini-2.0-flash',
  'gemini-2.0-flash-lite',
  'gemini-1.5-flash',
  'gemini-1.5-pro',
] as const

export const OPENAI_MODELS = [
  'gpt-4.1',
  'gpt-4.1-mini',
  'gpt-4.1-nano',
  'gpt-4o',
  'gpt-4o-mini',
  'o3',
  'o4-mini',
  'gpt-4-turbo',
] as const

export const ANTHROPIC_MODELS = [
  'claude-sonnet-4-20250514',
  'claude-opus-4-20250514',
  'claude-haiku-35-20241022',
  'claude-3-7-sonnet-latest',
  'claude-3-5-sonnet-latest',
  'claude-3-5-haiku-latest',
] as const

export const OPENROUTER_MODELS = [
  'openai/gpt-4o-mini',
  'openai/gpt-4o',
  'anthropic/claude-sonnet-4',
  'anthropic/claude-haiku-3.5',
  'anthropic/claude-3.5-sonnet',
  'anthropic/claude-3.5-haiku',
  'google/gemini-2.5-flash',
  'meta-llama/llama-3.3-70b-instruct',
  'mistralai/mistral-large',
] as const

export const DEEPSEEK_MODELS = [
  'deepseek-v4-flash',
  'deepseek-v4-pro',
  'deepseek-chat',
  'deepseek-reasoner',
] as const

export const HF_MODELS = [
  'meta-llama/Llama-3.3-70B-Instruct',
  'meta-llama/Meta-Llama-3-8B-Instruct',
  'meta-llama/Llama-3.2-3B-Instruct',
  'Qwen/Qwen2.5-72B-Instruct',
  'Qwen/Qwen2.5-Coder-32B-Instruct',
  'Qwen/Qwen3-235B-A22B',
  'mistralai/Mistral-Small-24B-Instruct-2501',
  'mistralai/Mistral-7B-Instruct-v0.3',
  'google/gemma-3-27b-it',
  'google/gemma-2-9b-it',
] as const

export const LOCAL_MODELS = [
  'local-model',
  'Qwen/Qwen2.5-7B-Instruct-GGUF',
  'meta-llama/Llama-3.2-3B-Instruct-GGUF',
  'lmstudio-community/gemma-2-9b-it-GGUF',
] as const

/** Mutable list — populated at runtime via Ollama API */
export const OLLAMA_MODELS: string[] = []

/** Map from provider key → model list for easy lookup */
export const PROVIDER_MODELS = {
  gemini: GEMINI_MODELS,
  openai: OPENAI_MODELS,
  anthropic: ANTHROPIC_MODELS,
  openrouter: OPENROUTER_MODELS,
  deepseek: DEEPSEEK_MODELS,
  huggingface: HF_MODELS,
  local_openai: LOCAL_MODELS,
  ollama: OLLAMA_MODELS,
} as const
