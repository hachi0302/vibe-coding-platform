// TUI 搜索：顶栏搜索框 ↔ 聚焦格子的活跃终端 tab 之间的胶水。
//
// 架构与 chatToolbar.ts 平行：
//   - TuiTopbar 写 search；读 searchCount / searchIndex
//   - 本模块自己运行搜索引擎（扫描 xterm buffer），不需要外部 view 注册
//   - 切 tab / 切 pane 时自动重跑搜索

import { ref, watch } from 'vue'
import { activeUiId } from './panes'
import { tabs } from './terminals'
import type { Terminal } from '@xterm/xterm'

export const tuiSearch = ref('')
export const tuiSearchCount = ref(0)
export const tuiSearchIndex = ref(0)

type FocusFn = () => void
let focuser: FocusFn | null = null
export function setTuiSearchFocuser(fn: FocusFn | null) { focuser = fn }
export function focusTuiSearchBox() { focuser?.() }

interface Match { row: number; col: number; len: number }
let matches: Match[] = []

function activeTerm(): Terminal | null {
  const uid = activeUiId.value
  if (uid == null) return null
  return tabs.value.find(t => t.uiId === uid)?.term ?? null
}

function runSearch() {
  matches = []
  tuiSearchCount.value = 0
  tuiSearchIndex.value = 0

  const term = activeTerm()
  const q = tuiSearch.value
  if (!term || !q) {
    term?.clearSelection()
    return
  }

  const buf = term.buffer.active
  const lower = q.toLowerCase()
  for (let y = 0; y < buf.length; y++) {
    const line = buf.getLine(y)
    if (!line) continue
    const text = line.translateToString().toLowerCase()
    let start = 0
    for (;;) {
      const idx = text.indexOf(lower, start)
      if (idx === -1) break
      matches.push({ row: y, col: idx, len: q.length })
      start = idx + 1
    }
  }

  tuiSearchCount.value = matches.length
  if (matches.length > 0) {
    // 默认跳到视口内最近的匹配（或最后一个）
    const viewTop = buf.viewportY
    const viewBot = viewTop + term.rows
    let best = matches.length - 1
    for (let i = 0; i < matches.length; i++) {
      if (matches[i].row >= viewTop) {
        best = i
        break
      }
    }
    if (matches[best].row > viewBot && best > 0) best = matches.length - 1
    tuiSearchIndex.value = best + 1
    selectCurrent(term)
  } else {
    term.clearSelection()
  }
}

function selectCurrent(term: Terminal) {
  const m = matches[tuiSearchIndex.value - 1]
  if (!m) return
  term.select(m.col, m.row, m.len)
  const viewTop = term.buffer.active.viewportY
  const viewBot = viewTop + term.rows
  if (m.row < viewTop || m.row >= viewBot) {
    term.scrollToLine(Math.max(0, m.row - Math.floor(term.rows / 3)))
  }
}

export function tuiNavigate(dir: 1 | -1) {
  if (matches.length === 0) return
  let idx = tuiSearchIndex.value - 1 + dir
  if (idx < 0) idx = matches.length - 1
  if (idx >= matches.length) idx = 0
  tuiSearchIndex.value = idx + 1
  const term = activeTerm()
  if (term) selectCurrent(term)
}

export function resetTuiToolbar() {
  tuiSearch.value = ''
  tuiSearchCount.value = 0
  tuiSearchIndex.value = 0
  matches = []
}

watch(tuiSearch, runSearch)
watch(activeUiId, () => {
  if (tuiSearch.value) runSearch()
})
