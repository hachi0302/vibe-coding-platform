import { afterEach, describe, expect, it, vi } from 'vitest'
import {
  navigate,
  resetChatToolbar,
  search,
  searchCount,
  searchIndex,
  searchScope,
  setSearchNavigator,
  toolsCollapsed,
} from '../src/chatToolbar'

afterEach(() => {
  setSearchNavigator(null)
  resetChatToolbar()
})

describe('chatToolbar refs', () => {
  it('start at their documented defaults', () => {
    // toolsCollapsed 默认 true：跟 <details> 初始 DOM（关闭）对齐，
    // 否则首次点"展开"按钮要点两下才动作（见 chatToolbar.ts 注释）。
    expect(toolsCollapsed.value).toBe(true)
    expect(search.value).toBe('')
    expect(searchScope.value).toBe('all')
    expect(searchCount.value).toBe(0)
    expect(searchIndex.value).toBe(0)
  })
})

describe('resetChatToolbar', () => {
  it('zeroes every piece of search/collapse state', () => {
    toolsCollapsed.value = false
    search.value = 'needle'
    searchScope.value = 'agent'
    searchCount.value = 7
    searchIndex.value = 3

    resetChatToolbar()

    // 重置回 true（折叠态），跟新会话进入时 <details> 默认关闭的 DOM 状态对齐。
    expect(toolsCollapsed.value).toBe(true)
    expect(search.value).toBe('')
    expect(searchScope.value).toBe('all')
    expect(searchCount.value).toBe(0)
    expect(searchIndex.value).toBe(0)
  })
})

describe('search navigator', () => {
  it('does nothing when no navigator is registered', () => {
    expect(() => navigate(1)).not.toThrow()
  })

  it('forwards the direction to a registered navigator', () => {
    const fn = vi.fn()
    setSearchNavigator(fn)

    navigate(1)
    navigate(-1)

    expect(fn).toHaveBeenNthCalledWith(1, 1)
    expect(fn).toHaveBeenNthCalledWith(2, -1)
  })

  it('stops forwarding once the navigator is unregistered', () => {
    const fn = vi.fn()
    setSearchNavigator(fn)
    setSearchNavigator(null)

    navigate(1)

    expect(fn).not.toHaveBeenCalled()
  })
})
