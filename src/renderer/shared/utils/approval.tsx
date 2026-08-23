/**
 * shared/utils/approval.tsx
 * Approval detail parsing and diff rendering helpers.
 * Shared by both Desktop and Web ChatPanel — do NOT duplicate this.
 */
import React, { type ReactNode } from 'react'


export interface ApprovalDetails {
  title: string
  body: string
  reason?: string
  isDangerous: boolean
}

function parseDiffStats(diffText?: string): string {
  if (!diffText) return ''
  let additions = 0
  let deletions = 0
  const lines = diffText.split('\n')
  for (const line of lines) {
    if (line.startsWith('+++ ') || line.startsWith('--- ')) continue
    if (line.startsWith('+')) additions++
    else if (line.startsWith('-')) deletions++
  }
  return `(+${additions} -${deletions})`
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function renderApprovalDetails(approval: any): ApprovalDetails {
  if (!approval) return { title: 'Action Pending Approval', body: 'No action details available.', isDangerous: false }
  if (approval.WriteFile) {
    const stats = parseDiffStats(approval.WriteFile.diff)
    return { title: 'Write File', body: `Path: ${approval.WriteFile.path}${stats ? ` ${stats}` : ''}`, reason: approval.WriteFile.diff ? `Diff:\n${approval.WriteFile.diff}` : 'Writing new file content.', isDangerous: false }
  }
  if (approval.ApplyPatch) {
    const stats = parseDiffStats(approval.ApplyPatch.diff)
    return { title: 'Apply Patch', body: `Path: ${approval.ApplyPatch.path}${stats ? ` ${stats}` : ''}`, reason: approval.ApplyPatch.diff ? `Diff:\n${approval.ApplyPatch.diff}` : 'Applying code patch.', isDangerous: false }
  }
  if (approval.RunShell) return { title: 'Run Shell Command', body: approval.RunShell.command, reason: 'Executing shell commands can modify your system.', isDangerous: true }
  if (approval.NoteWrite) return { title: 'Write Note', body: `Path: ${approval.NoteWrite.path}`, reason: 'Creating or updating workspace notes.', isDangerous: false }
  if (approval.RunPlugin) return { title: `Run Plugin: ${approval.RunPlugin.name}`, body: approval.RunPlugin.instruction, reason: 'Executing a native plugin action.', isDangerous: false }
  if (approval.McpTool) {
    const { server, tool, arguments: args } = approval.McpTool
    return { title: `Run MCP Tool: ${server}/${tool}`, body: typeof args === 'string' ? args : JSON.stringify(args, null, 2), reason: 'Running external MCP tool.', isDangerous: false }
  }
  if (approval.UserApproval) return { title: approval.UserApproval.title, body: approval.UserApproval.prompt, reason: 'The agent requested explicit approval.', isDangerous: false }
  if (approval.AskUser) {
    const hasOptions = Array.isArray(approval.AskUser.options) && approval.AskUser.options.length > 0
    const multiSelect = !!approval.AskUser.multiSelect
    const pickReason = multiSelect
      ? 'Pick one or more choices below, or type your own answer and submit.'
      : 'Pick a choice below, or type your own answer and submit.'
    return {
      title: 'Question From Agent',
      body: approval.AskUser.question,
      reason: hasOptions ? pickReason : 'Type your answer below and submit to respond to the agent.',
      isDangerous: false,
    }
  }
  if (approval.ExitPlanMode) return { title: 'Review Plan', body: approval.ExitPlanMode.plan, reason: 'Approve to allow file edits and shell commands. Reject to keep the agent investigating.', isDangerous: false }
  return { title: 'Unknown Action', body: JSON.stringify(approval, null, 2), reason: 'Requires approval to proceed.', isDangerous: false }
}

function parseHunkHeader(line: string): { oldStart: number; newStart: number } | null {
  const trimmed = line.trim()
  if (!trimmed.startsWith('@@')) return null
  const parts = trimmed.split('@@')
  if (parts.length < 3) return null
  const headerBody = parts[1].trim()
  let oldStart = 1
  let newStart = 1

  for (const token of headerBody.split(/\s+/)) {
    if (token.startsWith('-')) {
      const numStr = token.slice(1).split(',')[0]
      const parsed = parseInt(numStr, 10)
      if (!isNaN(parsed)) oldStart = parsed
    } else if (token.startsWith('+')) {
      const numStr = token.slice(1).split(',')[0]
      const parsed = parseInt(numStr, 10)
      if (!isNaN(parsed)) newStart = parsed
    }
  }
  return { oldStart, newStart }
}

export function renderDiff(diffText: string): ReactNode {
  if (!diffText) return null
  const lines = diffText.split('\n')
  let currentOldLine = 1
  let currentNewLine = 1

  return (
    <div style={{
      background: '#0b0f19',
      borderRadius: '6px',
      padding: '8px',
      border: '1px solid rgba(255, 255, 255, 0.08)',
      overflowX: 'auto',
      maxHeight: '400px',
      fontFamily: 'monospace',
      fontSize: '0.74rem',
      lineHeight: '1.4',
    }}>
      {lines.map((line, idx) => {
        if (line.startsWith('@@')) {
          const hunk = parseHunkHeader(line)
          if (hunk) {
            currentOldLine = hunk.oldStart
            currentNewLine = hunk.newStart
          }
          return (
            <div key={idx} style={{ color: '#64748b', fontWeight: 'bold', padding: '2px 6px' }}>
              {'       ' + line}
            </div>
          )
        }
        if (line.startsWith('---') || line.startsWith('+++')) {
          return (
            <div key={idx} style={{ color: '#64748b', fontWeight: 'bold', padding: '2px 6px' }}>
              {'       ' + line}
            </div>
          )
        }

        let lineNum = ''
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        let style: any = { whiteSpace: 'pre-wrap', padding: '2px 6px', display: 'flex', alignItems: 'center' }
        if (line.startsWith('+')) {
          lineNum = String(currentNewLine)
          currentNewLine++
          style = { ...style, background: 'rgba(16, 185, 129, 0.12)', borderLeft: '3px solid #10b981', color: '#a7f3d0' }
        } else if (line.startsWith('-')) {
          lineNum = String(currentOldLine)
          currentOldLine++
          style = { ...style, background: 'rgba(239, 68, 68, 0.12)', borderLeft: '3px solid #ef4444', color: '#fca5a5' }
        } else {
          lineNum = String(currentNewLine)
          currentOldLine++
          currentNewLine++
          style = { ...style, color: '#e2e8f0' }
        }

        return (
          <div key={idx} style={style}>
            <span style={{ color: '#64748b', marginRight: '12px', userSelect: 'none', display: 'inline-block', width: '36px', textAlign: 'right', flexShrink: 0 }}>
              {lineNum}
            </span>
            <span>{line}</span>
          </div>
        )
      })}
    </div>
  )
}
