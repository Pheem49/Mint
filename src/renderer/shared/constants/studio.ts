/**
 * shared/constants/studio.ts
 * Single source of truth for Image Studio and Veo Studio options (Aspect ratios, style presets, quick prompts).
 */

export interface AspectRatioOption {
  value: string
  label: string
  icon: string
}

export const IMAGE_ASPECT_RATIOS: AspectRatioOption[] = [
  { value: '1:1', label: '1:1', icon: '■' },
  { value: '16:9', label: '16:9', icon: '▬' },
  { value: '9:16', label: '9:16', icon: '▮' },
  { value: '4:3', label: '4:3', icon: '▭' },
]

export const VIDEO_ASPECT_RATIOS: AspectRatioOption[] = [
  { value: '16:9', label: '16:9', icon: '▬' },
  { value: '9:16', label: '9:16', icon: '▮' },
  { value: '1:1', label: '1:1', icon: '■' },
]

export const IMAGE_STYLE_PRESETS = [
  'photorealistic',
  'anime',
  'oil painting',
  'watercolor',
  'cinematic',
  'digital art',
  '3D render',
  'sketch',
] as const

export const VIDEO_STYLE_PRESETS = [
  'cinematic',
  'photorealistic',
  'anime',
  '3D animation',
  'documentary',
  'animation',
  'action',
  'nature',
] as const
