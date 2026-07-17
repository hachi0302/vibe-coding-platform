// 账号额度（5 小时 / 周）的前端取数与展示模型。数据源是后端 `account_usage`
// 命令（OAuth 用量接口），每个窗口带精确利用率 + 重置时间。底栏的额度徽标据此渲染。
//
// 刷新策略 = 事件驱动 + 慢轮询兜底：
//   · 事件驱动：每轮对话结束（onResult）→ bumpUsage()，强制拉新（force=true 绕过后端缓存），
//     因为那一刻账号额度刚被这次对话消耗、值会变 —— 这才是真·实时，且只在会变时才打接口。
//   · 慢轮询：进入 live chat 时订阅（立即拉一次 + 之后每 60s），用来兜住「在本 app 之外」消耗
//     的用量（同时开着的终端 Claude Code / 别的机器）。不快轮询：接口对高频调用会持续 429。
// 多个订阅者共享同一个定时器（引用计数）。某次拉取失败不清空已有快照 —— 保留「最近一次成功」的值。
//
// 即时呈现：最近一次成功的快照写进 localStorage，模块加载时回种 usage.value —— 这样进入 live chat
// 的瞬间徽标就有值（不必干等首个网络请求返回，429 期间尤其明显），后台再静默 revalidate 覆盖。
import { ref } from 'vue'
import { accountUsage } from './api'
import type { AccountUsage } from './types'

/** 轮询间隔。接口对密集调用敏感（社区报告会持续 429），60s 足够且后端还有 20s TTL 缓存兜底。 */
const POLL_MS = 60_000

/** 最近一次成功快照的 localStorage 键（即时回种用）。 */
const CACHE_KEY = 'csv:usage:v1'

function loadCachedUsage(): AccountUsage | null {
  try {
    const raw = localStorage.getItem(CACHE_KEY)
    return raw ? (JSON.parse(raw) as AccountUsage) : null
  } catch {
    return null
  }
}
function saveCachedUsage(u: AccountUsage): void {
  try {
    localStorage.setItem(CACHE_KEY, JSON.stringify(u))
  } catch {
    /* 配额/隐私模式失败无妨，纯加速用 */
  }
}

/** 当前额度快照（响应式，供底栏徽标读取）。模块加载即用 localStorage 回种 → 开屏不空白。 */
export const usage = ref<AccountUsage | null>(loadCachedUsage())
/** 最近一次拉取的错误（成功后清空）。订阅账号无 OAuth 凭证时这里会有值。 */
export const usageError = ref<string | null>(null)

/** 两次「拉接口」之间的最小间隔。事件驱动强制刷新若距上次拉取不足这个间隔就跳过，
 *  防止密集对话把强制刷新叠成高频请求触发 429（接口对此很敏感）。略小于后端 20s 缓存。 */
const MIN_FETCH_GAP_MS = 15_000

/** 倒计时心跳间隔。重置倒计时只到「分」，30s 跳一次足够顺滑且几乎零成本（纯本地、不打接口）。 */
const TICK_MS = 30_000

/** 当前时间（ms），每 TICK_MS 跳一次。重置倒计时（formatRemaining）据此响应式重算 —— 纯前端、零网络。 */
export const nowMs = ref(Date.now())

let timer: ReturnType<typeof setInterval> | undefined
let tickTimer: ReturnType<typeof setInterval> | undefined
let subscribers = 0
let inFlight = false
let lastFetchAt = 0

async function refresh(force = false): Promise<void> {
  if (inFlight) return
  inFlight = true
  lastFetchAt = Date.now()
  try {
    const u = await accountUsage(force)
    usage.value = u
    saveCachedUsage(u) // 落盘最近一次成功值 → 下次开屏即时回种
    usageError.value = null
  } catch (e) {
    // 失败（含 429）不清空 usage.value —— 徽标保留「最近一次成功」的百分比，不闪空。
    usageError.value = String(e)
  } finally {
    inFlight = false
  }
}

/**
 * 事件驱动刷新：一轮对话结束后调用（见 chatSessions onResult）。强制拉新（绕过后端 20s 缓存），
 * 拿到刚被这次对话改变的最新额度。仅在有徽标正在展示（订阅中）时才打接口；没人看就不浪费请求，
 * 等下次进入 live chat 的首拉补上。再加一道节流：距上次拉取不足 MIN_FETCH_GAP_MS 就跳过 ——
 * 正常对话间隔（人+模型延迟）远大于此，首轮结束即可实时刷新；只有密集连发才会被节流到下次轮询补上。
 */
export function bumpUsage(): void {
  if (subscribers === 0) return
  if (Date.now() - lastFetchAt < MIN_FETCH_GAP_MS) return
  void refresh(true)
}

/** 订阅额度轮询（第一个订阅者立即拉一次并启动定时器 + 倒计时心跳）。配 stopUsagePolling 用。 */
export function startUsagePolling(): void {
  subscribers += 1
  if (subscribers === 1) {
    void refresh()
    timer = setInterval(() => void refresh(), POLL_MS)
    nowMs.value = Date.now()
    tickTimer = setInterval(() => { nowMs.value = Date.now() }, TICK_MS)
  }
}

/** 退订（最后一个订阅者离开时停掉定时器 + 心跳）。 */
export function stopUsagePolling(): void {
  subscribers = Math.max(0, subscribers - 1)
  if (subscribers === 0) {
    if (timer) { clearInterval(timer); timer = undefined }
    if (tickTimer) { clearInterval(tickTimer); tickTimer = undefined }
  }
}

export type UsageWindowKey = 'five_hour' | 'seven_day'
export interface UsageWindowView {
  key: UsageWindowKey
  /** 取整后的利用率百分比 0–100。 */
  percent: number
  /** ISO8601 重置时间（可能缺失）。 */
  resetsAt?: string
}

/** 把额度快照整理成固定顺序 [5h, 周] 的展示窗口列表；窗口对象不存在则跳过。 */
export function usageWindows(u: AccountUsage | null | undefined): UsageWindowView[] {
  if (!u) return []
  const out: UsageWindowView[] = []
  if (u.fiveHour) {
    out.push({ key: 'five_hour', percent: Math.round(u.fiveHour.utilization ?? 0), resetsAt: u.fiveHour.resetsAt ?? undefined })
  }
  if (u.sevenDay) {
    out.push({ key: 'seven_day', percent: Math.round(u.sevenDay.utilization ?? 0), resetsAt: u.sevenDay.resetsAt ?? undefined })
  }
  return out
}

/** 额度徽标配色档位：与上下文徽标（cc-ctx）一致 —— ≥90 红、≥70 紫、其余常规。 */
export function usageLevel(percent: number): 'normal' | 'warn' | 'danger' {
  if (percent >= 90) return 'danger'
  if (percent >= 70) return 'warn'
  return 'normal'
}

/**
 * 重置倒计时：距 `resetsAt` 还剩多久，紧凑格式 `2d6h` / `4h30m` / `45m` / `<1m`。
 * 纯函数（now 由调用方传入响应式 nowMs，便于测试 + 每跳重算）。无效/已过期 → 空串（不显示）。
 */
export function formatRemaining(resetsAt: string | undefined | null, now: number): string {
  if (!resetsAt) return ''
  const t = new Date(resetsAt).getTime()
  if (Number.isNaN(t)) return ''
  let s = Math.floor((t - now) / 1000)
  if (s <= 0) return '' // 已过期 / 正在重置：不显示倒计时
  const d = Math.floor(s / 86400); s -= d * 86400
  const h = Math.floor(s / 3600); s -= h * 3600
  const m = Math.floor(s / 60)
  if (d > 0) return `${d}d${h}h`
  if (h > 0) return `${h}h${m}m`
  if (m > 0) return `${m}m`
  return '<1m'
}
