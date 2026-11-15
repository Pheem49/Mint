import { describe, expect, it } from 'vitest'
import {
  getExtension,
  materialFileIcon,
  materialFolderIcon,
  documentIcon,
  folderIcon,
  folderOpenIcon,
  FILE_ICONS_BY_EXTENSION,
  FILE_ICONS_BY_NAME,
} from './fileIcons'

describe('fileIcons utility', () => {
  it('correctly extracts extensions case-insensitively', () => {
    expect(getExtension('main.py')).toBe('py')
    expect(getExtension('foo.BAR.TSX')).toBe('tsx')
    expect(getExtension('Dockerfile')).toBe('')
    expect(getExtension('.gitignore')).toBe('gitignore')
  })

  it('maps python files (.py, .pyw, .ipynb) to Python/Jupyter icons', () => {
    const pyIcon = materialFileIcon('script.py', 'py')
    expect(pyIcon).toBeDefined()
    expect(pyIcon).not.toBe(documentIcon)
    expect(pyIcon).toBe(FILE_ICONS_BY_EXTENSION['py'])

    const ipynbIcon = materialFileIcon('analysis.ipynb', 'ipynb')
    expect(ipynbIcon).toBe(FILE_ICONS_BY_EXTENSION['ipynb'])
  })

  it('maps major programming languages to their dedicated icons', () => {
    expect(materialFileIcon('main.rs', 'rs')).toBe(FILE_ICONS_BY_EXTENSION['rs'])
    expect(materialFileIcon('main.go', 'go')).toBe(FILE_ICONS_BY_EXTENSION['go'])
    expect(materialFileIcon('Main.java', 'java')).toBe(FILE_ICONS_BY_EXTENSION['java'])
    expect(materialFileIcon('app.c', 'c')).toBe(FILE_ICONS_BY_EXTENSION['c'])
    expect(materialFileIcon('app.cpp', 'cpp')).toBe(FILE_ICONS_BY_EXTENSION['cpp'])
    expect(materialFileIcon('App.cs', 'cs')).toBe(FILE_ICONS_BY_EXTENSION['cs'])
    expect(materialFileIcon('script.sh', 'sh')).toBe(FILE_ICONS_BY_EXTENSION['sh'])
    expect(materialFileIcon('query.sql', 'sql')).toBe(FILE_ICONS_BY_EXTENSION['sql'])
    expect(materialFileIcon('doc.pdf', 'pdf')).toBe(FILE_ICONS_BY_EXTENSION['pdf'])
  })

  it('maps special file names by exact or lowercased name', () => {
    expect(materialFileIcon('Dockerfile', '')).toBe(FILE_ICONS_BY_NAME['dockerfile'])
    expect(materialFileIcon('.gitignore', 'gitignore')).toBe(FILE_ICONS_BY_NAME['.gitignore'])
    expect(materialFileIcon('package.json', 'json')).toBe(FILE_ICONS_BY_NAME['package.json'])
  })

  it('falls back to documentIcon for unrecognized extensions', () => {
    expect(materialFileIcon('mystery.unrecognizedext123', 'unrecognizedext123')).toBe(documentIcon)
  })

  it('returns appropriate folder icons', () => {
    expect(materialFolderIcon('unknown_folder', false)).toBe(folderIcon)
    expect(materialFolderIcon('unknown_folder', true)).toBe(folderOpenIcon)
    expect(materialFolderIcon('src', false)).not.toBe(folderIcon)
    expect(materialFolderIcon('src', true)).not.toBe(folderOpenIcon)
  })
})
