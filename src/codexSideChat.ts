// Codex `/side` 侧聊 —— 与 Claude Code 的 `/btw` 独立建模。
//
// Codex app-server 原生提供 `thread/fork` + `ephemeral: true`：有主 thread 时从它 fork，
// 没有时创建一个同样 ephemeral 的新 thread。两种路径都不会 materialize 到 session history，
// 因此关闭时只需停止 app-server，不要复用 Claude btw 的磁盘 purge 逻辑。

import { computed, shallowRef } from 'vue'
import { startChat, closeChat, sendPrompt, type ChatSession } from './chatSessions'
import { focusedPane } from './panes'

/** 每个分屏格子各持有一个 Codex `/side` 会话。 */
const perPane = shallowRef(new Map<number, ChatSession>())

/** 当前聚焦格子的 Codex side，会由 CodexSidePanel 渲染。 */
export const codexSideChat = computed<ChatSession | null>(() => {
  const pane = focusedPane.value
  return pane ? perPane.value.get(pane.id) ?? null : null
})

/** 供会话列表过滤使用。ephemeral thread 正常不会出现，保留防御性过滤。 */
export function activeCodexSideSessionIds(): Set<string> {
  const ids = new Set<string>()
  for (const session of perPane.value.values()) {
    if (session.sessionId) ids.add(session.sessionId)
  }
  return ids
}

/** 每个 Codex side 的最小化状态；不会与 Claude btw 共用本地状态。 */
const minimizedState = new Map<number, boolean>()
export function isCodexSideMinimized(uiId: number): boolean {
  return minimizedState.get(uiId) ?? false
}
export function setCodexSideMinimized(uiId: number, minimized: boolean): void {
  minimizedState.set(uiId, minimized)
}

export interface OpenCodexSideChatOptions {
  projectKey: string
  cwd: string
  /** 主 Codex thread id；提供后通过 app-server `thread/fork` 继承当前上下文。 */
  forkThreadId?: string
  model?: string
  effort?: string
  permissionMode?: string
  /** `/side prompt` 的首句。 */
  prompt?: string
  title?: string
}

function attach(paneId: number, session: ChatSession): void {
  const next = new Map(perPane.value)
  next.set(paneId, session)
  perPane.value = next
}

/**
 * 打开或复用当前分屏的 Codex `/side`。
 *
 * 使用 `forkSessionId` 而非普通 `sessionId`，避免前端在 app-server 返回新 ephemeral thread id
 * 前把主聊天的 id 误认为侧聊 id。
 */
export async function openCodexSideChat(
  opts: OpenCodexSideChatOptions,
): Promise<ChatSession | null> {
  const pane = focusedPane.value
  if (!pane) return null

  const existing = perPane.value.get(pane.id)
  if (existing) {
    if (opts.prompt) void sendPrompt(existing, opts.prompt)
    return existing
  }

  const paneId = pane.id
  const session = await startChat({
    agent: 'codex',
    projectKey: opts.projectKey,
    cwd: opts.cwd,
    title: opts.title ?? 'side',
    permissionMode: opts.permissionMode,
    model: opts.model,
    effort: opts.effort,
    ...(opts.forkThreadId ? { forkSessionId: opts.forkThreadId } : {}),
    fork: !!opts.forkThreadId,
    ephemeral: true,
    initialPrompt: opts.prompt,
    onReady: (created) => attach(paneId, created),
  })

  return session
}

/** 关闭当前聚焦格子的 Codex `/side`。ephemeral thread 不需要磁盘清理。 */
export function closeCodexSideChat(): void {
  const pane = focusedPane.value
  if (!pane) return

  const session = perPane.value.get(pane.id)
  if (!session) return

  const next = new Map(perPane.value)
  next.delete(pane.id)
  perPane.value = next
  minimizedState.delete(session.uiId)
  void closeChat(session.uiId)
}

/** 切项目、切 agent 或关闭主视图时关闭全部 Codex side。 */
export function closeAllCodexSideChats(): void {
  const all = perPane.value
  perPane.value = new Map()
  for (const session of all.values()) {
    minimizedState.delete(session.uiId)
    void closeChat(session.uiId)
  }
}
