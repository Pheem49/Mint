import { describe, expect, it } from 'vitest'
import {
  describeTool,
  activitiesFrom,
  generateNaturalActivitySummary,
  formatDurationText,
  groupActivities,
  isMarkdownContent,
  cleanMarkdownOutput,
  activityKind,
  isAskUserActivity,
  extractAskUserQuestions,
} from './agentActivity'
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

  it('ignores dummy null/undefined strings and picks real command over path/query for run_shell', () => {
    expect(
      describeTool('run_shell', {
        command: 'cd /tmp && ls -la',
        path: 'null',
        query: 'null',
        name: 'null',
        title: 'null',
      }).target
    ).toBe('cd /tmp && ls -la')

    expect(
      describeTool('run_shell', {
        command: 'curl -s https://api.twitter.com',
        path: 'null',
        query: 'null',
      }).target
    ).toBe('curl -s https://api.twitter.com')
  })

  it('treats standalone dummy string command as empty and falls back to (empty command)', () => {
    expect(describeTool('run_shell', { command: 'null' }).target).toBe('(empty command)')
    expect(describeTool('run_shell', { command: 'undefined' }).target).toBe('(empty command)')
    expect(describeTool('run_shell', { command: 'None' }).target).toBe('(empty command)')
  })
})

describe('action-first target resolution and dummy value stripping', () => {
  it('prioritizes query for web_search even when dummy path or other keys are present', () => {
    expect(
      describeTool('web_search', {
        query: 'Kuuko W official twitter',
        path: 'null',
        command: 'null',
        name: 'null',
      }).target
    ).toBe('Kuuko W official twitter')
  })

  it('strips dummy query for web_search and falls back to (empty query)', () => {
    expect(describeTool('web_search', { query: 'null' }).target).toBe('(empty query)')
    expect(describeTool('web_search', { query: 'undefined' }).target).toBe('(empty query)')
  })

  it('prioritizes path for read_file even when dummy command or query are present', () => {
    expect(
      describeTool('read_file', {
        path: 'src/main.rs',
        command: 'null',
        query: 'null',
      }).target
    ).toBe('src/main.rs')
  })
})

describe('live progress retention across multi-step turns', () => {
  it('preserves all tool executions and timeline groups without dropping earlier tools', () => {
    let progress: AgentProgress[] = []
    // Simulate 15 shell tool executions interleaved with thoughts
    for (let i = 0; i < 15; i++) {
      progress = reduceLiveAgentProgress(progress, {
        type: 'Thought',
        data: { thought: `Thinking step ${i}` },
      })
      progress = reduceLiveAgentProgress(progress, {
        type: 'ToolStart',
        data: { action: 'run_shell', input: { command: `echo step ${i}` }, callId: `call-${i}` },
      })
      progress = reduceLiveAgentProgress(progress, {
        type: 'ToolEnd',
        data: { action: 'run_shell', input: { command: `echo step ${i}` }, result: `step ${i}`, callId: `call-${i}` },
      })
    }

    const view = activitiesFrom(progress)
    // All 15 tool executions must be preserved
    expect(view.items).toHaveLength(15)
    // The very first tool execution must still be present and intact
    expect(view.items[0].target).toBe('echo step 0')
    expect(view.items[14].target).toBe('echo step 14')
    // Timeline must not drop the early groups
    expect(view.timeline?.length).toBeGreaterThan(15)
  })
})

describe('modern semantic activity timeline metadata', () => {
  it('extracts Analyzed verb and line range for read_file', () => {
    const tool = describeTool('read_file', { path: 'src/App.tsx', startLine: 25, endLine: 50 })
    expect(tool.semanticVerb).toBe('Analyzed')
    expect(tool.lineRange).toBe('#L25-50')
  })

  it('extracts Edited verb and diff stats for edit_file', () => {
    const tool = describeTool('edit_file', {
      Path: 'src/App.tsx',
      Target: 'line1\nline2',
      Replacement: 'line1\nline2\nline3\nline4',
    })
    expect(tool.kind).toBe('file')
    expect(tool.semanticVerb).toBe('Edited')
    expect(tool.diffStats).toEqual({ additions: 4, deletions: 2 })
  })

  it('classifies edit_file, write_file, read_file and apply_code_edits as file kind', () => {
    expect(activityKind('edit_file', 'src/main.py')).toBe('file')
    expect(activityKind('write_file', 'src/main.py')).toBe('file')
    expect(activityKind('read_file', 'src/main.py')).toBe('file')
    expect(activityKind('apply_code_edits', 'src/main.py')).toBe('file')
  })

  it('extracts Ran verb and toolSummary intent for run_command', () => {
    const tool = describeTool('run_command', {
      CommandLine: 'npm run build:ui',
      toolSummary: 'Build UI for Desktop and Web',
    })
    expect(tool.semanticVerb).toBe('Ran')
    expect(tool.intentTitle).toBe('Build UI for Desktop and Web')
  })

  it('generates natural composite activity summary', () => {
    const view = activitiesFrom([
      { type: 'ToolStart', data: { action: 'read_file', input: { path: 'a.ts' } } },
      { type: 'ToolEnd', data: { action: 'read_file', input: { path: 'a.ts' }, result: 'ok' } },
      { type: 'ToolStart', data: { action: 'run_shell', input: { command: 'cargo check' } } },
      { type: 'ToolEnd', data: { action: 'run_shell', input: { command: 'cargo check' }, result: 'ok' } },
      { type: 'ToolStart', data: { action: 'write_file', input: { path: 'b.ts', content: 'hello' } } },
      { type: 'ToolEnd', data: { action: 'write_file', input: { path: 'b.ts' }, result: 'ok' } },
    ])
    expect(view.naturalSummary).toBe('Explored files, ran commands, edited files')
  })

  it('formats duration cleanly', () => {
    expect(formatDurationText(14)).toBe('14s')
    expect(formatDurationText(120)).toBe('2m')
    expect(formatDurationText(75)).toBe('1m 15s')
  })

  it('assigns semantic verbs and clean titles to grouped activities', () => {
    const items = [
      describeTool('run_shell', { command: 'echo 1' }),
      describeTool('run_shell', { command: 'echo 2' }),
    ]
    const groups = groupActivities(items)
    expect(groups).toHaveLength(1)
    expect(groups[0].semanticVerb).toBe('Ran')
    expect(groups[0].title).toBe('2 commands')
  })

  it('detects markdown content for skill and doc reads', () => {
    expect(isMarkdownContent('read_file', 'SKILL.md', '# Skill content')).toBe(true)
    expect(isMarkdownContent('read_file', 'README.md', 'Hello world')).toBe(true)
    expect(isMarkdownContent('read_file', 'a.ts', '[Loaded skill: ffmpeg]')).toBe(true)
    expect(isMarkdownContent('run_shell', 'npm test', 'PASS 1 test')).toBe(false)
    expect(isMarkdownContent('read_file', 'src/App.tsx', 'const x = 1;')).toBe(false)
  })

  it('cleans loaded skill frontmatter into readable markdown callout', () => {
    const raw = `[Loaded skill: ffmpeg-frame-extractor]
---
description: Extract video frames without distortion
revisions: 1
---

# Extracting frames
## Usage
- Run command`
    const cleaned = cleanMarkdownOutput(raw)
    expect(cleaned).toContain('**Skill: ffmpeg-frame-extractor**')
    expect(cleaned).toContain('> Extract video frames without distortion')
    expect(cleaned).toContain('# Extracting frames')
    expect(cleaned).not.toContain('---')
  })

  describe('ask_user activities', () => {
    it('correctly creates an ask_user activity with single query', () => {
      const act = describeTool('ask_user', { query: 'Which database should we use?' })
      expect(isAskUserActivity(act)).toBe(true)
      expect(act.semanticVerb).toBe('Asked')
      expect(act.askUserData?.questions).toHaveLength(1)
      expect(act.askUserData?.questions[0].question).toBe('Which database should we use?')
    })

    it('correctly creates an ask_user activity with multiple questions', () => {
      const act = describeTool('ask_user', {
        header: 'Architecture Choices',
        questions: [
          { question: 'Q1: Language?', answer: 'Rust' },
          { question: 'Q2: Database?', answer: 'PostgreSQL' },
        ],
      })
      expect(isAskUserActivity(act)).toBe(true)
      expect(act.askUserData?.header).toBe('Architecture Choices')
      expect(act.askUserData?.questions).toHaveLength(2)
      expect(act.askUserData?.questions[0].question).toBe('Q1: Language?')
      expect(act.askUserData?.questions[0].answer).toBe('Rust')
    })

    it('attaches answer from ToolEnd event in activitiesFrom', () => {
      const view = activitiesFrom([
        {
          type: 'ToolStart',
          data: {
            action: 'ask_user',
            input: { query: 'Choose a port:' },
          },
        },
        {
          type: 'ToolEnd',
          data: {
            action: 'ask_user',
            result: 'User answered: 8080',
          },
        },
      ])

      expect(view.items).toHaveLength(1)
      const act = view.items[0]
      expect(act.state).toBe('done')
      const questions = extractAskUserQuestions(act)
      expect(questions).toHaveLength(1)
      expect(questions[0].question).toBe('Choose a port:')
      expect(questions[0].answer).toBe('8080')
    })

    it('extracts questions from legacy (Answered: "...") format in target', () => {
      const legacyAct = {
        label: 'ask_user',
        action: 'ask_user',
        target: '(Answered: "Python 3.11") Which Python version to use?',
        kind: 'tool' as const,
        state: 'done' as const,
      }
      expect(isAskUserActivity(legacyAct)).toBe(true)
      const extracted = extractAskUserQuestions(legacyAct)
      expect(extracted).toHaveLength(1)
      expect(extracted[0].question).toBe('Which Python version to use?')
      expect(extracted[0].answer).toBe('Python 3.11')
    })
  })
})


