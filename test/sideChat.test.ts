import { beforeEach, describe, expect, it, vi } from 'vitest'

// 侧聊现在是「每个分屏格子各持一份」的模型：openSideChat 不再自己 invoke
// `agent_chat_start`，而是委托 chatSessions.startChat / sendPrompt / closeChat，
// 并按 focusedPane 归类。于是这里 mock 掉这两个协作模块，断言委托口径即可。
const { focusedPane, startChatMock, closeChatMock, sendPromptMock } = vi.hoisted(() => ({
  focusedPane: { value: null as { id: number } | null },
  startChatMock: vi.fn(),
  closeChatMock: vi.fn(),
  sendPromptMock: vi.fn(),
}))

vi.mock('../src/panes', () => ({ focusedPane }))

vi.mock('../src/chatSessions', () => ({
  startChat: startChatMock,
  closeChat: closeChatMock,
  sendPrompt: sendPromptMock,
}))

vi.mock('../src/api', () => ({
  purgeBtwSession: vi.fn().mockResolvedValue(undefined),
}))

import { sideChat, openSideChat, closeSideChat } from '../src/sideChat'

describe('btw side chat store', () => {
  beforeEach(() => {
    startChatMock.mockReset()
    closeChatMock.mockReset()
    sendPromptMock.mockReset()
    // 每个用例用**独立的 pane id**起步，于是 perPane 里天然无残留（无需清内部 state）。
    focusedPane.value = null
  })

  it('starts a side chat with bypassPermissions', async () => {
    focusedPane.value = { id: 1 }
    startChatMock.mockResolvedValueOnce({ uiId: 1 })
    await openSideChat({ projectKey: 'proj', cwd: '/tmp' })

    expect(startChatMock).toHaveBeenCalledTimes(1)
    expect(startChatMock.mock.calls[0][0]).toMatchObject({
      agent: 'claude',
      cwd: '/tmp',
      permissionMode: 'bypassPermissions',
    })
    expect(sideChat.value).not.toBeNull()
  })

  it('starts a fresh side chat without sessionId when no fork requested', async () => {
    focusedPane.value = { id: 2 }
    startChatMock.mockResolvedValueOnce({ uiId: 2 })
    await openSideChat({ projectKey: 'proj', cwd: '/tmp' })

    expect(startChatMock.mock.calls[0][0]).not.toHaveProperty('sessionId')
  })

  it('sends the /btw prompt as the first message', async () => {
    focusedPane.value = { id: 3 }
    startChatMock.mockResolvedValueOnce({ uiId: 3 })
    await openSideChat({ projectKey: 'proj', cwd: '/tmp', prompt: 'what does foo do?' })

    // 全新开框时首句走 startChat 的 initialPrompt（不是二次 sendPrompt）。
    expect(startChatMock.mock.calls[0][0]).toMatchObject({ initialPrompt: 'what does foo do?' })
  })

  it('reuses the open panel instead of spawning a second process', async () => {
    focusedPane.value = { id: 4 }
    startChatMock.mockResolvedValueOnce({ uiId: 4 })
    await openSideChat({ projectKey: 'proj', cwd: '/tmp' })

    await openSideChat({ projectKey: 'proj', cwd: '/tmp', prompt: 'follow up' })

    expect(startChatMock).toHaveBeenCalledTimes(1) // 没有再起新子进程
    expect(sendPromptMock).toHaveBeenCalledTimes(1)
    expect(sendPromptMock.mock.calls[0][1]).toBe('follow up')
  })

  it('closeSideChat stops the subprocess and clears the ref', async () => {
    focusedPane.value = { id: 5 }
    startChatMock.mockResolvedValueOnce({ uiId: 5 })
    await openSideChat({ projectKey: 'proj', cwd: '/tmp' })

    closeSideChat()
    expect(sideChat.value).toBeNull()
    expect(closeChatMock).toHaveBeenCalledWith(5)
  })
})
