// 「btw」侧聊浮框 —— 每个分屏格子各自持有一份独立的侧聊会话。
//
// 语义取自 Claude Code 官方对 `/btw` 的定位：一个**临时**的旁支问答，能看见当前对话
// 的上下文，但答完即走、不进主历史。本 app 是「外部进程」模型（每个 chat 是独立的
// stream-json 子进程），无法像 TUI 那样共享内存里的对话，于是用 `--fork-session`
// 从主聊**派生**一份独立会话来还原「继承上下文却不污染」这一灵魂：
//   · 主聊有 sessionId（已落盘）→ --resume + --fork-session：继承上下文，写到新文件；
//   · 没有（如全新主聊还没出首个 result）→ 退化为同目录下的一个全新 Claude 会话。
// 关掉浮框 = 停子进程 + purge fork 文件（不留痕迹）。
//
// 进程模型复用 chatSessions：侧聊本身就是一个普通 ChatSession，被 `startChat` 推进
// `chatSessions.value`（于是模块时钟、事件路由、closeChat 全都直接复用），只是另由这里
// 的 map 按 paneId 持有、与主视图的 `liveChat` 互不干扰。

import { computed, shallowRef } from 'vue'
import { startChat, closeChat, sendPrompt, type ChatSession } from './chatSessions'
import * as api from './api'
import { focusedPane } from './panes'

/** 每个分屏格子的 btw 侧聊会话；null = 该格子未开侧聊。 */
const perPane = shallowRef(new Map<number, ChatSession>())

/** 当前聚焦格子的 btw 侧聊会话（给 ChatSidePanel 渲染用）。 */
export const sideChat = computed<ChatSession | null>(() => {
  const fp = focusedPane.value
  if (!fp) return null
  return perPane.value.get(fp.id) ?? null
})

/** 按 pane id 取侧聊（给 PaneContent 内部渲染用）。 */
export function sideChatForPane(paneId: number): ChatSession | null {
  return perPane.value.get(paneId) ?? null
}

/** 每个 btw 的最小化状态（组件重建后恢复用）。key = ChatSession.uiId。 */
const minimizedState = new Map<number, boolean>()
export function isBtwMinimized(uiId: number): boolean { return minimizedState.get(uiId) ?? false }
export function setBtwMinimized(uiId: number, v: boolean) { minimizedState.set(uiId, v) }

/** 所有活跃 btw 侧聊的 session ID 集合（列表过滤用，不在会话列表里显示）。 */
export function activeBtwSessionIds(): Set<string> {
  const ids = new Set<string>()
  for (const s of perPane.value.values()) {
    if (s.sessionId) ids.add(s.sessionId)
  }
  return ids
}

export interface OpenSideChatOptions {
  /** 侧聊所属项目 key（= ProjectInfo.dirName），仅用于归类/标题。 */
  projectKey: string
  /** 工作目录 —— 侧聊子进程的 cwd，必须存在。 */
  cwd: string
  /** 主聊的 session id：非空则 fork 继承其上下文；空/缺省则全新会话。 */
  forkSessionId?: string
  /** 沿用主聊的模型（保持口径一致）；缺省走 CLI 默认。 */
  model?: string
  effort?: string
  /** `/btw 你的提示词` 直接带词：开框即发这一句。 */
  prompt?: string
  title?: string
}

/**
 * 打开（或复用）当前聚焦格子 的 btw 侧聊浮框。已开则不重开子进程：带词就把这句发进
 * 现有侧聊，否则只是把焦点交还给已存在的浮框。返回当前侧聊会话。
 */
export async function openSideChat(opts: OpenSideChatOptions): Promise<ChatSession | null> {
  const fp = focusedPane.value
  if (!fp) return null

  const existing = perPane.value.get(fp.id)
  if (existing) {
    if (opts.prompt) void sendPrompt(existing, opts.prompt)
    return existing
  }

  const session = await startChat({
    agent: 'claude',
    projectKey: opts.projectKey,
    cwd: opts.cwd,
    title: opts.title ?? 'btw',
    permissionMode: 'bypassPermissions',
    model: opts.model,
    effort: opts.effort,
    initialPrompt: opts.prompt,
  })

  const next = new Map(perPane.value)
  next.set(fp.id, session)
  perPane.value = next

  return session
}

/** 关闭当前聚焦格子的 btw 侧聊：停子进程 + 清理 CLI 产生的会话文件。 */
export function closeSideChat(): void {
  const fp = focusedPane.value
  if (!fp) return

  const s = perPane.value.get(fp.id)
  if (!s) return

  const next = new Map(perPane.value)
  next.delete(fp.id)
  perPane.value = next

  void closeChat(s.uiId)
  if (s.sessionId && s.projectKey) {
    api.purgeBtwSession(s.projectKey, s.sessionId).catch(() => {})
  }
}

/**
 * 关闭**所有** pane 的 btw 侧聊。切 agent / 切项目时调用 —— 此时整个视图已换，
 * 旧 side chat 不再有意义。
 */
export function closeAllSideChats(): void {
  const all = perPane.value
  perPane.value = new Map()
  for (const s of all.values()) {
    void closeChat(s.uiId)
    if (s.sessionId && s.projectKey) {
      api.purgeBtwSession(s.projectKey, s.sessionId).catch(() => {})
    }
  }
}
