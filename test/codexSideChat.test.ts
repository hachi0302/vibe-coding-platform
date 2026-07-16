import { beforeEach, describe, expect, it, vi } from 'vitest'

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

import {
  closeCodexSideChat,
  codexSideChat,
  openCodexSideChat,
} from '../src/codexSideChat'

function sideSession(uiId: number) {
  return { uiId, sessionId: '' }
}

function startWith(session: ReturnType<typeof sideSession>) {
  startChatMock.mockImplementationOnce(async (opts: { onReady?: (s: typeof session) => void }) => {
    opts.onReady?.(session)
    return session
  })
}

describe('Codex /side store', () => {
  beforeEach(() => {
    startChatMock.mockReset()
    closeChatMock.mockReset()
    sendPromptMock.mockReset()
    focusedPane.value = null
  })

  it('starts an ephemeral Codex fork instead of a Claude btw session', async () => {
    focusedPane.value = { id: 101 }
    startWith(sideSession(1))

    await openCodexSideChat({
      projectKey: 'proj',
      cwd: '/tmp',
      forkThreadId: 'thread-1',
      permissionMode: 'approve',
      model: 'gpt-5.4',
      effort: 'high',
    })

    expect(startChatMock).toHaveBeenCalledWith(
      expect.objectContaining({
        agent: 'codex',
        title: 'side',
        forkSessionId: 'thread-1',
        fork: true,
        ephemeral: true,
        permissionMode: 'approve',
      }),
    )
    expect(codexSideChat.value).not.toBeNull()
  })

  it('starts a fresh ephemeral thread when the main Codex chat has no thread id yet', async () => {
    focusedPane.value = { id: 102 }
    startWith(sideSession(2))

    await openCodexSideChat({ projectKey: 'proj', cwd: '/tmp' })

    expect(startChatMock).toHaveBeenCalledWith(
      expect.objectContaining({ fork: false, ephemeral: true }),
    )
    expect(startChatMock.mock.calls[0][0]).not.toHaveProperty('forkSessionId', undefined)
  })

  it('sends a /side prompt as the first message', async () => {
    focusedPane.value = { id: 103 }
    startWith(sideSession(3))

    await openCodexSideChat({
      projectKey: 'proj',
      cwd: '/tmp',
      prompt: 'inspect the current diff',
    })

    expect(startChatMock).toHaveBeenCalledWith(
      expect.objectContaining({ initialPrompt: 'inspect the current diff' }),
    )
  })

  it('reuses the side thread in the same pane and closes it independently', async () => {
    focusedPane.value = { id: 104 }
    startWith(sideSession(4))
    await openCodexSideChat({ projectKey: 'proj', cwd: '/tmp' })

    await openCodexSideChat({ projectKey: 'proj', cwd: '/tmp', prompt: 'follow up' })
    expect(startChatMock).toHaveBeenCalledTimes(1)
    expect(sendPromptMock).toHaveBeenCalledWith(expect.anything(), 'follow up')

    closeCodexSideChat()
    expect(codexSideChat.value).toBeNull()
    expect(closeChatMock).toHaveBeenCalledWith(4)
  })
})
