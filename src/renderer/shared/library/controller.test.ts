import { describe, expect, it } from 'vitest'
import { createLibraryController } from './controller'
import type { LibraryItem, LibraryPage, LibraryQuery } from '../types'
const item = (id: string): LibraryItem => ({ id, name: `${id}.md`, filename: `${id}.md`, kind: 'file', mimeType: 'text/markdown', size: 5, createdAt: '2026-10-15T00:00:00Z', origin: 'uploaded', source: 'chat', message: '', deletedAt: null, thumbnailStatus: 'unavailable', availability: 'ready', references: [] })
const deferred = <T,>() => { let resolve!: (value: T) => void; const promise = new Promise<T>(r => { resolve = r }); return { promise, resolve } }
describe('Library controller', () => {
  it('rejects stale search pages and keeps known files after a refresh fails', async () => {
    const late = deferred<LibraryPage>(); let fail = false
    const controller = createLibraryController({ list: async query => { if (fail) throw new Error('Offline'); return query.search === 'old' ? late.promise : { items: [item('new')], nextCursor: null } }, setTrashed: async () => {}, remove: async () => {} })
    const old = controller.setQuery({ search: 'old' })
    await controller.setQuery({ search: 'new' }); late.resolve({ items: [item('old')], nextCursor: null }); await old
    expect(controller.getSnapshot().items.map(i => i.id)).toEqual(['new'])
    fail = true; await controller.refresh()
    expect(controller.getSnapshot().items.map(i => i.id)).toEqual(['new'])
    expect(controller.getSnapshot().error).toContain('Offline')
  })
  it('paginates with the returned cursor and surfaces failed Trash actions', async () => {
    const queries: LibraryQuery[] = []; let fail = true
    const controller = createLibraryController({ list: async query => { queries.push(query); return query.cursor ? { items: [item('second')], nextCursor: null } : { items: [item('first')], nextCursor: 'next' } }, setTrashed: async () => { if (fail) throw new Error('Locked') }, remove: async () => {} })
    await controller.refresh(); await controller.loadMore()
    expect(queries[1].cursor).toBe('next')
    expect(controller.getSnapshot().items.map(i => i.id)).toEqual(['first', 'second'])
    await controller.setTrashed('first', true)
    expect(controller.getSnapshot().items).toHaveLength(2)
    expect(controller.getSnapshot().actionError).toContain('Locked')
    fail = false; await controller.setTrashed('first', true)
    expect(controller.getSnapshot().items.map(i => i.id)).toEqual(['second'])
  })
})

it('does not resurrect a trashed file when an older refresh finishes', async()=>{
 const late=deferred<LibraryPage>();let initial=true
 const controller=createLibraryController({list:async()=>initial?(initial=false,{items:[item('one')],nextCursor:null}):late.promise,setTrashed:async()=>{},remove:async()=>{}})
 await controller.refresh();const refresh=controller.refresh()
 await controller.setTrashed('one',true)
 late.resolve({items:[item('one')],nextCursor:null});await refresh
 expect(controller.getSnapshot().items).toEqual([])
 expect(controller.getSnapshot().loading).toBe(false)
})

it('removes a Trash entry only after system Trash succeeds and rejects a stale refresh',async()=>{
 let fail=true;let initial=true;const late=deferred<LibraryPage>()
 const controller=createLibraryController({list:async()=>initial?(initial=false,{items:[item('one')],nextCursor:null}):late.promise,setTrashed:async()=>{},remove:async()=>{if(fail)throw new Error('System Trash unavailable')}})
 await controller.setQuery({trashed:true});await controller.remove('one')
 expect(controller.getSnapshot().items).toHaveLength(1);expect(controller.getSnapshot().actionError).toContain('System Trash unavailable')
 const refresh=controller.refresh();fail=false;await controller.remove('one');late.resolve({items:[item('one')],nextCursor:null});await refresh
 expect(controller.getSnapshot().items).toEqual([])
})

it('refreshes every loaded page while thumbnail statuses change', async () => {
  let ready = false
  const first = Array.from({ length: 40 }, (_, i) => ({ ...item(`first-${i}`), thumbnailStatus: ready ? 'ready' as const : 'pending' as const }))
  const second = Array.from({ length: 40 }, (_, i) => item(`second-${i}`))
  const controller = createLibraryController({ list: async q => q.cursor ? { items: second, nextCursor: 'page-three' } : { items: first.map(i => ({ ...i, thumbnailStatus: ready ? 'ready' : 'pending' })), nextCursor: 'page-two' }, setTrashed: async () => {}, remove: async () => {} })
  await controller.refresh(); await controller.loadMore()
  expect(controller.getSnapshot().items).toHaveLength(80)
  ready = true; await controller.refresh()
  expect(controller.getSnapshot().items).toHaveLength(80)
  expect(controller.getSnapshot().items[0].thumbnailStatus).toBe('ready')
  expect(controller.getSnapshot().nextCursor).toBe('page-three')
})
