import { describe, expect, it } from 'vitest'
import {
  usedContextTokens,
  contextWindowFor,
  contextPercent,
  formatTokensShort,
} from '../src/chatContext'
import type { UsageSummary } from '../src/types'

const usage = (p: Partial<UsageSummary>): UsageSummary => ({
  inputTokens: 0,
  outputTokens: 0,
  cacheCreationInputTokens: 0,
  cacheCreation1hInputTokens: 0,
  cacheReadInputTokens: 0,
  reasoningOutputTokens: 0,
  total: 0,
  ...p,
})

describe('chatContext', () => {
  it('usedContextTokens = input + cacheRead + cacheCreation', () => {
    expect(
      usedContextTokens(usage({ inputTokens: 1000, cacheReadInputTokens: 20000, cacheCreationInputTokens: 500 })),
    ).toBe(21500)
    expect(usedContextTokens(undefined)).toBe(0)
    expect(usedContextTokens(null)).toBe(0)
  })

  it('contextWindowFor：claude 默认 200k，[1m] → 1M，codex → 1M', () => {
    expect(contextWindowFor('claude', 'claude-opus-4-8')).toBe(200_000)
    expect(contextWindowFor('claude', 'claude-opus-4-8[1m]')).toBe(1_000_000)
    expect(contextWindowFor('claude', 'opus')).toBe(200_000)
    expect(contextWindowFor('codex', 'gpt-5.4')).toBe(1_000_000)
    expect(contextWindowFor('claude', undefined)).toBe(200_000)
  })

  it('contextWindowFor：占用 > 200k 反推 1M（从 TUI 1M 续聊的种子值）', () => {
    // 标准 200k 装不下 366k，只可能是 1M 窗口；model 字段不带 [1m] 标记时靠占用反推。
    expect(contextWindowFor('claude', 'claude-opus-4-8', 366_824)).toBe(1_000_000)
    // 占用仍在 200k 以内 → 维持 200k（live GUI chat 走标准窗口，不误判）。
    expect(contextWindowFor('claude', 'claude-opus-4-8', 150_000)).toBe(200_000)
  })

  it('contextPercent：取整、封顶 100、窗口 0 → 0', () => {
    expect(contextPercent(50_000, 200_000)).toBe(25)
    expect(contextPercent(300_000, 200_000)).toBe(100)
    expect(contextPercent(123, 0)).toBe(0)
  })

  it('formatTokensShort：k / M', () => {
    expect(formatTokensShort(500)).toBe('500')
    expect(formatTokensShort(21_500)).toBe('22k')
    expect(formatTokensShort(1_200_000)).toBe('1.2M')
    expect(formatTokensShort(12_000_000)).toBe('12M')
  })
})
