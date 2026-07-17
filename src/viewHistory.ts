// View 历史 —— 记录每个项目打开过的会话，按 (agent, dir, session) 去重。
// 独立成模块（不依赖 xterm / tauri）以便单测：只用 vue 的 ref + 类型。

import { ref } from 'vue'
import type { Agent, SessionMeta } from './types'

const VIEW_HISTORY_KEY = 'viewHistory:v1'

export interface ViewHistoryEntry {
  agent: Agent
  dir: string
  session: SessionMeta
  mode: 'read' | 'chat'
  openedAt: number
}

export const viewHistory = ref<ViewHistoryEntry[]>(loadViewHistory())

function loadViewHistory(): ViewHistoryEntry[] {
  try {
    const raw = localStorage.getItem(VIEW_HISTORY_KEY)
    if (!raw) return []
    const arr = JSON.parse(raw)
    if (!Array.isArray(arr)) return []
    return arr
      .filter((v: any) => v && v.agent && v.dir && v.session && (v.session.id || v.session.path))
      .map((v: any) => ({
        agent: v.agent,
        dir: v.dir,
        session: v.session,
        mode: v.mode === 'chat' ? 'chat' : 'read',
        openedAt: typeof v.openedAt === 'number' ? v.openedAt : 0,
      }))
  } catch {
    return []
  }
}

export function persistViewHistory() {
  try {
    localStorage.setItem(VIEW_HISTORY_KEY, JSON.stringify(viewHistory.value))
  } catch {}
}

export function viewKey(s: { id?: string; path?: string }): string {
  return s.id || s.path || ''
}

function findViewIndex(agent: Agent, dir: string, key: string): number {
  if (!key) return -1
  return viewHistory.value.findIndex(
    (v) =>
      v.agent === agent &&
      v.dir === dir &&
      (v.session.id === key || v.session.path === key),
  )
}

export function recordView(input: {
  agent: Agent
  dir: string
  session: SessionMeta
  mode: 'read' | 'chat'
}) {
  const key = viewKey(input.session)
  if (!input.dir || !key) return
  const i = findViewIndex(input.agent, input.dir, key)
  const now = Date.now()
  if (i >= 0) {
    const prev = viewHistory.value[i]
    viewHistory.value[i] = {
      ...prev,
      session: {
        ...input.session,
        path: input.session.path || prev.session.path,
      },
      mode: input.mode,
      openedAt: now,
    }
  } else {
    viewHistory.value.push({
      agent: input.agent,
      dir: input.dir,
      session: input.session,
      mode: input.mode,
      openedAt: now,
    })
  }
  persistViewHistory()
}

export function setViewTitle(agent: Agent, key: string, title: string) {
  if (!key) return
  let changed = false
  viewHistory.value = viewHistory.value.map((v) => {
    if (
      v.agent === agent &&
      (v.session.id === key || v.session.path === key) &&
      v.session.title !== title
    ) {
      changed = true
      return { ...v, session: { ...v.session, title } }
    }
    return v
  })
  if (changed) persistViewHistory()
}

export function removeView(agent: Agent, dir: string, key: string) {
  const i = findViewIndex(agent, dir, key)
  if (i < 0) return
  viewHistory.value.splice(i, 1)
  persistViewHistory()
}

export function removeViewEverywhere(agent: Agent, key: string) {
  if (!key) return
  const next = viewHistory.value.filter(
    (v) => !(v.agent === agent && (v.session.id === key || v.session.path === key)),
  )
  if (next.length === viewHistory.value.length) return
  viewHistory.value = next
  persistViewHistory()
}

export function sortViewHistory(list: ViewHistoryEntry[], filter?: string): ViewHistoryEntry[] {
  const q = (filter ?? '').trim().toLowerCase()
  const filtered = q
    ? list.filter((v) => (v.session.title ?? '').toLowerCase().includes(q))
    : list.slice()
  return filtered.sort((a, b) => b.openedAt - a.openedAt)
}
