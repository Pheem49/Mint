import { describe, expect, it } from 'vitest'
import { describeTool, activitiesFrom } from './agentActivity'
import { viewedImagesFrom } from './viewedImages'
import { reduceLiveAgentProgress } from '../agentProgress'
import type { AgentProgress } from '../types'

describe('skill reading activity', () => {
  const input = { path: '/repo/.agents/skills/review/SKILL.md', skill_name: 'review' }
  it('shows a reading label while the tool runs', () => {
    expect(describeTool('read_file', input).label).toBe('Reading skill: review')
  })
  it('shows loaded only after a complete successful skill read', () => {
    const view = activitiesFrom([
      { type: 'ToolStart', data: { action: 'read_file', input } },
      { type: 'ToolEnd', data: { action: 'read_file', input, result: '[Loaded skill: review]\nInstructions' } },
    ])
    expect(view.items[0].label).toBe('Loaded skill: review')
  })
  it('does not claim a failed skill read was loaded', () => {
    const view = activitiesFrom([
      { type: 'ToolStart', data: { action: 'read_file', input } },
      { type: 'ToolEnd', data: { action: 'read_file', input, result: 'Error: denied' } },
    ])
    expect(view.items[0].state).toBe('error')
    expect(view.items[0].label).not.toContain('Loaded skill:')
  })
  it('matches parallel completions to the skill that finished', () => {
    const second = { path: '/repo/skills/test/SKILL.md', skill_name: 'test' }
    const view = activitiesFrom([
      { type: 'ToolStart', data: { action: 'read_file', input } },
      { type: 'ToolStart', data: { action: 'read_file', input: second } },
      { type: 'ToolEnd', data: { action: 'read_file', input, result: '[Loaded skill: review]\nInstructions' } },
    ])
    expect(view.items[0].label).toBe('Loaded skill: review')
    expect(view.items[1].label).toBe('Reading skill: test')
    expect(view.timeline?.filter(item => item.kind === 'activity')).toHaveLength(2)
  })
  it('does not mark a line-range preview as a full skill load', () => {
    const view = activitiesFrom([
      { type: 'ToolStart', data: { action: 'read_file', input } },
      { type: 'ToolEnd', data: { action: 'read_file', input, result: '1 | Header only' } },
    ])
    expect(view.items[0].label).toBe('Read skill preview: review')
  })
})

describe('MCP tool outcomes', () => {
  it('uses the explicit failure status even when diagnostic text has no error prefix', () => {
    const input = { server: 'blender', tool: 'render' }
    const view = activitiesFrom([
      { type: 'ToolStart', data: { callId: 'render-1', action: 'mcp_tool', input } },
      { type: 'ToolEnd', data: { callId: 'render-1', action: 'mcp_tool', input, result: 'render failed', status: 'failed' } },
    ] as any)
    expect(view.items[0].state).toBe('error')
  })
})

it('shows a readable view_image result instead of raw image bytes while preserving hook notes', () => {
  const input = { path: '/workspace/screen.png' }
  const view = activitiesFrom([
    { type: 'ToolStart', data: { action: 'view_image', input } },
    { type: 'ToolEnd', data: { action: 'view_image', input, result: 'data:image/png;base64,aGVsbG8=\n\n[Hook] Checked' } },
  ])
  expect(view.items[0].result).not.toContain('data:image/')
  expect(view.items[0].result).toContain('Image displayed below')
  expect(view.items[0].result).toContain('[Hook] Checked')
})

it('attaches viewedImage metadata to successful view_image activities but not failed ones', () => {
  const failInput = { path: '/tmp/failed.png' }
  const successInput = { path: '.mint-screenshots/captured.png' }
  const dataUri = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mNk+A8AAQUBAScY42YAAAAASUVORK5CYII='

  const view = activitiesFrom([
    { type: 'ToolStart', data: { callId: 'call-fail', action: 'view_image', input: failInput } },
    { type: 'ToolEnd', data: { callId: 'call-fail', action: 'view_image', input: failInput, result: 'Error: file not found', status: 'error' } },
    {
      type: 'ToolArtifacts',
      data: {
        callId: 'call-success',
        warnings: [],
        artifacts: [
          {
            id: 'art-captured',
            name: 'captured.png',
            mimeType: 'image/png',
            kind: 'image',
            size: 1024,
            chatId: 'chat-1',
            callId: 'call-success',
            server: 'view_image',
          },
        ],
      },
    },
    { type: 'ToolStart', data: { callId: 'call-success', action: 'view_image', input: successInput } },
    { type: 'ToolEnd', data: { callId: 'call-success', action: 'view_image', input: successInput, result: 'Image saved: captured.png (artifactId: art-captured)', status: 'success' } },
  ])

  // Failed activity should not have a viewed image
  expect(view.items[0].viewedImage).toBeUndefined()

  // Successful activity should have the artifact-backed viewed image
  expect(view.items[1].viewedImage).toBeDefined()
  expect(view.items[1].viewedImage?.artifactId).toBe('art-captured')
  expect(view.items[1].viewedImage?.path).toBe('.mint-screenshots/captured.png')
  expect(view.items[1].viewedImage?.name).toBe('captured.png')
})

describe('subagent event isolation', () => {
  it('excludes subagent ToolStart, ToolEnd, and ContextCompaction from the main activity view', () => {
    const view = activitiesFrom([
      { type: 'ToolStart', data: { action: 'dispatch_agents', input: { tasks: [] } } },
      { type: 'ToolStart', data: { action: 'read_file', input: { path: '/sub/file.rs' }, subagent: 'recon' } },
      { type: 'ToolEnd', data: { action: 'read_file', input: { path: '/sub/file.rs' }, result: 'content', subagent: 'recon' } },
      { type: 'ContextCompaction', data: { status: 'completed', message: 'Subagent context compacted', subagent: 'recon' } },
      { type: 'ToolEnd', data: { action: 'dispatch_agents', input: { tasks: [] }, result: 'all done' } },
    ])
    // Only the parent dispatch_agents should be in the main activity view
    expect(view.items).toHaveLength(1)
    expect(view.items[0].action).toBe('dispatch_agents')
    expect(view.timeline?.filter(t => t.kind === 'activity')).toHaveLength(1)
    expect(view.timeline?.find(t => t.kind === 'thought' && t.thought.includes('Subagent context compacted'))).toBeUndefined()
  })
})

describe('fault-tolerant run_shell command extraction', () => {
  it('extracts command from alias script or code', () => {
    expect(describeTool('run_shell', { script: 'curl -s https://example.com' }).target).toBe('curl -s https://example.com')
    expect(describeTool('run_shell', { code: 'pytest tests/' }).target).toBe('pytest tests/')
  })

  it('unpacks command from stringified arguments', () => {
    expect(describeTool('run_shell', { arguments: '{"command": "git status"}' }).target).toBe('git status')
  })

  it('unpacks command from object arguments', () => {
    expect(describeTool('run_shell', { arguments: { cmd: 'cargo check' } }).target).toBe('cargo check')
  })

  it('extracts raw string command', () => {
    expect(describeTool('run_shell', 'echo hello' as unknown as Record<string, unknown>).target).toBe('echo hello')
  })

  it('falls back to (empty command) when no command is provided anywhere', () => {
    expect(describeTool('run_shell', {}).target).toBe('(empty command)')
  })
})

