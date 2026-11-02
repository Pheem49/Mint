import { describe, expect, it } from 'vitest'
import {
  getModelMetadata,
  hasNativeThinkingSupport,
  isNonTextModel,
} from './models'

describe('models thinking classification', () => {
  it('excludes only non-text media and embedding models from thinking', () => {
    expect(isNonTextModel('nano-banana-pro-preview')).toBe(true)
    expect(isNonTextModel('gemini-nano-banana-2.1')).toBe(true)
    expect(isNonTextModel('gemini-3.1-flash-image')).toBe(true)
    expect(isNonTextModel('dall-e-3')).toBe(true)
    expect(isNonTextModel('text-embedding-3-large')).toBe(true)
    expect(isNonTextModel('whisper-1')).toBe(true)
    expect(isNonTextModel('gemini-2.5-flash-native-audio-preview-12-2025')).toBe(true)

    expect(getModelMetadata('nano-banana-pro-preview', 'gemini').supportsThinking).toBe(false)
    expect(getModelMetadata('gemini-nano-banana-2.1', 'gemini').supportsThinking).toBe(false)
  })

  it('allows thinking on all text and chat models across providers', () => {
    expect(getModelMetadata('deepseek-flash', 'deepseek').supportsThinking).toBe(true)
    expect(getModelMetadata('deepseek-v4-pro', 'deepseek').supportsThinking).toBe(true)
    expect(getModelMetadata('deepseek-chat', 'deepseek').supportsThinking).toBe(true)
    expect(getModelMetadata('gemini-2.5-flash', 'gemini').supportsThinking).toBe(true)
    expect(getModelMetadata('gemini-1.5-flash', 'gemini').supportsThinking).toBe(true)
    expect(getModelMetadata('gpt-4o', 'openai').supportsThinking).toBe(true)
    expect(getModelMetadata('claude-haiku-4.5', 'anthropic').supportsThinking).toBe(true)
    expect(getModelMetadata('meta-llama/llama-3.3-70b-instruct', 'openrouter').supportsThinking).toBe(true)
  })

  it('distinguishes native reasoning API models from prompted CoT models', () => {
    expect(hasNativeThinkingSupport('deepseek-flash', 'deepseek')).toBe(true)
    expect(hasNativeThinkingSupport('deepseek-v4-pro', 'deepseek')).toBe(true)
    expect(hasNativeThinkingSupport('deepseek-reasoner', 'deepseek')).toBe(true)
    expect(hasNativeThinkingSupport('deepseek-chat', 'deepseek')).toBe(false)

    expect(hasNativeThinkingSupport('gemini-2.5-flash', 'gemini')).toBe(true)
    expect(hasNativeThinkingSupport('gemini-3.6-flash', 'gemini')).toBe(true)
    expect(hasNativeThinkingSupport('gemini-1.5-flash', 'gemini')).toBe(false)

    expect(hasNativeThinkingSupport('gpt-5.6-terra', 'openai')).toBe(true)
    expect(hasNativeThinkingSupport('o3-mini', 'openai')).toBe(true)
    expect(hasNativeThinkingSupport('gpt-4o', 'openai')).toBe(false)

    expect(hasNativeThinkingSupport('claude-sonnet-5', 'anthropic')).toBe(true)
    expect(hasNativeThinkingSupport('claude-haiku-4.5', 'anthropic')).toBe(true)
    expect(hasNativeThinkingSupport('claude-3-5-sonnet', 'anthropic')).toBe(false)
  })
})
