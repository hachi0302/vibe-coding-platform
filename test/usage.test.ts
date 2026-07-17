import { describe, expect, it } from 'vitest'
import { usageWindows, usageLevel, formatRemaining } from '../src/usage'
import type { AccountUsage } from '../src/types'

describe('usageWindows', () => {
  it('空快照 → 空数组', () => {
    expect(usageWindows(null)).toEqual([])
    expect(usageWindows(undefined)).toEqual([])
    expect(usageWindows({})).toEqual([])
  })

  it('固定顺序 [5h, 周]，百分比取整', () => {
    const u: AccountUsage = {
      sevenDay: { utilization: 81.4, resetsAt: '2026-06-28T05:00:00Z' },
      fiveHour: { utilization: 10.6, resetsAt: '2026-06-26T18:00:00Z' },
    }
    expect(usageWindows(u)).toEqual([
      { key: 'five_hour', percent: 11, resetsAt: '2026-06-26T18:00:00Z' },
      { key: 'seven_day', percent: 81, resetsAt: '2026-06-28T05:00:00Z' },
    ])
  })

  it('只有一个窗口时只回该窗口', () => {
    expect(usageWindows({ sevenDay: { utilization: 50 } })).toEqual([
      { key: 'seven_day', percent: 50, resetsAt: undefined },
    ])
  })

  it('utilization 缺失按 0 处理；null 窗口跳过', () => {
    const u = { fiveHour: { utilization: undefined as unknown as number }, sevenDay: null }
    expect(usageWindows(u as AccountUsage)).toEqual([
      { key: 'five_hour', percent: 0, resetsAt: undefined },
    ])
  })
})

describe('usageLevel', () => {
  it('≥90 红 / ≥70 紫 / 其余常规（与 context 一致）', () => {
    expect(usageLevel(0)).toBe('normal')
    expect(usageLevel(69)).toBe('normal')
    expect(usageLevel(70)).toBe('warn')
    expect(usageLevel(89)).toBe('warn')
    expect(usageLevel(90)).toBe('danger')
    expect(usageLevel(100)).toBe('danger')
  })
})

describe('formatRemaining', () => {
  const now = Date.parse('2026-06-26T12:00:00Z')
  const at = (ms: number) => new Date(now + ms).toISOString()
  const MIN = 60_000
  const HR = 60 * MIN
  const DAY = 24 * HR

  it('缺失 / 非法 / 已过期 → 空串', () => {
    expect(formatRemaining(undefined, now)).toBe('')
    expect(formatRemaining(null, now)).toBe('')
    expect(formatRemaining('not-a-date', now)).toBe('')
    expect(formatRemaining(at(-1), now)).toBe('') // 已过期
    expect(formatRemaining(at(0), now)).toBe('') // 正好到点
  })

  it('紧凑格式 d/h/m', () => {
    expect(formatRemaining(at(45 * MIN), now)).toBe('45m')
    expect(formatRemaining(at(4 * HR + 30 * MIN), now)).toBe('4h30m')
    expect(formatRemaining(at(2 * DAY + 6 * HR + 59 * MIN), now)).toBe('2d6h') // 天级省略分钟
    expect(formatRemaining(at(39 * HR + 30 * MIN), now)).toBe('1d15h') // ≥24h 进位到天
  })

  it('不足 1 分钟 → <1m', () => {
    expect(formatRemaining(at(30_000), now)).toBe('<1m')
  })
})
