import { describe, expect, it } from 'vitest'
import { createLibraryTransport } from './transport'
describe('Library wire contract', () => {
  it('encodes filters, uses indexed IDs for content and surfaces server errors', async () => {
    const calls: {url: URL; method: string}[] = []
    const api = createLibraryTransport({ isNative: () => false, apiBase: () => 'http://localhost/api', fetch: async (input, init) => {
      const url = new URL(String(input)); calls.push({url,method:init?.method ?? 'GET'})
      if(url.pathname.endsWith('/content')) return new Response('hello', {headers:{'Content-Type':'text/markdown'}})
      if(init?.method === 'POST' || init?.method === 'DELETE') return Response.json({message:'Storage unavailable'}, {status:503})
      return Response.json({items:[],nextCursor:null})
    } })
    await api.listLibrary({kind:'file',search:'สรุป & notes',chatId:'A',trashed:true,cursor:'next/+'})
    expect(calls).toHaveLength(1)
    expect(calls[0].url.searchParams.get('search')).toBe('สรุป & notes')
    expect(calls[0].url.searchParams.get('trashed')).toBe('true')
    expect(calls[0].url.searchParams.get('cursor')).toBe('next/+')
    expect(await (await api.readLibraryFile('item 1')).text()).toBe('hello')
    expect(calls[1].url.pathname).toBe('/api/library/item%201/content')
    await expect(api.setLibraryTrashed('item 1',true)).rejects.toThrow('Storage unavailable')
    expect(calls[2].method).toBe('POST')
    expect(calls[2].url.pathname).toBe('/api/library/item%201/trash')
    await expect(api.removeLibraryItem('item 1')).rejects.toThrow('Storage unavailable')
    expect(calls[3].method).toBe('DELETE')
    expect(calls[3].url.pathname).toBe('/api/library/item%201')
  })
})
