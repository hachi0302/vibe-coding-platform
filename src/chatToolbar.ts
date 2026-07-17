// 聊天页顶栏与 ChatView 之间的共享状态。
//
// ChatTopbar 住在 App.vue 的顶栏 slot 里、ChatView 住在 main 区域，二者并不共享父
// 模板。这里不引入 store，用一个轻量模块（refs + 注册式 navigator）做胶水：
//   - ChatTopbar 写：toolsCollapsed / search
//   - ChatView 写：searchCount / searchIndex；在 mount 时通过 setSearchNavigator
//     注册一个跳转函数，topbar 点击 ↑/↓ 时调用。
// 切换会话时 App.vue 调 resetChatToolbar() 清掉所有状态。

import { ref } from 'vue'

/** true = 把所有 <details>（tool_use / tool_result / thinking）都折叠；false = 都展开。
 *  默认 true —— 必须跟 DOM 初始状态对齐：`<details>` 渲染时没绑 `:open`，
 *  浏览器默认就是"关闭/折叠"。如果这里默认 false（"已展开"），用户首次点按钮
 *  会先把 ref 翻成 true 触发 sweepDetails(false) 重新关闭一遍已经关闭的 details
 *  ——视觉上没动静，等第二次点才真展开。 */
export const toolsCollapsed = ref(true)

/** 当前搜索关键词；空串表示未搜索 */
export const search = ref('')

/** 搜索范围筛选 —— 让用户在大会话里只搜某一类内容（区分用户输入 / 助手输出 / 工具噪音）：
 *   - all:   不过滤
 *   - user:  只匹配用户消息
 *   - agent: 助手回复 + 助手作出的"文件改动"型工具调用（Write/Edit/MultiEdit/NotebookEdit/apply_patch）
 *   - tools: 其它工具调用（Read/Bash/Grep/…），即"过程性噪音"
 *
 * ChatView 在渲染时通过 data-search-scope 在 .msg-row / tool_use <details> 上打标签，
 * 搜索时 DOM walker 沿祖先链找到最近的 data-search-scope 判断是否计入。 */
export type SearchScope = 'all' | 'user' | 'agent' | 'tools'
export const searchScope = ref<SearchScope>('all')

/** 搜索结果总数 + 当前 1-based 索引（0 表示无匹配）—— 由 ChatView 写入 */
export const searchCount = ref(0)
export const searchIndex = ref(0)

type Navigator = (dir: 1 | -1) => void
let navigator: Navigator | null = null

/** ChatView mount 时注册自己的跳转函数；unmount 时传 null 注销。 */
export function setSearchNavigator(fn: Navigator | null) {
  navigator = fn
}

/** Topbar 调用：1 跳下一个匹配，-1 跳上一个。无 ChatView 时静默忽略。 */
export function navigate(dir: 1 | -1) {
  navigator?.(dir)
}

// 「focus 搜索框」也走相同的注册模式 —— 让原生菜单的 Find in Session…（⌘F）
// 跨过组件边界把焦点推到 ChatTopbar 的 <input>。ChatTopbar mount 时注册自己
// 的 focus + select 函数，unmount 时清掉。
type FocusFn = () => void
let focuser: FocusFn | null = null

/** ChatTopbar mount 时把 focus+select 函数注册进来；unmount 时传 null。 */
export function setSearchFocuser(fn: FocusFn | null) {
  focuser = fn
}

/** 菜单 / 全局快捷键调用：聚焦聊天页搜索框并全选已有内容。 */
export function focusSearchBox() {
  focuser?.()
}

/** 切换会话 / 关闭会话时把所有状态归零。toolsCollapsed 回到 true ——
 *  新会话的 <details> 默认就是关闭的，state 跟着对齐。 */
export function resetChatToolbar() {
  toolsCollapsed.value = true
  search.value = ''
  searchCount.value = 0
  searchIndex.value = 0
  searchScope.value = 'all'
}
