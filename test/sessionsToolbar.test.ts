import { afterEach, beforeEach, describe, expect, it } from 'vitest'
import {
  filterSessions,
  resetSessionsToolbar,
  sessionSearch,
  sessionSort,
  sessionsFilterActive,
} from '../src/sessionsToolbar'
import type { SessionMeta } from '../src/types'

const session = (over: Partial<SessionMeta> & { path: string }): SessionMeta => ({
  id: 'sess-abcdef12',
  fileName: 's.jsonl',
  title: 'A session',
  modified: 0,
  size: 100,
  messageCount: 3,
  ...over,
})

// sessionsToolbar holds module-level state; reset it around every test.
beforeEach(() => resetSessionsToolbar())
afterEach(() => resetSessionsToolbar())

describe('filterSessions', () => {
  const items = [
    session({ path: 'a', title: 'Refactor parser', id: 'id-a', modified: 300, size: 50, messageCount: 9 }),
    session({ path: 'b', title: 'Fix login bug', id: '', modified: 100, size: 300, messageCount: 1 }),
    session({ path: 'c', title: 'Add tests', id: 'id-c', modified: 200, size: 150, messageCount: 5 }),
  ]

  it('returns every item, newest first, with no filters', () => {
    expect(filterSessions(items).map((s) => s.path)).toEqual(['a', 'c', 'b'])
  })

  it('sorts oldest first', () => {
    sessionSort.value = 'oldest'
    expect(filterSessions(items).map((s) => s.path)).toEqual(['b', 'c', 'a'])
  })

  it('sorts by size, largest first', () => {
    sessionSort.value = 'size'
    expect(filterSessions(items).map((s) => s.path)).toEqual(['b', 'c', 'a'])
  })

  it('sorts by message count, most first', () => {
    sessionSort.value = 'messages'
    expect(filterSessions(items).map((s) => s.path)).toEqual(['a', 'c', 'b'])
  })

  it('breaks size ties by newest modified', () => {
    const tied = [
      session({ path: 'old', size: 100, modified: 10 }),
      session({ path: 'new', size: 100, modified: 20 }),
    ]
    sessionSort.value = 'size'
    expect(filterSessions(tied).map((s) => s.path)).toEqual(['new', 'old'])
  })

  // 关键词搜索现在走后端（searchSessions），filterSessions 不再做 title/id 文本匹配 ——
  // 留 sessionSearch 这个开关只为让 sessionsFilterActive 触发 App.vue 一次性加载整个项目。
  it('ignores sessionSearch — text matching moved to the backend', () => {
    sessionSearch.value = 'LOGIN'
    expect(filterSessions(items).map((s) => s.path)).toEqual(['a', 'c', 'b'])
  })

  it('does not mutate the input array', () => {
    sessionSort.value = 'oldest'
    const input = [...items]
    filterSessions(input)
    expect(input.map((s) => s.path)).toEqual(['a', 'b', 'c'])
  })
})

describe('sessionsFilterActive', () => {
  it('is false in the default state', () => {
    expect(sessionsFilterActive.value).toBe(false)
  })

  it('is true once a search term is entered', () => {
    sessionSearch.value = 'x'
    expect(sessionsFilterActive.value).toBe(true)
  })

  it('treats a whitespace-only search as inactive', () => {
    sessionSearch.value = '   '
    expect(sessionsFilterActive.value).toBe(false)
  })

  it('is true for any non-default sort', () => {
    sessionSort.value = 'size'
    expect(sessionsFilterActive.value).toBe(true)
  })
})

describe('resetSessionsToolbar', () => {
  it('restores every field to its default', () => {
    sessionSearch.value = 'q'
    sessionSort.value = 'messages'

    resetSessionsToolbar()

    expect(sessionSearch.value).toBe('')
    expect(sessionSort.value).toBe('recent')
    expect(sessionsFilterActive.value).toBe(false)
  })
})
