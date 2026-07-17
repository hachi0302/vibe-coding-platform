import { afterEach, describe, expect, it } from 'vitest'
import { isAutoModeConfirmed, rememberAutoModeConfirmed } from '../src/autoMode'

const CWD_A = '/Users/wuchao/develop/flutter/sales-app'
const CWD_B = '/Users/wuchao/apps/claude-session-viewer'

afterEach(() => {
  localStorage.clear()
})

describe('autoMode confirmed-workspaces', () => {
  it('is not confirmed before remembering', () => {
    expect(isAutoModeConfirmed(CWD_A)).toBe(false)
  })

  it('remembers a workspace and persists to localStorage', () => {
    rememberAutoModeConfirmed(CWD_A)
    expect(isAutoModeConfirmed(CWD_A)).toBe(true)
    // 跨「刷新」仍记得：直接读底层存储验证持久化。
    const raw = localStorage.getItem('autoModeConfirmedWorkspaces')
    expect(raw && JSON.parse(raw)).toContain(CWD_A)
  })

  it('is per-workspace — confirming A does not confirm B', () => {
    rememberAutoModeConfirmed(CWD_A)
    expect(isAutoModeConfirmed(CWD_B)).toBe(false)
  })

  it('ignores empty / undefined cwd (no entry, never confirmed)', () => {
    rememberAutoModeConfirmed('')
    rememberAutoModeConfirmed(undefined)
    expect(isAutoModeConfirmed('')).toBe(false)
    expect(isAutoModeConfirmed(undefined)).toBe(false)
    expect(localStorage.getItem('autoModeConfirmedWorkspaces')).toBeNull()
  })

  it('dedupes repeated remembers', () => {
    rememberAutoModeConfirmed(CWD_A)
    rememberAutoModeConfirmed(CWD_A)
    const raw = localStorage.getItem('autoModeConfirmedWorkspaces')
    expect(JSON.parse(raw as string)).toEqual([CWD_A])
  })

  it('tolerates corrupt storage (returns not-confirmed instead of throwing)', () => {
    localStorage.setItem('autoModeConfirmedWorkspaces', '{not json')
    expect(isAutoModeConfirmed(CWD_A)).toBe(false)
  })
})
