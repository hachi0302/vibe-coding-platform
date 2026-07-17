// §10.5 上下文窗口指示 —— 纯函数，便于单测；UI 在 ChatComposer.vue。
//
// 已占用上下文取**最近一轮 result.usage**：input + cache_read + cache_creation
// （= 喂给模型的总输入 ≈ 当前上下文占用）。窗口大小 stream 不给，前端按 model 近似维护
// （见 contextWindowFor 注释）。故百分比是**近似值**，够用于「快满了」的直觉提示。

import type { Agent, UsageSummary } from './types'

/** 最近一轮喂给模型的总输入 tokens（≈ 当前上下文占用）。 */
export function usedContextTokens(u: UsageSummary | undefined | null): number {
  if (!u) return 0
  return (
    (u.inputTokens || 0) +
    (u.cacheReadInputTokens || 0) +
    (u.cacheCreationInputTokens || 0)
  )
}

/**
 * 模型 → 上下文窗口大小（近似，前端维护；headless stream 不含 modelContextWindow）。
 * - 含 `[1m]` 标记 → 1M（Claude 1M beta）。
 * - codex → 1M（其 gpt-5.x 默认大窗口，用户 config 多为 1_000_000）。
 * - 其余 claude → 200k；但若实际占用已 > 200k（只可能是 1M 窗口，标准 200k 装不下，
 *   常见于从 TUI 1M 会话续聊过来的种子值），按 1M 算，避免角标封顶在 100% 误导。
 *   transcript 的 model 字段不带 `[1m]` 标记，所以 1M 只能从占用量反推。
 */
export function contextWindowFor(
  agent: Agent,
  model: string | undefined | null,
  used = 0,
): number {
  const m = (model || '').toLowerCase()
  if (m.includes('[1m]')) return 1_000_000
  if (agent === 'codex') return 1_000_000
  if (agent === 'claude' && /^(opus|sonnet|fable)$/.test(m) && used > 200_000) return 1_000_000
  if (used > 200_000) return 1_000_000
  return 200_000
}

/** 上下文占用百分比（0..100 取整，封顶 100）。窗口 ≤0 → 0。 */
export function contextPercent(used: number, window: number): number {
  if (window <= 0) return 0
  return Math.min(100, Math.round((used / window) * 100))
}

/** 紧凑 token 数：85k / 1.2M。 */
export function formatTokensShort(n: number): string {
  if (n >= 1_000_000) {
    const v = n / 1_000_000
    return `${v >= 10 ? Math.round(v) : v.toFixed(1)}M`
  }
  if (n >= 1000) return `${Math.round(n / 1000)}k`
  return String(Math.round(n))
}
