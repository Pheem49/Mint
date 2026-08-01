/**
 * shared/constants/models.ts
 * Canonical AI model lists — single source of truth for both Desktop and Web renderers.
 *
 * To add or update models, edit only this file.
 * Both src/components/ChatPanel.tsx and src-web/components/ChatPanel.tsx
 * import from here via their respective re-export files.
 */

export const GEMINI_MODELS = [
  'gemini-3.6-flash',
  'gemini-3.5-flash',
  'gemini-3.5-flash-lite',
  'gemini-3.1-flash-lite',
  'gemini-2.5-flash',
] as const

export const OPENAI_MODELS = [
  'gpt-5.6-sol',
  'gpt-5.6-terra',
  'gpt-5.6-luna',
  'gpt-5.5-thinking',
  'gpt-5.5-pro',
] as const

export const ANTHROPIC_MODELS = [
  'claude-sonnet-5',
  'claude-opus-5',
  'claude-sonnet-4.6',
  'claude-opus-4.8',
  'claude-haiku-4.5',
] as const

export const OPENROUTER_MODELS = [
  'openai/gpt-5.6-terra',
  'anthropic/claude-sonnet-5',
  'google/gemini-3.6-flash',
  'x-ai/grok-4.5',
  'deepseek/deepseek-v4-pro',
] as const

export const DEEPSEEK_MODELS = [
  'deepseek-v4-flash',
  'deepseek-v4-pro',
  'deepseek-chat',
  'deepseek-reasoner',
] as const

export const HF_MODELS = [
  'Qwen/Qwen3.6-27B',
  'deepseek-ai/DeepSeek-V4-Flash',
  'google/gemma-3-27b-it',
  'meta-llama/Llama-3.3-70B-Instruct',
  'microsoft/phi-4',
  'zai-org/GLM-5.2-FP8',
  'mistralai/Mistral-Large-Instruct',
  'openai/gpt-oss-120b',
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

export const IMAGE_STUDIO_MODELS = {
  nanobanana: [
    { value: 'gemini-3.1-flash-image', label: 'Gemini 3.1 Flash Image (Default)' },
    { value: 'gemini-3-pro-image',     label: 'Gemini 3 Pro Image' },
    { value: 'gemini-2.5-flash-image', label: 'Gemini 2.5 Flash Image' },
  ],
  bfl: [
    { value: 'flux-pro-1.1',       label: 'FLUX 1.1 Pro (Default)' },
    { value: 'flux-pro-1.1-ultra', label: 'FLUX 1.1 Pro Ultra' },
    { value: 'flux-pro',           label: 'FLUX Pro' },
    { value: 'flux-dev',           label: 'FLUX Dev' },
    { value: 'flux-schnell',       label: 'FLUX Schnell' },
    { value: 'flux-kontext-pro',   label: 'FLUX Kontext Pro' },
    { value: 'flux-kontext-max',   label: 'FLUX Kontext Max' },
    { value: 'flux-fill-pro',      label: 'FLUX Fill Pro' },
  ],
  dalle: [
    { value: 'gpt-image-1', label: 'GPT Image 1 (Default)' },
    { value: 'dall-e-3',    label: 'DALL·E 3' },
  ],
  stability: [
    { value: 'ultra',             label: 'Stable Image Ultra (Default)' },
    { value: 'core',              label: 'Stable Image Core' },
    { value: 'sd3.5-large',       label: 'SD3.5 Large' },
    { value: 'sd3.5-large-turbo', label: 'SD3.5 Large Turbo' },
    { value: 'sd3-medium',        label: 'SD3 Medium' },
  ],
  ideogram: [
    { value: 'V_3',       label: 'Ideogram V3 (Default)' },
    { value: 'V_2',       label: 'Ideogram V2' },
    { value: 'V_2_TURBO', label: 'Ideogram V2 Turbo' },
  ],
  replicate: [
    { value: 'black-forest-labs/flux-1.1-pro',    label: 'FLUX 1.1 Pro (Default)' },
    { value: 'black-forest-labs/flux-kontext-pro',label: 'FLUX Kontext Pro' },
    { value: 'black-forest-labs/flux-fill-pro',   label: 'FLUX Fill Pro' },
    { value: 'black-forest-labs/flux-schnell',    label: 'FLUX Schnell' },
    { value: 'stability-ai/sdxl',                 label: 'SDXL' },
    { value: 'timbrooks/instruct-pix2pix',        label: 'InstructPix2Pix' },
  ],
}

export const VEO_STUDIO_MODELS = {
  veo: [
    { value: 'veo-3.1-generate-preview',      label: 'Veo 3.1 Generate Preview (Default)' },
    { value: 'veo-3.1-fast-generate-preview', label: 'Veo 3.1 Fast Generate Preview' },
    { value: 'veo-3.1-lite-generate-preview', label: 'Veo 3.1 Lite Generate Preview' },
  ],
}
