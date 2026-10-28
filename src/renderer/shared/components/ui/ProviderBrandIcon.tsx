import React from 'react'
import anthropicLogo from '@lobehub/icons-static-svg/icons/anthropic.svg?url'
import deepseekLogo from '@lobehub/icons-static-svg/icons/deepseek-color.svg?url'
import geminiLogo from '@lobehub/icons-static-svg/icons/gemini-color.svg?url'
import groqLogo from '@lobehub/icons-static-svg/icons/groq.svg?url'
import huggingFaceLogo from '@lobehub/icons-static-svg/icons/huggingface-color.svg?url'
import lmStudioLogo from '@lobehub/icons-static-svg/icons/lmstudio.svg?url'
import mistralLogo from '@lobehub/icons-static-svg/icons/mistral-color.svg?url'
import ollamaLogo from '@lobehub/icons-static-svg/icons/ollama.svg?url'
import openAiLogo from '@lobehub/icons-static-svg/icons/openai.svg?url'
import openRouterLogo from '@lobehub/icons-static-svg/icons/openrouter-color.svg?url'
import perplexityLogo from '@lobehub/icons-static-svg/icons/perplexity-color.svg?url'
import qwenLogo from '@lobehub/icons-static-svg/icons/qwen-color.svg?url'
import togetherLogo from '@lobehub/icons-static-svg/icons/together-color.svg?url'

export interface ProviderBrandIconProps {
  provider?: string
  model?: string
  customLogo?: string
  customProviders?: Array<{ id: string; displayName?: string; logoDataUrl?: string }>
  size?: number
  className?: string
}

interface LogoEntry {
  src: string
  color?: string
}

const BRAND_LOGOS: Record<string, LogoEntry> = {
  anthropic: { src: anthropicLogo, color: '#d97757' },
  claude: { src: anthropicLogo, color: '#d97757' },
  deepseek: { src: deepseekLogo },
  gemini: { src: geminiLogo },
  google: { src: geminiLogo },
  groq: { src: groqLogo, color: '#f97316' },
  huggingface: { src: huggingFaceLogo },
  hf: { src: huggingFaceLogo },
  local_openai: { src: lmStudioLogo, color: '#8b5cf6' },
  lmstudio: { src: lmStudioLogo, color: '#8b5cf6' },
  mistral: { src: mistralLogo },
  ollama: { src: ollamaLogo, color: '#e2e8f0' },
  openai: { src: openAiLogo, color: '#10a37f' },
  gpt: { src: openAiLogo, color: '#10a37f' },
  openrouter: { src: openRouterLogo },
  perplexity: { src: perplexityLogo },
  qwen: { src: qwenLogo },
  together: { src: togetherLogo },
}

export function matchProviderKey(provider = '', model = ''): string | null {
  const p = provider.toLowerCase().trim()
  const m = model.toLowerCase().trim()
  const combined = `${p} ${m}`

  if (combined.includes('deepseek')) return 'deepseek'
  if (combined.includes('gemini') || combined.includes('google')) return 'gemini'
  if (combined.includes('claude') || combined.includes('anthropic')) return 'anthropic'
  if (combined.includes('openai') || combined.includes('gpt')) return 'openai'
  if (combined.includes('groq')) return 'groq'
  if (combined.includes('openrouter')) return 'openrouter'
  if (combined.includes('ollama')) return 'ollama'
  if (combined.includes('mistral')) return 'mistral'
  if (combined.includes('perplexity')) return 'perplexity'
  if (combined.includes('qwen')) return 'qwen'
  if (combined.includes('together')) return 'together'
  if (combined.includes('lmstudio') || combined.includes('local_openai')) return 'local_openai'
  if (combined.includes('huggingface') || combined.includes('hf')) return 'huggingface'

  return null
}

export default function ProviderBrandIcon({
  provider = '',
  model = '',
  customLogo,
  customProviders,
  size = 13,
  className = '',
}: ProviderBrandIconProps) {
  // 1. Direct custom logo prop
  let effectiveLogo = customLogo

  // 2. Check in customProviders list
  if (!effectiveLogo && customProviders && provider) {
    const rawId = provider.toLowerCase().replace(/^custom:/, '')
    const matchedCp = customProviders.find(
      (cp) => cp.id.toLowerCase() === rawId || (cp.displayName && cp.displayName.toLowerCase() === rawId)
    )
    if (matchedCp?.logoDataUrl) {
      effectiveLogo = matchedCp.logoDataUrl
    }
  }

  if (effectiveLogo) {
    const isImage =
      effectiveLogo.startsWith('data:') ||
      effectiveLogo.startsWith('http://') ||
      effectiveLogo.startsWith('https://') ||
      effectiveLogo.startsWith('/')

    if (!isImage) {
      return (
        <span
          className={`provider-chip-logo provider-chip-emoji ${className}`}
          style={{
            width: `${size}px`,
            height: `${size}px`,
            display: 'inline-flex',
            alignItems: 'center',
            justifyContent: 'center',
            fontSize: `${Math.max(10, size - 1)}px`,
            lineHeight: 1,
          }}
          aria-hidden="true"
        >
          {effectiveLogo}
        </span>
      )
    }

    return (
      <span
        className={`provider-chip-logo ${className}`}
        style={{ width: `${size}px`, height: `${size}px` }}
        aria-hidden="true"
      >
        <img src={effectiveLogo} alt="" />
      </span>
    )
  }

  // 3. Match known brand logo
  const brandKey = matchProviderKey(provider, model)
  const brand = brandKey ? BRAND_LOGOS[brandKey] : null

  if (brand) {
    if (brand.color) {
      return (
        <span
          className={`provider-chip-logo is-monochrome ${className}`}
          style={{
            width: `${size}px`,
            height: `${size}px`,
            '--provider-brand-color': brand.color,
            maskImage: `url("${brand.src}")`,
            WebkitMaskImage: `url("${brand.src}")`,
          } as React.CSSProperties}
          aria-hidden="true"
        />
      )
    }

    return (
      <span
        className={`provider-chip-logo ${className}`}
        style={{ width: `${size}px`, height: `${size}px` }}
        aria-hidden="true"
      >
        <img src={brand.src} alt="" />
      </span>
    )
  }

  // 4. Fallback dot if unrecognized
  return <span className={`provider-chip-dot ${className}`} aria-hidden="true" />
}
