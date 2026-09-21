import { describe, expect, it } from 'vitest'
import { parseCardJsonSafely } from './cardJson'

describe('JSON-backed chat card parsing', () => {
  it('parses valid card data without changing its content', () => {
    const payload = { title: 'Weather', daily: { dates: ['2026-09-21'] } }
    expect(parseCardJsonSafely(JSON.stringify(payload))).toEqual(payload)
  })

  it('repairs detached scalar fields in arrays of card items', () => {
    const malformed = '[{"title":"Plan","desc":"Define the rules"}, "badge":"แผน"}]'
    expect(parseCardJsonSafely(malformed)).toEqual([
      { title: 'Plan', desc: 'Define the rules', badge: 'แผน' },
    ])
  })

  it('repairs detached numeric and boolean properties too', () => {
    expect(parseCardJsonSafely('[{"title":"Metric"}, "score":3, "active":true}]')).toEqual([
      { title: 'Metric', score: 3, active: true },
    ])
  })

  it('repairs detached fields on every item in a card grid', () => {
    const malformed = '[{"icon":"flask-conical","title":"ออกแบบการ dogfood","desc":"วางกติกา: กี่งาน/วัน, friction log, exit criteria"}, "badge":"แผน"}, {"icon":"layers","title":"เจาะ 7 ชั้น","desc":"เทียบ Mint-CLI vs OpenHands ทีละชั้น ว่าต่างตรงไหน","badge":"วิเคราะห์"}, {"icon":"shift-right","title":"วางก่อน","desc":"คุยเล่นต่อ ไม่ต้องลงมือ","badge":"แค่คุย"}]'
    expect(parseCardJsonSafely(malformed)).toEqual([
      { icon: 'flask-conical', title: 'ออกแบบการ dogfood', desc: 'วางกติกา: กี่งาน/วัน, friction log, exit criteria', badge: 'แผน' },
      { icon: 'layers', title: 'เจาะ 7 ชั้น', desc: 'เทียบ Mint-CLI vs OpenHands ทีละชั้น ว่าต่างตรงไหน', badge: 'วิเคราะห์' },
      { icon: 'shift-right', title: 'วางก่อน', desc: 'คุยเล่นต่อ ไม่ต้องลงมือ', badge: 'แค่คุย' },
    ])
  })

  it('does not guess how to repair unrelated malformed JSON', () => {
    expect(parseCardJsonSafely('[{"title":"Plan", "unknown":}]')).toBeNull()
  })
})
