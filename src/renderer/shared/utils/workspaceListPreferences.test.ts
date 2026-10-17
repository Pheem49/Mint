import { describe, expect, it } from 'vitest'
import { createWorkspaceListPreferences } from './workspaceListPreferences'
import { workspacePaths } from './workspaces'

function storage() {
  const data = new Map<string, string>()
  return { getItem: (key: string) => data.get(key) ?? null, setItem: (key: string, value: string) => { data.set(key, value) } }
}

describe('workspace list removal', () => {
  it('hides removed paths even when current workspace and saved sessions still reference them', () => {
    const saved = storage()
    const preferences = createWorkspaceListPreferences(saved)
    preferences.remove(' /projects/Demo/ ')
    expect(workspacePaths(['/projects/Demo', '/projects/Mint'], '/projects/Demo', ['/projects/Demo/'], preferences.getSnapshot()))
      .toEqual(['/projects/Mint'])
    const reloaded = createWorkspaceListPreferences(saved)
    expect(workspacePaths([], '/projects/Demo', ['/projects/Demo'], reloaded.getSnapshot())).toEqual([])
  })

  it('notifies every picker and allows explicitly reopening the folder to restore it', () => {
    const preferences = createWorkspaceListPreferences(storage())
    const chat: string[][] = [], code: string[][] = []
    preferences.subscribe(() => chat.push(preferences.getSnapshot()))
    const unsubscribe = preferences.subscribe(() => code.push(preferences.getSnapshot()))
    preferences.remove('C:\\Projects\\Demo\\')
    expect(chat).toEqual([['C:/Projects/Demo']])
    expect(code).toEqual([['C:/Projects/Demo']])
    unsubscribe()
    preferences.restore('C:/Projects/Demo/')
    expect(chat).toEqual([['C:/Projects/Demo'], []])
    expect(code).toHaveLength(1)
    expect(workspacePaths(['C:/Projects/Demo'], '', [], preferences.getSnapshot())).toEqual(['C:/Projects/Demo'])
  })

  it('keeps list removal usable when browser storage is blocked', () => {
    const preferences = createWorkspaceListPreferences({ getItem: () => { throw Error('blocked') }, setItem: () => { throw Error('blocked') } })
    preferences.remove('/projects/Demo')
    expect(preferences.getSnapshot()).toEqual(['/projects/Demo'])
    preferences.restore('/projects/Demo')
    expect(preferences.getSnapshot()).toEqual([])
  })

  it('retains session-only removals when storage can be read but is full', () => {
    const preferences = createWorkspaceListPreferences({ getItem: () => null, setItem: () => { throw Error('quota exceeded') } })
    preferences.remove('/projects/Demo')
    preferences.reload()
    expect(preferences.getSnapshot()).toEqual(['/projects/Demo'])
  })

  it('ignores malformed stored preferences and reloads changes from another window', () => {
    const saved = storage()
    const otherWindow = createWorkspaceListPreferences(saved)
    const preferences = createWorkspaceListPreferences(saved)
    expect(preferences.getSnapshot()).toEqual([])
    otherWindow.remove('/projects/Demo')
    preferences.reload()
    expect(preferences.getSnapshot()).toEqual(['/projects/Demo'])
    otherWindow.restore('/projects/Demo')
    preferences.reload()
    expect(preferences.getSnapshot()).toEqual([])
    expect(createWorkspaceListPreferences({ getItem: () => '{invalid', setItem: () => {} }).getSnapshot()).toEqual([])
  })
})
