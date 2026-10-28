import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import TurnFileChanges, { getCommonDirectory } from './TurnFileChanges'

describe('getCommonDirectory', () => {
  it('returns empty string for empty array', () => {
    expect(getCommonDirectory([])).toBe('')
  })

  it('returns directory for single file path', () => {
    expect(getCommonDirectory(['/home/user/project/file.ts'])).toBe('/home/user/project')
  })

  it('extracts common directory across multiple paths', () => {
    const paths = [
      '/home/pheem49/Downloads/Cosplay/probe_api.py',
      '/home/pheem49/Downloads/Cosplay/scan_bundle.py',
      '/home/pheem49/Downloads/Cosplay/fetch_api.py',
    ]
    expect(getCommonDirectory(paths)).toBe('/home/pheem49/Downloads/Cosplay')
  })

  it('extracts common root when files are in subfolders', () => {
    const paths = [
      '/workspace/src/components/App.tsx',
      '/workspace/src/utils/math.ts',
    ]
    expect(getCommonDirectory(paths)).toBe('/workspace/src')
  })
})

describe('TurnFileChanges UI refinement', () => {
  it('renders common directory in summary header and hides redundant dir in file rows', () => {
    const changes = [
      {
        path: '/home/pheem49/Downloads/Cosplay/probe_api.py',
        created: true,
        additions: 37,
        deletions: 0,
        hunks: [],
      },
      {
        path: '/home/pheem49/Downloads/Cosplay/scan_bundle.py',
        created: true,
        additions: 18,
        deletions: 0,
        hunks: [],
      },
      {
        path: '/home/pheem49/Downloads/Cosplay/fetch_api.py',
        created: true,
        additions: 18,
        deletions: 0,
        hunks: [],
      },
      {
        path: '/home/pheem49/Downloads/Cosplay/main.py',
        created: true,
        additions: 22,
        deletions: 0,
        hunks: [],
      },
    ]

    const html = renderToStaticMarkup(<TurnFileChanges changes={changes} />)

    // Summary label and common directory in header
    expect(html).toContain('4 files created')
    expect(html).toContain('file-changes-common-dir')
    expect(html).toContain('/home/pheem49/Downloads/Cosplay')

    // Filenames without repetitive full paths
    expect(html).toContain('probe_api.py')
    expect(html).toContain('scan_bundle.py')
    expect(html).toContain('fetch_api.py')

    // Rounded green chip for new files
    expect(html).toContain('file-changes-chip-new')
    expect(html).toContain('New file')
    expect(html).not.toContain('[NEW FILE]')

    // Show more button
    expect(html).toContain('Show 1 more file')
  })
})
