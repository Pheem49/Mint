import { describe, expect, it } from 'vitest'
import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import McpToolResults from '../components/McpToolResults'
import { reduceLiveAgentProgress, compactAgentProgressForPersistence } from '../agentProgress'

describe('MCP progress retention', () => {
  it('keeps artifacts visible after later tools and folds progress by call', () => {
    let events: any[] = [{ type: 'ToolArtifacts', data: { callId: 'a', artifacts: [{ id: 'image' }], warnings: [] } }]
    for (let i = 0; i < 30; i++) events = reduceLiveAgentProgress(events, { type: 'Thought', data: { thought: 'working' } })
    for (let i = 0; i < 30; i++) events = reduceLiveAgentProgress(events, { type: 'ToolEnd', data: { action: 'read_file', input: {}, result: 'ok' } })
    for (let i = 1; i < 4; i++) events = reduceLiveAgentProgress(events, { type: 'ToolProgress', data: { callId: 'a', server: 's', tool: 't', update: { progress: i, total: 4, elapsedSecs: i } } } as any)
    expect(events.filter(e => e.type === 'ToolArtifacts')).toHaveLength(1)
    expect(events.filter(e => e.type === 'ToolProgress')).toHaveLength(1)
    expect((events.find(e => e.type === 'ToolProgress') as any).data.update.progress).toBe(3)
  })
  it('persists only the last progress per call without losing attachments', () => {
    const events: any[] = [1, 2].map(progress => ({ type: 'ToolProgress', data: { callId: 'a', server: 's', tool: 't', update: { progress } } }))
    events.push({ type: 'ToolArtifacts', data: { callId: 'a', artifacts: [], warnings: ['file unavailable'] } })
    const persisted = compactAgentProgressForPersistence(events)
    expect(persisted.filter(e => e.type === 'ToolProgress')).toHaveLength(1)
    expect(persisted.filter(e => e.type === 'ToolArtifacts')).toHaveLength(1)
  })
})

describe('MCP result display', () => {
  it('shows blocked legacy calls as failed instead of completed', () => {
    const html = renderToStaticMarkup(<McpToolResults progress={[{ type: 'ToolEnd', data: { callId: 'blocked', action: 'mcp_tool', input: {server: 's', tool: 'render'}, result: 'Blocked by hook: denied' } }]} />)
    expect(html).toContain('Failed')
    expect(html).not.toContain('Completed')
  })
  it('shows attachments and terminal status without an assistant summary', () => {
    const progress: any[] = [
      { type: 'ToolStart', data: { callId: 'a', action: 'mcp_tool', input: { server: 'blender', tool: 'render' } } },
      { type: 'ToolArtifacts', data: { callId: 'a', artifacts: [{ id: 'link', kind: 'link', name: 'model.obj', uri: 'https://example.test/model.obj' }], warnings: [] } },
      { type: 'ToolEnd', data: { callId: 'a', action: 'mcp_tool', input: { server: 'blender', tool: 'render' }, status: 'timed_out', result: 'unconfirmed' } },
    ]
    const html = renderToStaticMarkup(<McpToolResults progress={progress} />)
    expect(html).toContain('model.obj')
    expect(html).toContain('href="https://example.test/model.obj"')
    expect(html).toContain('remote completion is unconfirmed')
    expect(html).not.toContain('Completed')
  })
  it('does not turn custom or executable resource URIs into browser links', () => {
    const progress: any[] = [{ type: 'ToolArtifacts', data: { callId: 'a', warnings: [], artifacts: [{ id: 'x', kind: 'link', name: 'resource', uri: 'javascript:alert(1)' }] } }]
    const html = renderToStaticMarkup(<McpToolResults progress={progress} />)
    expect(html).toContain('resource')
    expect(html).not.toContain('href=')
  })
  it('shows real progress only for an active call', () => {
    const progress: any[] = [{ type: 'ToolProgress', data: { callId: 'a', server: 's', tool: 'render', update: { progress: 25, total: 100, message: 'Rendering', elapsedSecs: 3 } } }]
    expect(renderToStaticMarkup(<McpToolResults progress={progress} running />)).toContain('value="25"')
    progress.push({ type: 'ToolEnd', data: { callId: 'a', action: 'mcp_tool', input: {}, result: 'stopped', status: 'cancelled' } })
    const html = renderToStaticMarkup(<McpToolResults progress={progress} />)
    expect(html).toContain('remote work may still be running')
    expect(html).not.toContain('<progress')
  })
})

describe('Device verification display', () => {
  const event = (state: string, seq = 1): any => ({type:'DeviceState', data:{callId:'motor',server:'device',tool:'set_speed',update:{state,deviceId:'motor',operationId:'op',target:{rpm:100},actual:{rpm:state==='verified'?100:0},observationSeq:seq,elapsedSecs:2,message:'Observed device state'}}})
  it('distinguishes receipt, running, and verified completion', () => {
    expect(renderToStaticMarkup(<McpToolResults progress={[event('accepted')]} running />)).toContain('Command accepted')
    expect(renderToStaticMarkup(<McpToolResults progress={[event('running')]} running />)).toContain('Awaiting device completion')
    const html=renderToStaticMarkup(<McpToolResults progress={[event('verified',3)]} />)
    expect(html).toContain('Device completion verified')
    expect(html).toContain('rpm')
    expect(html).toContain('100')
  })
  it('retains acceptance and the latest observation in saved history', () => {
    let events: any[]=[]
    for(const state of ['accepted','running','running','verified']) events=reduceLiveAgentProgress(events,event(state))
    for(let i=0;i<40;i++) events=reduceLiveAgentProgress(events,{type:'ToolEnd',data:{action:'read_file',input:{},result:'ok'}})
    const states=compactAgentProgressForPersistence(events).filter(e=>e.type==='DeviceState')
    expect(states).toHaveLength(2)
    expect((states[0] as any).data.update.state).toBe('accepted')
    expect((states[1] as any).data.update.state).toBe('verified')
  })
  it('shows discovery without claiming execution', () => {
    const html=renderToStaticMarkup(<McpToolResults progress={[{type:'ToolDiscovery',data:{server:'device',tool:'move',message:'Reading definition'}} as any]} />)
    expect(html).toContain('Reading definition')
    expect(html).not.toContain('Completed')
  })
})

it('shows stopped waiting when interrupted during a device operation', () => {
  const progress: any[]=[{type:'DeviceState',data:{callId:'a',server:'device',tool:'move',update:{state:'running',deviceId:'motor',operationId:'op',target:{position:10},actual:{position:3},observationSeq:2,elapsedSecs:1,message:'Moving'}}}]
  const html=renderToStaticMarkup(<McpToolResults progress={progress} interrupted />)
  expect(html).toContain('Stopped waiting')
  expect(html).not.toContain('Awaiting device completion')
})
