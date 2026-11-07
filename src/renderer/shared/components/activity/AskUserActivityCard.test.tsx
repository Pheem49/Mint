import React from 'react'
import { renderToStaticMarkup } from 'react-dom/server'
import { describe, expect, it } from 'vitest'
import { AskUserActivityCard } from './AskUserActivityCard'
import { AgentActivityTable } from './AgentActivityTable'
import { activitiesFrom } from '../../utils/agentActivity'

describe('AskUserActivityCard', () => {
  it('returns null when questions list is empty', () => {
    const html = renderToStaticMarkup(<AskUserActivityCard questions={[]} />)
    expect(html).toBe('')
  })

  it('renders single question card with "1 question" header and answer', () => {
    const html = renderToStaticMarkup(
      <AskUserActivityCard
        questions={[
          {
            question: 'Which framework should we use?',
            answer: 'React + TypeScript',
          },
        ]}
      />,
    )

    expect(html).toContain('1 question')
    expect(html).toContain('Which framework should we use?')
    expect(html).toContain('React + TypeScript')
    expect(html).toContain('modern-ask-user-card')
    expect(html).toContain('modern-ask-user-header')
  })

  it('renders multi-question card matching user design with "3 questions"', () => {
    const html = renderToStaticMarkup(
      <AskUserActivityCard
        questions={[
          {
            question: 'คุณต้องการใช้ภาษาและเทคโนโลยีใดในการสร้าง MCP Server (Hardware Tooling)?',
            answer: '(Recommended) Python (ใช้ FastMCP / mcp SDK + pyserial + psutil) — พัฒนาไวและมีไลบรารีฮาร์ดแวร์/OS ครบครัน',
          },
          {
            question: 'คุณต้องการให้สร้างโมดูลใดบ้างในเวอร์ชันแรกนี้?',
            answer: 'USB / Serial Bridge, Hardware Flashing & Toolchains, Process & Port Management',
          },
          {
            question: 'สำหรับฝั่ง Toolchain & Flashing คุณใช้งานเครื่องมือตัวไหนเป็นหลัก?',
            answer: 'Arduino CLI (arduino-cli), PlatformIO (pio), ESP-IDF / esptool',
          },
        ]}
      />,
    )

    expect(html).toContain('3 questions')
    expect(html).toContain('คุณต้องการใช้ภาษาและเทคโนโลยีใดในการสร้าง MCP Server')
    expect(html).toContain('(Recommended) Python')
    expect(html).toContain('คุณต้องการให้สร้างโมดูลใดบ้างในเวอร์ชันแรกนี้?')
    expect(html).toContain('USB / Serial Bridge')
    expect(html).toContain('Toolchain')
    expect(html).toContain('Flashing คุณใช้งานเครื่องมือตัวไหนเป็นหลัก?')
    expect(html).toContain('Arduino CLI')
  })

  it('renders optional header in headerText if specified', () => {
    const html = renderToStaticMarkup(
      <AskUserActivityCard
        header="MCP Config"
        questions={[
          {
            question: 'Port number?',
            answer: '3000',
          },
        ]}
      />,
    )

    expect(html).toContain('1 question · MCP Config')
    expect(html).toContain('Port number?')
    expect(html).toContain('3000')
  })

  it('renders within AgentActivityTable when activity is answered (done)', () => {
    const view = activitiesFrom([
      {
        type: 'ToolStart',
        data: {
          action: 'ask_user',
          input: { query: 'Which model should be used?' },
        },
      },
      {
        type: 'ToolEnd',
        data: {
          action: 'ask_user',
          result: 'User answered: Gemini 2.5 Pro',
        },
      },
    ])

    const html = renderToStaticMarkup(<AgentActivityTable activityView={view} />)
    expect(html).toContain('modern-ask-user-card')
    expect(html).toContain('1 question')
    expect(html).toContain('Which model should be used?')
    expect(html).toContain('Gemini 2.5 Pro')
  })

  it('renders standard active item while ask_user is waiting to be answered', () => {
    const view = activitiesFrom([
      {
        type: 'ToolStart',
        data: {
          action: 'ask_user',
          input: { query: 'Waiting for your answer...' },
        },
      },
    ])

    const html = renderToStaticMarkup(<AgentActivityTable activityView={view} />)
    expect(html).not.toContain('modern-ask-user-card')
    expect(html).toContain('Asked')
    expect(html).toContain('Waiting for your answer...')
  })
})
