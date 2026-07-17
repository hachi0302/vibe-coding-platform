import { ref } from 'vue'
import type { Agent } from './types'

// 「最近打开过的项目」—— 按 agent 分桶存 dirName，最近在前。
// 仅记录 app 内的打开行为，与项目目录 mtime 无关；空状态欢迎区据此做快捷跳转。

const KEY = 'recents:v1'
const CAP = 6

type RecentMap = Partial<Record<Agent, string[]>>

function load(): RecentMap {
  try {
    const obj = JSON.parse(localStorage.getItem(KEY) ?? '{}')
    return obj && typeof obj === 'object' && !Array.isArray(obj) ? obj : {}
  } catch {
    return {}
  }
}

// 响应式快照：WelcomeView 读它，recordRecent 写后整体替换以触发刷新。
export const recents = ref<RecentMap>(load())

/** 取某 agent 的最近打开项目 dirName 列表（最近在前）。 */
export function getRecents(agent: Agent): string[] {
  return recents.value[agent] ?? []
}

/** 记录一次「打开项目」：dir 提到队首、去重、截断到 CAP。 */
export function recordRecent(agent: Agent, dir: string) {
  const prev = recents.value[agent] ?? []
  const next = [dir, ...prev.filter((d) => d !== dir)].slice(0, CAP)
  recents.value = { ...recents.value, [agent]: next }
  localStorage.setItem(KEY, JSON.stringify(recents.value))
}

/** 从最近打开中删除某条 dir —— Welcome 卡片 hover 时小 × 的入口。 */
export function removeRecent(agent: Agent, dir: string) {
  const prev = recents.value[agent] ?? []
  if (!prev.includes(dir)) return
  const next = prev.filter((d) => d !== dir)
  recents.value = { ...recents.value, [agent]: next }
  localStorage.setItem(KEY, JSON.stringify(recents.value))
}

/** 清空某 agent 的最近打开列表 —— Welcome 区段右侧「清除」按钮。 */
export function clearRecents(agent: Agent) {
  if (!(recents.value[agent]?.length)) return
  const next = { ...recents.value }
  delete next[agent]
  recents.value = next
  localStorage.setItem(KEY, JSON.stringify(recents.value))
}
