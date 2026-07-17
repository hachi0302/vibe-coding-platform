import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import {
  viewHistory,
  recordView,
  removeView,
  removeViewEverywhere,
  sortViewHistory,
  persistViewHistory,
  type ViewHistoryEntry,
} from '../src/viewHistory'
import type { SessionMeta } from '../src/types'

function sess(path: string, title = path): SessionMeta {
  return {
    id: path,
    fileName: path,
    path,
    title,
    modified: 0,
    size: 0,
    messageCount: 0,
  } as SessionMeta
}

function entry(over: Partial<ViewHistoryEntry>): ViewHistoryEntry {
  return {
    agent: 'claude',
    dir: '/p',
    session: sess('a'),
    mode: 'read',
    openedAt: 0,
    ...over,
  }
}

let now = 1000
beforeEach(() => {
  localStorage.clear()
  viewHistory.value = []
  now = 1000
  vi.spyOn(Date, 'now').mockImplementation(() => now)
})
afterEach(() => {
  vi.restoreAllMocks()
})

describe('recordView', () => {
  it('appends a new entry with openedAt=now', () => {
    recordView({ agent: 'claude', dir: '/p', session: sess('a.jsonl'), mode: 'read' })
    expect(viewHistory.value).toHaveLength(1)
    expect(viewHistory.value[0]).toMatchObject({
      agent: 'claude',
      dir: '/p',
      openedAt: 1000,
      mode: 'read',
    })
  })

  it('dedups by (agent,dir,path): re-record bumps openedAt + mode, keeps one entry', () => {
    recordView({ agent: 'claude', dir: '/p', session: sess('a.jsonl'), mode: 'read' })
    now = 2000
    recordView({ agent: 'claude', dir: '/p', session: sess('a.jsonl', 'renamed'), mode: 'chat' })
    expect(viewHistory.value).toHaveLength(1)
    expect(viewHistory.value[0].openedAt).toBe(2000)
    expect(viewHistory.value[0].mode).toBe('chat')
    expect(viewHistory.value[0].session.title).toBe('renamed')
  })

  it('treats same path under a different agent/dir as a distinct entry', () => {
    recordView({ agent: 'claude', dir: '/p', session: sess('a.jsonl'), mode: 'read' })
    recordView({ agent: 'codex', dir: '/p', session: sess('a.jsonl'), mode: 'read' })
    recordView({ agent: 'claude', dir: '/q', session: sess('a.jsonl'), mode: 'read' })
    expect(viewHistory.value).toHaveLength(3)
  })

  it('ignores entries without a dir or session path', () => {
    recordView({ agent: 'claude', dir: '', session: sess('a.jsonl'), mode: 'read' })
    recordView({ agent: 'claude', dir: '/p', session: sess(''), mode: 'read' })
    expect(viewHistory.value).toHaveLength(0)
  })
})

describe('removeView', () => {
  it('drops the matching entry only', () => {
    recordView({ agent: 'claude', dir: '/p', session: sess('a.jsonl'), mode: 'read' })
    recordView({ agent: 'claude', dir: '/p', session: sess('b.jsonl'), mode: 'read' })
    removeView('claude', '/p', 'a.jsonl')
    expect(viewHistory.value.map((v) => v.session.path)).toEqual(['b.jsonl'])
  })
})

describe('removeViewEverywhere', () => {
  it('removes all matching entries for the deleted session across dirs/modes', () => {
    recordView({ agent: 'claude', dir: '/p', session: sess('a.jsonl'), mode: 'read' })
    recordView({ agent: 'claude', dir: '/q', session: sess('a.jsonl'), mode: 'chat' })
    recordView({ agent: 'codex', dir: '/p', session: sess('a.jsonl'), mode: 'read' })
    recordView({ agent: 'claude', dir: '/p', session: sess('b.jsonl'), mode: 'read' })
    removeViewEverywhere('claude', 'a.jsonl')
    expect(viewHistory.value.map((v) => `${v.agent}:${v.session.path}`)).toEqual([
      'codex:a.jsonl',
      'claude:b.jsonl',
    ])
  })
})

describe('sortViewHistory', () => {
  it('sorts by openedAt desc', () => {
    const list = [
      entry({ session: sess('r1', 'recent one'), openedAt: 100 }),
      entry({ session: sess('r2', 'recent two'), openedAt: 300 }),
      entry({ session: sess('r3', 'oldest'), openedAt: 50 }),
    ]
    const out = sortViewHistory(list)
    expect(out.map((v) => v.session.path)).toEqual(['r2', 'r1', 'r3'])
  })

  it('filters by case-insensitive title substring', () => {
    const list = [
      entry({ session: sess('a', 'Fix login bug'), openedAt: 300 }),
      entry({ session: sess('b', 'Add usage badges'), openedAt: 200 }),
      entry({ session: sess('c', 'refactor LOGIN flow'), openedAt: 100 }),
    ]
    const out = sortViewHistory(list, 'login')
    expect(out.map((v) => v.session.path)).toEqual(['a', 'c'])
  })

  it('does not mutate the input array', () => {
    const list = [
      entry({ session: sess('r1'), openedAt: 100 }),
      entry({ session: sess('r2'), openedAt: 50 }),
    ]
    const snapshot = list.map((v) => v.session.path)
    sortViewHistory(list)
    expect(list.map((v) => v.session.path)).toEqual(snapshot)
  })
})

describe('persistence', () => {
  it('persistViewHistory writes the reactive list to localStorage', () => {
    recordView({ agent: 'claude', dir: '/p', session: sess('a.jsonl'), mode: 'read' })
    persistViewHistory()
    const raw = localStorage.getItem('viewHistory:v1')
    expect(raw).toBeTruthy()
    const parsed = JSON.parse(raw!)
    expect(parsed).toHaveLength(1)
    expect(parsed[0].session.path).toBe('a.jsonl')
  })

  it('mutations auto-persist (recordView writes through)', () => {
    recordView({ agent: 'claude', dir: '/p', session: sess('a.jsonl'), mode: 'read' })
    expect(JSON.parse(localStorage.getItem('viewHistory:v1')!)).toHaveLength(1)
  })

  it('reloads only valid entries from localStorage on module init', async () => {
    localStorage.setItem(
      'viewHistory:v1',
      JSON.stringify([
        { agent: 'claude', dir: '/p', session: { path: 'ok' }, mode: 'chat', openedAt: 5 },
        { agent: 'claude', dir: '/p', session: {} }, // no path → dropped
        { foo: 'bar' }, // garbage → dropped
      ]),
    )
    vi.resetModules()
    const mod = await import('../src/viewHistory')
    expect(mod.viewHistory.value).toHaveLength(1)
    expect(mod.viewHistory.value[0]).toMatchObject({
      session: { path: 'ok' },
      mode: 'chat',
      openedAt: 5,
    })
  })
})
