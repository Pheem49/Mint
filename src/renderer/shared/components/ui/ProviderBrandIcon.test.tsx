/// <reference path="../../env.d.ts" />
import { describe, expect, it } from 'vitest'
// @ts-ignore react-dom/server lacks types without @types/react-dom
import { renderToStaticMarkup } from 'react-dom/server'
import ProviderBrandIcon, { matchProviderKey } from './ProviderBrandIcon'

describe('matchProviderKey', () => {
  it('correctly maps various provider and model combinations', () => {
    expect(matchProviderKey('deepseek', 'deepseek-flash')).toBe('deepseek')
    expect(matchProviderKey('DeepSeek', 'deepseek-chat')).toBe('deepseek')
    expect(matchProviderKey('google', 'gemini-1.5-pro')).toBe('gemini')
    expect(matchProviderKey('gemini', 'gemini-2.0-flash')).toBe('gemini')
    expect(matchProviderKey('anthropic', 'claude-3-5-sonnet')).toBe('anthropic')
    expect(matchProviderKey('openai', 'gpt-4o')).toBe('openai')
    expect(matchProviderKey('groq', 'llama-3.3-70b')).toBe('groq')
    expect(matchProviderKey('ollama', 'qwen2.5-coder')).toBe('ollama')
    expect(matchProviderKey('openrouter', 'anthropic/claude-3.5')).toBe('anthropic') // 'claude' / 'anthropic' matched via combined
    expect(matchProviderKey('openrouter', 'mistralai/mistral-large')).toBe('openrouter') // openrouter prioritized
    expect(matchProviderKey('custom_unknown', 'unknown_model')).toBe(null)
  })
})

describe('ProviderBrandIcon component', () => {
  it('renders a brand logo for known provider (e.g. DeepSeek)', () => {
    const html = renderToStaticMarkup(<ProviderBrandIcon provider="deepseek" model="deepseek-flash" size={13} />)
    expect(html).toContain('provider-chip-logo')
    expect(html).toContain('<img')
  })

  it('renders monochrome brand icon with mask for providers like Anthropic or OpenAI', () => {
    const htmlAnthropic = renderToStaticMarkup(<ProviderBrandIcon provider="anthropic" model="claude-3-5-sonnet" size={13} />)
    expect(htmlAnthropic).toContain('provider-chip-logo')
    expect(htmlAnthropic).toContain('is-monochrome')
    expect(htmlAnthropic).toContain('--provider-brand-color')

    const htmlOpenAI = renderToStaticMarkup(<ProviderBrandIcon provider="openai" model="gpt-4o" size={13} />)
    expect(htmlOpenAI).toContain('provider-chip-logo')
    expect(htmlOpenAI).toContain('is-monochrome')
  })

  it('renders custom provider logo from customProviders list', () => {
    const customProviders = [
      { id: 'my-custom-llm', displayName: 'My LLM', logoDataUrl: 'data:image/svg+xml,<svg></svg>' }
    ]
    const html = renderToStaticMarkup(
      <ProviderBrandIcon provider="my-custom-llm" model="custom-v1" customProviders={customProviders} size={13} />
    )
    expect(html).toContain('provider-chip-logo')
    expect(html).toContain('src="data:image/svg+xml,&lt;svg&gt;&lt;/svg&gt;"')
  })

  it('renders emoji when custom logo is an emoji', () => {
    const customProviders = [
      { id: 'custom-emoji-provider', displayName: 'Emoji AI', logoDataUrl: '⚡' }
    ]
    const html = renderToStaticMarkup(
      <ProviderBrandIcon provider="custom-emoji-provider" model="model-1" customProviders={customProviders} size={13} />
    )
    expect(html).toContain('provider-chip-emoji')
    expect(html).toContain('⚡')
  })

  it('falls back to dot when provider is unknown and not in customProviders', () => {
    const html = renderToStaticMarkup(<ProviderBrandIcon provider="unknown-ai" model="x-model" size={13} />)
    expect(html).toContain('provider-chip-dot')
  })
})
