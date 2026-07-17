import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const { invokeMock } = vi.hoisted(() => ({
  invokeMock: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({
  invoke: invokeMock,
}))

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}))

import {
  chatEffectiveEffortForTest,
  enqueuePrompt,
  interruptChat,
  parseRetryLine,
  reconnectChats,
  removeQueued,
  respondPermission,
  respondQuestion,
  startChat,
} from '../src/chatSessions'
import type { ChatPermissionRequest, ChatQuestionRequest } from '../src/types'

afterEach(() => {
  vi.useRealTimers()
})

describe('chatSessions Claude API-key compatibility', () => {
  beforeEach(() => {
    invokeMock.mockReset()
  })

  it('drops Claude effort for API-key sessions', () => {
    expect(
      chatEffectiveEffortForTest({
        agent: 'claude',
        model: 'claude-opus-4-8',
        effort: 'high',
        apiKeySource: 'ANTHROPIC_API_KEY',
      }),
    ).toBeUndefined()
  })

  it('keeps Claude effort for subscription sessions', () => {
    expect(
      chatEffectiveEffortForTest({
        agent: 'claude',
        model: 'claude-opus-4-8',
        effort: 'high',
        apiKeySource: 'none',
      }),
    ).toBe('high')
  })

  it('starts Claude chat without forcing a default model or effort', async () => {
    invokeMock.mockResolvedValueOnce({ chatId: 1, processModel: 'longLivedStdin' })
    const s = await startChat({
      agent: 'claude',
      projectKey: 'proj',
      cwd: '/tmp',
      title: 'Chat',
    })
    expect(s.model).toBeUndefined()
    expect(s.effort).toBeUndefined()
    expect(invokeMock).toHaveBeenCalledWith(
      'agent_chat_start',
      expect.objectContaining({
        agent: 'claude',
        model: undefined,
        effort: undefined,
      }),
    )
  })

  it('interrupts the current Claude turn by restarting the long-lived process with resume', async () => {
    invokeMock.mockResolvedValueOnce(undefined)
    invokeMock.mockResolvedValueOnce({ chatId: 8, processModel: 'longLivedStdin' })
    const session = {
      chatId: 7,
      agent: 'claude',
      cwd: '/tmp',
      sessionId: 'sess-1',
      permissionMode: 'acceptEdits',
      model: undefined,
      effort: undefined,
      apiKeySource: 'none',
      processModel: 'longLivedStdin',
      applied: { permissionMode: 'acceptEdits', model: undefined, effort: undefined },
      status: 'running',
      turnState: 'running',
      turnStartedAt: Date.now(),
      lastTurnMs: 0,
      msgs: [],
      queue: [],
      live: { kind: 'text', text: 'hello' },
      pendingPermissions: [],
      pendingQuestions: [],
    } as any
    await interruptChat(session)
    expect(invokeMock).toHaveBeenNthCalledWith(1, 'agent_chat_stop', { id: 7 })
    expect(invokeMock).toHaveBeenNthCalledWith(2, 'agent_chat_start', {
      agent: 'claude',
      cwd: '/tmp',
      sessionId: 'sess-1',
      permissionMode: 'acceptEdits',
      model: undefined,
      effort: undefined,
      fork: undefined,
      useReclaude: false,
    })
    expect(session.chatId).toBe(8)
    expect(session.status).toBe('running')
    expect(session.turnState).toBe('idle')
    expect(session.live).toBeNull()
    expect(session.msgs).toHaveLength(1)
    expect(session.msgs[0].role).toBe('user')
    expect(session.msgs[0].blocks[0].text).toBe('[Request interrupted by user]')
  })
})

describe('reconnectChats — restored live messages', () => {
  beforeEach(() => {
    invokeMock.mockReset()
  })

  it('normalizes missing timestamps and assistant model labels', async () => {
    vi.useFakeTimers()
    vi.setSystemTime(new Date('2026-07-09T12:58:00.000Z'))
    invokeMock.mockResolvedValueOnce([
      {
        chatId: 77,
        agent: 'codex',
        projectKey: 'proj',
        cwd: '/tmp',
        sessionId: 'thread-1',
        messages: [
          {
            role: 'assistant',
            sidechain: false,
            timestamp: null,
            blocks: [{ kind: 'text', text: 'hi', isError: false }],
          },
        ],
        turnState: 'idle',
        turnStartedAtMs: null,
        permissionMode: 'approve',
        model: 'gpt-5.4',
        effort: 'high',
        processModel: 'codexAppServer',
      },
    ])

    const [session] = await reconnectChats()

    expect(session.msgs[0].timestamp).toBe('2026-07-09T12:58:00.000Z')
    expect(session.msgs[0].model).toBe('gpt-5.4')
    expect(session.lastModel).toBe('gpt-5.4')
  })
})

describe('parseRetryLine — network-retry detection from CLI stderr', () => {
  it('extracts attempt/max from "(N/M)" form', () => {
    expect(parseRetryLine('Request failed · retrying (4/10) · 24s')).toEqual({ attempt: 4, max: 10 })
  })

  it('extracts attempt/max from "N of M" form', () => {
    expect(parseRetryLine('API error, retrying 2 of 5...')).toEqual({ attempt: 2, max: 5 })
  })

  it('matches transient-error keywords without a count → empty object', () => {
    expect(parseRetryLine('Overloaded, backing off')).toEqual({})
    expect(parseRetryLine('fetch failed: ECONNRESET')).toEqual({})
    expect(parseRetryLine('socket hang up')).toEqual({})
  })

  it('is case-insensitive', () => {
    expect(parseRetryLine('RETRYING request')).toEqual({})
  })

  it('returns null for unrelated stderr lines', () => {
    expect(parseRetryLine('[debug] loaded 3 of 4 plugins')).toBeNull()
    expect(parseRetryLine('Reading config from ~/.claude')).toBeNull()
    expect(parseRetryLine('')).toBeNull()
  })
})

describe('respondPermission — interactive tool-permission reply', () => {
  beforeEach(() => {
    invokeMock.mockReset()
  })

  const permReq = (over: Partial<ChatPermissionRequest> = {}): ChatPermissionRequest => ({
    requestId: 'req-1',
    toolName: 'Bash',
    input: { command: 'ls' },
    ...over,
  })

  it('writes the decision back to the matching chat and dequeues the request', async () => {
    invokeMock.mockResolvedValueOnce({ chatId: 42, processModel: 'longLivedStdin' })
    const s = await startChat({ agent: 'claude', projectKey: 'p', cwd: '/tmp', title: 'C' })
    const r = permReq()
    s.pendingPermissions = [r]
    invokeMock.mockReset()
    invokeMock.mockResolvedValueOnce(undefined)

    await respondPermission(s, r, 'allow-once')

    expect(s.pendingPermissions).toHaveLength(0)
    expect(invokeMock).toHaveBeenCalledWith('agent_chat_respond_permission', {
      id: 42,
      requestId: 'req-1',
      decision: { behavior: 'allow', updatedInput: { command: 'ls' } },
    })
  })

  it('dequeues only the answered request, leaving others pending', async () => {
    invokeMock.mockResolvedValueOnce({ chatId: 7, processModel: 'longLivedStdin' })
    const s = await startChat({ agent: 'claude', projectKey: 'p', cwd: '/tmp', title: 'C' })
    const a = permReq({ requestId: 'a' })
    const b = permReq({ requestId: 'b' })
    s.pendingPermissions = [a, b]
    invokeMock.mockReset()
    invokeMock.mockResolvedValueOnce(undefined)

    await respondPermission(s, a, 'deny')

    expect(s.pendingPermissions.map((p) => p.requestId)).toEqual(['b'])
  })
})

describe('respondQuestion — structured AskUserQuestion reply', () => {
  beforeEach(() => {
    invokeMock.mockReset()
  })

  const qReq = (over: Partial<ChatQuestionRequest> = {}): ChatQuestionRequest => ({
    requestId: 'q-1',
    questions: [{ question: 'Pick one', options: [{ label: 'A' }, { label: 'B' }] }],
    ...over,
  })

  it('writes an allow decision with the answers map and dequeues the question', async () => {
    invokeMock.mockResolvedValueOnce({ chatId: 42, processModel: 'longLivedStdin' })
    const s = await startChat({ agent: 'claude', projectKey: 'p', cwd: '/tmp', title: 'C' })
    const r = qReq()
    s.pendingQuestions = [r]
    invokeMock.mockReset()
    invokeMock.mockResolvedValueOnce(undefined)

    await respondQuestion(s, r, [{ labels: ['B'] }])

    expect(s.pendingQuestions).toHaveLength(0)
    expect(invokeMock).toHaveBeenCalledWith('agent_chat_respond_question', {
      id: 42,
      requestId: 'q-1',
      decision: {
        behavior: 'allow',
        updatedInput: { questions: r.questions, answers: { 'Pick one': 'B' } },
      },
    })
  })

  it('writes a deny decision when the user cancels (null selections)', async () => {
    invokeMock.mockResolvedValueOnce({ chatId: 9, processModel: 'longLivedStdin' })
    const s = await startChat({ agent: 'claude', projectKey: 'p', cwd: '/tmp', title: 'C' })
    const r = qReq()
    s.pendingQuestions = [r]
    invokeMock.mockReset()
    invokeMock.mockResolvedValueOnce(undefined)

    await respondQuestion(s, r, null)

    expect(s.pendingQuestions).toHaveLength(0)
    expect(invokeMock).toHaveBeenCalledWith('agent_chat_respond_question', {
      id: 9,
      requestId: 'q-1',
      decision: { behavior: 'deny', message: 'The user declined to answer the question.', interrupt: false },
    })
  })

  it('dequeues only the answered question, leaving others pending', async () => {
    invokeMock.mockResolvedValueOnce({ chatId: 7, processModel: 'longLivedStdin' })
    const s = await startChat({ agent: 'claude', projectKey: 'p', cwd: '/tmp', title: 'C' })
    const a = qReq({ requestId: 'a' })
    const b = qReq({ requestId: 'b' })
    s.pendingQuestions = [a, b]
    invokeMock.mockReset()
    invokeMock.mockResolvedValueOnce(undefined)

    await respondQuestion(s, a, [{ labels: ['A'] }])

    expect(s.pendingQuestions.map((q) => q.requestId)).toEqual(['b'])
  })
})

describe('chatSessions message queue (type-while-running)', () => {
  beforeEach(() => {
    invokeMock.mockReset()
  })

  // 起一个空闲的 Claude 长驻会话；之后的 invoke（send / stop）一律 resolve。
  async function startClaude() {
    invokeMock.mockResolvedValueOnce({ chatId: 1, processModel: 'longLivedStdin' })
    const s = await startChat({ agent: 'claude', projectKey: 'p', cwd: '/tmp', title: 'C' })
    invokeMock.mockResolvedValue(undefined)
    return s
  }

  it('sends immediately when idle and the queue is empty', async () => {
    const s = await startClaude()
    enqueuePrompt(s, 'hello')
    await Promise.resolve()
    expect(s.queue).toHaveLength(0)
    expect(s.turnState).toBe('running')
    expect(invokeMock).toHaveBeenCalledWith(
      'agent_chat_send',
      expect.objectContaining({ id: 1, text: 'hello' }),
    )
  })

  it('queues instead of sending while a turn is running, preserving FIFO order and attachments', async () => {
    const s = await startClaude()
    s.turnState = 'running' // 模拟一轮进行中
    enqueuePrompt(s, 'first')
    enqueuePrompt(s, 'second', [{ dataUrl: 'd', mediaType: 'image/png', data: 'x' }] as never)
    await Promise.resolve()
    expect(s.queue.map((q) => q.text)).toEqual(['first', 'second'])
    expect(s.queue[1].images).toHaveLength(1)
    expect(invokeMock).not.toHaveBeenCalledWith('agent_chat_send', expect.anything())
  })

  it('removeQueued drops a pending message by id', async () => {
    const s = await startClaude()
    s.turnState = 'running'
    enqueuePrompt(s, 'a')
    enqueuePrompt(s, 'b')
    removeQueued(s, s.queue[0].id)
    expect(s.queue.map((q) => q.text)).toEqual(['b'])
  })

  it('ignores empty messages (no text / images / files)', async () => {
    const s = await startClaude()
    s.turnState = 'running'
    enqueuePrompt(s, '   ')
    expect(s.queue).toHaveLength(0)
  })

  it('does not queue or send once the session has ended', async () => {
    const s = await startClaude()
    s.status = 'exited'
    enqueuePrompt(s, 'hello')
    expect(s.queue).toHaveLength(0)
    expect(invokeMock).not.toHaveBeenCalledWith('agent_chat_send', expect.anything())
  })

  it('preserves the queue when the current turn is interrupted and drains next', async () => {
    const s = await startClaude()
    s.turnState = 'running'
    enqueuePrompt(s, 'pending-1')
    enqueuePrompt(s, 'pending-2')
    expect(s.queue).toHaveLength(2)
    // 中断（长驻 = stop + restart）：先 stop 旧进程，再 start 新进程。
    invokeMock.mockReset()
    invokeMock.mockResolvedValueOnce(undefined) // stop
    invokeMock.mockResolvedValueOnce({ chatId: 2, processModel: 'longLivedStdin' }) // start
    invokeMock.mockResolvedValue(undefined) // drain sends
    await interruptChat(s)
    // pending-1 被 drain 发出，pending-2 还在队列
    expect(s.queue).toHaveLength(1)
    expect(s.queue[0].text).toBe('pending-2')
  })
})
