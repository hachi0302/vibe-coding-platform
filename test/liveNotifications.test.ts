import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} = vi.hoisted(() => ({
  isPermissionGranted: vi.fn(),
  requestPermission: vi.fn(),
  sendNotification: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-notification', () => ({
  isPermissionGranted,
  requestPermission,
  sendNotification,
}))

import {
  enqueueLiveNotification,
  resetLiveNotificationsForTests,
  summarizeLiveAppend,
} from '../src/liveNotifications'
import type { Msg } from '../src/types'

function msg(role: 'user' | 'assistant', text: string): Msg {
  return {
    role,
    sidechain: false,
    blocks: [{ kind: 'text', text, isError: false }],
  }
}

async function flushTimers(ms = 2000) {
  await vi.advanceTimersByTimeAsync(ms)
  await Promise.resolve()
}

describe('liveNotifications', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    isPermissionGranted.mockReset()
    requestPermission.mockReset()
    sendNotification.mockReset()
    isPermissionGranted.mockResolvedValue(true)
    requestPermission.mockResolvedValue('granted')
    resetLiveNotificationsForTests()
  })

  afterEach(() => {
    resetLiveNotificationsForTests()
    vi.useRealTimers()
  })

  it('summarizes the latest assistant text block', () => {
    expect(
      summarizeLiveAppend([
        msg('user', 'ignore me'),
        { role: 'assistant', sidechain: false, blocks: [{ kind: 'tool_result', isError: false }] },
        msg('assistant', '  final   answer\nwith  spaces  '),
      ]),
    ).toBe('final answer with spaces')
  })

  it('suppresses notifications while the app is visible', async () => {
    enqueueLiveNotification({
      agent: 'codex',
      sessionTitle: 'Island',
      sessionPath: '/tmp/island.jsonl',
      messages: [msg('assistant', 'hello')],
      appVisible: true,
    })
    await flushTimers()
    expect(sendNotification).not.toHaveBeenCalled()
  })

  it('sends a single aggregated notification for a burst', async () => {
    enqueueLiveNotification({
      agent: 'codex',
      sessionTitle: 'Island',
      sessionPath: '/tmp/island.jsonl',
      messages: [msg('assistant', 'first chunk')],
      appVisible: false,
    })
    await vi.advanceTimersByTimeAsync(400)
    enqueueLiveNotification({
      agent: 'codex',
      sessionTitle: 'Island',
      sessionPath: '/tmp/island.jsonl',
      messages: [msg('assistant', 'second chunk')],
      appVisible: false,
    })

    await flushTimers()

    expect(sendNotification).toHaveBeenCalledTimes(1)
    expect(sendNotification).toHaveBeenCalledWith(
      expect.objectContaining({
        title: expect.stringContaining('Island'),
        body: expect.stringContaining('second chunk'),
      }),
    )
    expect(
      (sendNotification.mock.calls[0]?.[0] as { body?: string } | undefined)?.body,
    ).toContain('1')
  })

  it('requests permission when needed', async () => {
    isPermissionGranted.mockResolvedValue(false)

    enqueueLiveNotification({
      agent: 'claude',
      sessionTitle: 'Chat',
      sessionPath: '/tmp/chat.jsonl',
      messages: [msg('assistant', 'permission test')],
      appVisible: false,
    })

    await flushTimers()

    expect(requestPermission).toHaveBeenCalledTimes(1)
    expect(sendNotification).toHaveBeenCalledTimes(1)
  })
})
