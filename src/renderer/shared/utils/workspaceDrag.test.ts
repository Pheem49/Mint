import { describe, expect, it } from 'vitest'
import {
  WORKSPACE_FILE_MIME,
  WORKSPACE_PATH_MIME,
  WORKSPACE_IMAGE_MIME,
  isImageFileName,
  formatWorkspaceReference,
  setWorkspaceDragData,
  resolveImageToDataUri,
} from './workspaceDrag'

describe('workspaceDrag utility', () => {
  it('defines the standard workspace path MIME constant', () => {
    expect(WORKSPACE_PATH_MIME).toBe('application/x-mint-workspace-path')
    expect(WORKSPACE_FILE_MIME).toBe('application/x-mint-workspace-file')
    expect(WORKSPACE_IMAGE_MIME).toBe('application/x-mint-workspace-image')
  })

  describe('isImageFileName', () => {
    it('identifies common image extensions', () => {
      expect(isImageFileName('photo.png')).toBe(true)
      expect(isImageFileName('mockup.JPG')).toBe(true)
      expect(isImageFileName('diagram.svg')).toBe(true)
      expect(isImageFileName('asset.webp')).toBe(true)
      expect(isImageFileName('anim.gif')).toBe(true)
    })

    it('rejects non-image files', () => {
      expect(isImageFileName('presentation.pdf')).toBe(false)
      expect(isImageFileName('script.py')).toBe(false)
      expect(isImageFileName('README.md')).toBe(false)
    })
  })

  describe('formatWorkspaceReference', () => {
    it('preserves existing @ prefix', () => {
      expect(formatWorkspaceReference('@src/main.rs')).toBe('@src/main.rs')
    })

    it('adds @ prefix to relative paths', () => {
      expect(formatWorkspaceReference('Harness_Presentation.pdf')).toBe('@Harness_Presentation.pdf')
      expect(formatWorkspaceReference('docs/api.md')).toBe('@docs/api.md')
    })

    it('resolves absolute path to relative reference when inside workspace root', () => {
      expect(
        formatWorkspaceReference(
          '/home/user/project/docs/guide.pdf',
          '/home/user/project'
        )
      ).toBe('@docs/guide.pdf')
    })

    it('handles Windows paths and trailing slashes in workspace root', () => {
      expect(
        formatWorkspaceReference(
          'C:\\Users\\dev\\project\\assets\\logo.png',
          'C:/Users/dev/project/'
        )
      ).toBe('@assets/logo.png')
    })

    it('returns empty string for empty input', () => {
      expect(formatWorkspaceReference('')).toBe('')
    })
  })

  describe('setWorkspaceDragData', () => {
    it('sets path MIME, text/plain, and workspace file MIME on DataTransfer', () => {
      const store: Record<string, string> = {}
      const mockDataTransfer = {
        setData: (format: string, data: string) => {
          store[format] = data
        },
        effectAllowed: 'none',
      } as unknown as DataTransfer

      setWorkspaceDragData(mockDataTransfer, {
        path: 'Harness_Presentation.pdf',
        root: '/home/user/project',
        kind: 'file',
      })

      expect(store[WORKSPACE_PATH_MIME]).toBe('@Harness_Presentation.pdf')
      expect(store['text/plain']).toBe('@Harness_Presentation.pdf')
      expect(store[WORKSPACE_FILE_MIME]).toContain('Harness_Presentation.pdf')
      expect(mockDataTransfer.effectAllowed).toBe('copy')
    })

    it('omits workspace file MIME when kind is directory', () => {
      const store: Record<string, string> = {}
      const mockDataTransfer = {
        setData: (format: string, data: string) => {
          store[format] = data
        },
        effectAllowed: 'none',
      } as unknown as DataTransfer

      setWorkspaceDragData(mockDataTransfer, {
        path: 'src',
        root: '/home/user/project',
        kind: 'directory',
      })

      expect(store[WORKSPACE_PATH_MIME]).toBe('@src')
      expect(store['text/plain']).toBe('@src')
      expect(store[WORKSPACE_FILE_MIME]).toBeUndefined()
      expect(store[WORKSPACE_IMAGE_MIME]).toBeUndefined()
    })

    it('sets WORKSPACE_IMAGE_MIME when dragged item is an image file', () => {
      const store: Record<string, string> = {}
      const mockDataTransfer = {
        setData: (format: string, data: string) => {
          store[format] = data
        },
        effectAllowed: 'none',
      } as unknown as DataTransfer

      setWorkspaceDragData(mockDataTransfer, {
        path: 'assets/hero.png',
        root: '/home/user/project',
        kind: 'file',
        url: 'data:image/png;base64,abc1234',
      })

      expect(store[WORKSPACE_IMAGE_MIME]).toBeDefined()
      const parsed = JSON.parse(store[WORKSPACE_IMAGE_MIME])
      expect(parsed.path).toBe('assets/hero.png')
      expect(parsed.url).toBe('data:image/png;base64,abc1234')
      expect(parsed.name).toBe('hero.png')
    })
  })

  describe('resolveImageToDataUri', () => {
    it('returns dataUri directly if target already starts with data:image/', async () => {
      const dataUri = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUg=='
      const result = await resolveImageToDataUri(dataUri)
      expect(result).toBe(dataUri)
    })

    it('falls back to platformReader for workspace image paths', async () => {
      const mockReader = async (path: string, _root: string) => `data:image/jpeg;base64,mock_${path}`
      const result = await resolveImageToDataUri('images/sample.jpg', '/workspace', mockReader)
      expect(result).toBe('data:image/jpeg;base64,mock_images/sample.jpg')
    })

    it('returns empty string if resolution fails', async () => {
      const result = await resolveImageToDataUri('nonexistent.jpg')
      expect(result).toBe('')
    })
  })
})
