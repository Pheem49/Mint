import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import SkillsView from './SkillsView'

describe('SkillsView workspace display', () => {
  it('displays the active workspace path in the header', () => {
    const html = renderToStaticMarkup(
      <SkillsView
        listSkills={async () => []}
        addSkill={async () => {}}
        deleteSkill={async () => {}}
        workspacePath="/workspace/project"
      />,
    )
    expect(html).toContain('/workspace/project')
    expect(html).toContain('Workspace')
  })

  it('indicates when no workspace is selected', () => {
    const html = renderToStaticMarkup(
      <SkillsView
        listSkills={async () => []}
        addSkill={async () => {}}
        deleteSkill={async () => {}}
        workspacePath=""
      />,
    )
    expect(html).toContain('No active workspace')
  })
})
