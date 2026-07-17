import {
  isPermissionGranted,
  requestPermission,
  sendNotification,
} from '@tauri-apps/plugin-notification'
import { t } from './i18n'
import type { Agent, Msg } from './types'

const BURST_MS = 1200
const COOLDOWN_MS = 8000
const MAX_BODY_LEN = 160

type PendingNotification = {
  title: string
  body: string
  extraCount: number
  key: string
}

let flushTimer = 0
let nextAllowedAt = 0
let pending: PendingNotification | null = null
let permissionRequest: Promise<boolean> | null = null

function normalizeText(input: string): string {
  return input.replace(/\s+/g, ' ').trim()
}

function clampText(input: string, maxLen = MAX_BODY_LEN): string {
  if (input.length <= maxLen) return input
  return `${input.slice(0, maxLen - 1).trimEnd()}...`
}

export function summarizeLiveAppend(messages: Msg[]): string | null {
  for (let i = messages.length - 1; i >= 0; i -= 1) {
    const msg = messages[i]
    if (msg.role !== 'assistant') continue
    for (let j = msg.blocks.length - 1; j >= 0; j -= 1) {
      const block = msg.blocks[j]
      if (block.kind !== 'text' || !block.text) continue
      const text = clampText(normalizeText(block.text))
      if (text) return text
    }
  }
  return null
}

async function ensurePermission(): Promise<boolean> {
  try {
    if (await isPermissionGranted()) return true
  } catch {
    return false
  }
  if (!permissionRequest) {
    permissionRequest = requestPermission()
      .then((state: string) => state === 'granted')
      .catch(() => false)
      .finally(() => {
        permissionRequest = null
      })
  }
  return permissionRequest!
}

function scheduleFlush() {
  if (!pending) return
  window.clearTimeout(flushTimer)
  const delay = Math.max(BURST_MS, nextAllowedAt - Date.now(), 0)
  flushTimer = window.setTimeout(() => {
    void flushPending()
  }, delay)
}

async function flushPending() {
  flushTimer = 0
  if (!pending) return
  const ready = pending
  pending = null
  if (!(await ensurePermission())) return
  const suffix =
    ready.extraCount > 0 ? t('notify.live.more', { n: ready.extraCount }) : ''
  try {
    sendNotification({
      title: ready.title,
      body: `${ready.body}${suffix}`,
    })
  } catch {
    return
  }
  nextAllowedAt = Date.now() + COOLDOWN_MS
}

export function clearPendingLiveNotification() {
  pending = null
  window.clearTimeout(flushTimer)
  flushTimer = 0
}

export function enqueueLiveNotification(args: {
  agent: Agent
  sessionTitle: string
  sessionPath: string
  messages: Msg[]
  appVisible: boolean
}) {
  if (args.appVisible) {
    clearPendingLiveNotification()
    return
  }
  const body = summarizeLiveAppend(args.messages)
  if (!body) return
  const title = `${t(`stats.scope.${args.agent}`)} · ${args.sessionTitle}`
  if (pending && pending.key === args.sessionPath) {
    pending = {
      ...pending,
      title,
      body,
      extraCount: pending.extraCount + 1,
    }
  } else {
    pending = {
      title,
      body,
      extraCount: 0,
      key: args.sessionPath,
    }
  }
  scheduleFlush()
}

export function resetLiveNotificationsForTests() {
  nextAllowedAt = 0
  clearPendingLiveNotification()
  permissionRequest = null
}
