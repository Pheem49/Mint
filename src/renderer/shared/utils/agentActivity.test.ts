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

