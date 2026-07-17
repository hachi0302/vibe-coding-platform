import { beforeEach, describe, expect, it } from 'vitest'
import {
  clearRecents,
  closeGlobalSearch,
  globalSearchOpen,
  openGlobalSearch,
  pushRecent,
  recentSearches,
  removeRecent,
} from '../src/globalSearch'

// 共享状态 + sessionStorage。每个用例先清干净，免得互相污染。
beforeEach(() => {
  closeGlobalSearch()
  clearRecents()
  sessionStorage.clear()
})

describe('globalSearch state', () => {
  it('open / close flips the shared ref', () => {
    expect(globalSearchOpen.value).toBe(false)
    openGlobalSearch()
    expect(globalSearchOpen.value).toBe(true)
    closeGlobalSearch()
    expect(globalSearchOpen.value).toBe(false)
  })

  it('pushRecent prepends and dedupes', () => {
    pushRecent('foo')
    pushRecent('bar')
    pushRecent('foo')
    expect(recentSearches.value).toEqual(['foo', 'bar'])
  })

  it('pushRecent ignores empty / whitespace queries', () => {
    pushRecent('   ')
    pushRecent('')
    expect(recentSearches.value).toEqual([])
  })

  it('caps the recent list at 6 entries', () => {
    for (let i = 1; i <= 8; i++) pushRecent(`q${i}`)
    expect(recentSearches.value).toHaveLength(6)
    expect(recentSearches.value[0]).toBe('q8')
  })

  it('persists recents to sessionStorage', () => {
    pushRecent('hello')
    const raw = sessionStorage.getItem('csv:global-search:recent')
    expect(raw && JSON.parse(raw)).toEqual(['hello'])
  })

  it('clearRecents wipes the list and the persisted entry', () => {
    pushRecent('hello')
    clearRecents()
    expect(recentSearches.value).toEqual([])
    expect(sessionStorage.getItem('csv:global-search:recent')).toBe('[]')
  })

  describe('removeRecent', () => {
    it('removes a single entry and keeps the others in order', () => {
      pushRecent('a')
      pushRecent('b')
      pushRecent('c')
      removeRecent('b')
      expect(recentSearches.value).toEqual(['c', 'a'])
    })

    it('persists the deletion to sessionStorage', () => {
      pushRecent('a')
      pushRecent('b')
      removeRecent('a')
      const raw = sessionStorage.getItem('csv:global-search:recent')
      expect(raw && JSON.parse(raw)).toEqual(['b'])
    })

    it('is a no-op for a query that is not in the list', () => {
      pushRecent('a')
      removeRecent('nope')
      expect(recentSearches.value).toEqual(['a'])
    })
  })
})
