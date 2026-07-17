// 回收站工具栏与 TrashView 之间的共享状态。
//
// TrashTopbar 住在 App.vue 的顶栏 slot 里、TrashView 住在 main 区域，二者并不
// 共享父模板。与 chatToolbar.ts 同理，这里用一个轻量模块（refs + 纯函数）做胶水：
//   - TrashTopbar 写：trashSearch / trashSort / trashProject / selectMode / selectedTrash
//   - TrashView 读 filterTrash() 拿到过滤+排序后的列表，并在 selectMode 下渲染勾选框
// 切换进回收站时 App.vue 调 resetTrashToolbar() 清状态。

import { ref } from 'vue'
import type { TrashItem } from './types'

export type TrashSort = 'recent' | 'oldest'

/** 搜索关键词：匹配标题 + 项目名，空串表示未搜索。 */
export const trashSearch = ref('')
/** 按删除时间排序：最近删除在前 / 最早删除在前。 */
export const trashSort = ref<TrashSort>('recent')
/** 项目筛选：'all' 或某个具体的 projectLabel。 */
export const trashProject = ref<string>('all')
/** 批量选择模式开关。 */
export const selectMode = ref(false)
/** 已勾选的 trashFile 集合（仅 selectMode 下有意义）。 */
export const selectedTrash = ref<Set<string>>(new Set())

/** 勾选 / 取消勾选一个回收站条目。整体替换 Set 以触发响应式更新。 */
export function toggleTrashSelected(trashFile: string) {
  const next = new Set(selectedTrash.value)
  if (next.has(trashFile)) next.delete(trashFile)
  else next.add(trashFile)
  selectedTrash.value = next
}

/** 退出批量模式：关掉 selectMode 并清空选择。 */
export function exitSelectMode() {
  selectMode.value = false
  selectedTrash.value = new Set()
}

/** 切换进 / 离开回收站时把所有工具栏状态归零。 */
export function resetTrashToolbar() {
  trashSearch.value = ''
  trashSort.value = 'recent'
  trashProject.value = 'all'
  selectMode.value = false
  selectedTrash.value = new Set()
}

/** 回收站里出现过的项目名（去重 + 排序），用于项目筛选下拉。 */
export function trashProjects(items: TrashItem[]): string[] {
  const seen = new Set<string>()
  for (const it of items) {
    const label = it.projectLabel.trim()
    if (label) seen.add(label)
  }
  return [...seen].sort((a, b) => a.localeCompare(b))
}

/** 应用搜索 + 项目筛选 + 时间排序。读取模块 refs，故在 computed 里调用即响应式；
 *  返回新数组，不改动入参。 */
export function filterTrash(items: TrashItem[]): TrashItem[] {
  const q = trashSearch.value.trim().toLowerCase()
  const proj = trashProject.value
  const out = items.filter((it) => {
    if (proj !== 'all' && it.projectLabel !== proj) return false
    if (q && !`${it.title} ${it.projectLabel}`.toLowerCase().includes(q)) return false
    return true
  })
  out.sort((a, b) =>
    trashSort.value === 'recent'
      ? b.deletedAt - a.deletedAt
      : a.deletedAt - b.deletedAt,
  )
  return out
}
