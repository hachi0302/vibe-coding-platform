// Auto 权限模式的「按工作区记住」开关。
//
// 切到 auto（自动）权限模式时弹一次二次确认（见 AutoModeConfirmModal）；用户点「Enable
// auto mode」后，把该工作区（项目 cwd）记下来，之后在同一工作区切 auto 不再追问
// —— 对齐截图里「You won't be asked again for this workspace」。
//
// 存 localStorage（仅前端、跨 webview 刷新有效；chat 子进程本就刷新即回收，无需后端持久化）。
// 纯字符串集合，便于在 jsdom 下单测。

const KEY = 'autoModeConfirmedWorkspaces'

function load(): Set<string> {
  try {
    const raw = localStorage.getItem(KEY)
    if (!raw) return new Set()
    const arr = JSON.parse(raw)
    return Array.isArray(arr) ? new Set(arr.filter((x) => typeof x === 'string')) : new Set()
  } catch {
    return new Set()
  }
}

/** 该工作区是否已确认过 auto 模式（确认过则切 auto 不再弹框）。 */
export function isAutoModeConfirmed(cwd: string | undefined): boolean {
  if (!cwd) return false
  return load().has(cwd)
}

/** 记住该工作区已确认 auto 模式。 */
export function rememberAutoModeConfirmed(cwd: string | undefined): void {
  if (!cwd) return
  const set = load()
  if (set.has(cwd)) return
  set.add(cwd)
  try {
    localStorage.setItem(KEY, JSON.stringify([...set]))
  } catch {
    /* 存储不可用（隐私模式 / 配额）时静默：大不了下次再确认一遍 */
  }
}
