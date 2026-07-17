// 全局搜索的共享状态。
//
// 模态盒住在 App.vue 的最外层，但「打开/关闭」的入口在好几个地方：
//   - 应用菜单（未来的）File → Search
//   - 顶栏未来可能加的搜索按钮
//   - ⌘⇧F / Ctrl⇧F 全局快捷键（在 App.vue 的 keydown 监听里翻 open）
// 用一个轻量模块保存 `open` 与最近搜索记录，避免 prop drilling。
//
// 「最近搜索」是会话级的，存在 sessionStorage 以便刷新页面后保留、
// 重启 app 后清空（不像偏好那样要长期保存）。
import { ref } from 'vue'

/** 模态是否可见。`true` 时 App.vue 渲染 <GlobalSearchModal>。 */
export const globalSearchOpen = ref(false)

const RECENT_KEY = 'csv:global-search:recent'
const RECENT_MAX = 6

function loadRecents(): string[] {
  try {
    const raw = sessionStorage.getItem(RECENT_KEY)
    if (!raw) return []
    const arr = JSON.parse(raw)
    return Array.isArray(arr) ? arr.filter((v): v is string => typeof v === 'string').slice(0, RECENT_MAX) : []
  } catch {
    return []
  }
}
function persistRecents(list: string[]) {
  try {
    sessionStorage.setItem(RECENT_KEY, JSON.stringify(list))
  } catch {
    /* sessionStorage 满了 / 被禁；忽略 */
  }
}

/** 最近搜索（最新的在前）；空查询态显示。 */
export const recentSearches = ref<string[]>(loadRecents())

/** 把一次查询写进最近列表；空 / 重复会被去重后置顶。 */
export function pushRecent(q: string) {
  const trimmed = q.trim()
  if (!trimmed) return
  const next = [trimmed, ...recentSearches.value.filter((x) => x !== trimmed)].slice(0, RECENT_MAX)
  recentSearches.value = next
  persistRecents(next)
}

/** 清空最近搜索（菜单里那一个「Clear」按钮的入口）。 */
export function clearRecents() {
  recentSearches.value = []
  persistRecents([])
}

/** 删除某一条最近搜索（hover 时小 × 的入口）。 */
export function removeRecent(q: string) {
  const next = recentSearches.value.filter((x) => x !== q)
  if (next.length === recentSearches.value.length) return
  recentSearches.value = next
  persistRecents(next)
}

/** 打开全局搜索。 */
export function openGlobalSearch() {
  globalSearchOpen.value = true
}

/** 关闭全局搜索。 */
export function closeGlobalSearch() {
  globalSearchOpen.value = false
}
