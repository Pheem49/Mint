import { describe, expect, it } from 'vitest'
import { viewedImagesFrom, viewedImageDataUri } from './viewedImages'
import type { AgentProgress } from '../types'

describe('viewedImagesFrom', () => {
  it('extracts legacy inline data URI from ToolEnd', () => {
    const dataUri = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII='
    const progress: AgentProgress[] = [
      {
        type: 'ToolEnd',
        data: {
          action: 'view_image',
          callId: 'call-1',
          input: { path: '/workspace/photos/photo.png' },
          result: `${dataUri}\n\n[Hook] Viewed`,
          status: 'success',
        },
      },
    ]

    const items = viewedImagesFrom(progress)
    expect(items).toHaveLength(1)
    expect(items[0]).toEqual({
      id: 'call-1',
      src: dataUri,
      path: '/workspace/photos/photo.png',
      name: 'photo.png',
    })
  })

  it('extracts artifact-backed view_image seamlessly', () => {
    const progress: AgentProgress[] = [
      {
        type: 'ToolArtifacts',
        data: {
          callId: 'call-view',
          warnings: [],
          artifacts: [
            {
              id: 'art-12345',
              name: 'example-com-shot.png',
              mimeType: 'image/png',
              kind: 'image',
              size: 88239,
              chatId: 'chat-test',
              callId: 'call-view',
              server: 'view_image',
            },
          ],
        },
      },
      {
        type: 'ToolEnd',
        data: {
          action: 'view_image',
          callId: 'call-view',
          input: { path: '.mint-screenshots/example-com-shot.png' },
          result: 'Image saved: example-com-shot.png (artifactId: art-12345). Original: 1440 × 900.',
          status: 'success',
        },
      },
    ]

    const items = viewedImagesFrom(progress)
    expect(items).toHaveLength(1)
    expect(items[0]).toEqual({
      id: 'call-view',
      artifactId: 'art-12345',
      path: '.mint-screenshots/example-com-shot.png',
      name: 'example-com-shot.png',
    })
  })

  it('recovers view_image artifacts even if ToolEnd was compacted or missing', () => {
    const progress: AgentProgress[] = [
      {
        type: 'ToolArtifacts',
        data: {
          callId: 'call-compacted',
          warnings: [],
          artifacts: [
            {
              id: 'art-recovered',
              name: 'recovered.png',
              mimeType: 'image/png',
              kind: 'image',
              size: 5000,
              chatId: 'chat-test',
              callId: 'call-compacted',
              server: 'view_image',
            },
          ],
        },
      },
    ]

    const items = viewedImagesFrom(progress)
    expect(items).toHaveLength(1)
    expect(items[0].artifactId).toBe('art-recovered')
    expect(items[0].name).toBe('recovered.png')
  })

  it('ignores non-view_image artifacts so they remain in their respective tool handlers', () => {
    const progress: AgentProgress[] = [
      {
        type: 'ToolArtifacts',
        data: {
          callId: 'call-browser',
          warnings: [],
          artifacts: [
            {
              id: 'art-browser',
              name: 'browser.png',
              mimeType: 'image/png',
              kind: 'image',
              size: 5000,
              chatId: 'chat-test',
              callId: 'call-browser',
              server: 'browser',
            },
          ],
        },
      },
    ]

    const items = viewedImagesFrom(progress)
    expect(items).toHaveLength(0)
  })
})
