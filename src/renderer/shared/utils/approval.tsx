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

// eslint-disable-next-line @typescript-eslint/no-explicit-any
export function renderApprovalDetails(approval: any): ApprovalDetails {
  if (!approval) return { title: 'Action Pending Approval', body: 'No action details available.', isDangerous: false }
  if (approval.WriteFile) return { title: 'Write File', body: `Path: ${approval.WriteFile.path}`, reason: approval.WriteFile.diff ? `Diff:\n${approval.WriteFile.diff}` : 'Writing new file content.', isDangerous: false }
  if (approval.ApplyPatch) return { title: 'Apply Patch', body: `Path: ${approval.ApplyPatch.path}`, reason: approval.ApplyPatch.diff ? `Diff:\n${approval.ApplyPatch.diff}` : 'Applying code patch.', isDangerous: false }
  if (approval.RunShell) return { title: 'Run Shell Command', body: approval.RunShell.command, reason: 'Executing shell commands can modify your system.', isDangerous: true }
  if (approval.NoteWrite) return { title: 'Write Note', body: `Path: ${approval.NoteWrite.path}`, reason: 'Creating or updating workspace notes.', isDangerous: false }
  if (approval.RunPlugin) return { title: `Run Plugin: ${approval.RunPlugin.name}`, body: approval.RunPlugin.instruction, reason: 'Executing a native plugin action.', isDangerous: false }
  if (approval.McpTool) {
    const { server, tool, arguments: args } = approval.McpTool
    return { title: `Run MCP Tool: ${server}/${tool}`, body: typeof args === 'string' ? args : JSON.stringify(args, null, 2), reason: 'Running external MCP tool.', isDangerous: false }
  }
  if (approval.UserApproval) return { title: approval.UserApproval.title, body: approval.UserApproval.prompt, reason: 'The agent requested explicit approval.', isDangerous: false }
  if (approval.AskUser) return { title: 'Question From Agent', body: approval.AskUser.question, reason: 'Type your answer below and submit to respond to the agent.', isDangerous: false }
  return { title: 'Unknown Action', body: JSON.stringify(approval, null, 2), reason: 'Requires approval to proceed.', isDangerous: false }
}

export function renderDiff(diffText: string): ReactNode {
  if (!diffText) return null
  const lines = diffText.split('\n')
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
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        let style: any = { whiteSpace: 'pre-wrap', padding: '2px 6px' }
        if (line.startsWith('@@') || line.startsWith('---') || line.startsWith('+++')) {
          style = { ...style, color: '#64748b', fontWeight: 'bold' }
        } else if (line.startsWith('+')) {
          style = { ...style, background: 'rgba(16, 185, 129, 0.12)', borderLeft: '3px solid #10b981', color: '#a7f3d0' }
        } else if (line.startsWith('-')) {
          style = { ...style, background: 'rgba(239, 68, 68, 0.12)', borderLeft: '3px solid #ef4444', color: '#fca5a5' }
        } else {
          style = { ...style, color: '#e2e8f0' }
        }
        return <div key={idx} style={style}>{line}</div>
      })}
    </div>
  )
}
