import { describe, expect, it } from 'vitest'
import { shouldCopyWindowsTerminalSelection } from '../src/terminals'

function key(over: Partial<KeyboardEvent> = {}) {
  return {
    type: 'keydown',
    key: 'c',
    ctrlKey: true,
    shiftKey: false,
    altKey: false,
    metaKey: false,
    ...over,
  } as KeyboardEvent
}

describe('terminal keyboard handling', () => {
  it('copies terminal selection on Windows Ctrl+C', () => {
    expect(shouldCopyWindowsTerminalSelection(key(), true, 'Win32')).toBe(true)
  })

  it('does not intercept Ctrl+C without a terminal selection', () => {
    expect(shouldCopyWindowsTerminalSelection(key(), false, 'Win32')).toBe(false)
  })

  it('does not intercept non-Windows Ctrl+C', () => {
    expect(shouldCopyWindowsTerminalSelection(key(), true, 'MacIntel')).toBe(false)
  })

  it('does not intercept modified or unrelated keys', () => {
    expect(shouldCopyWindowsTerminalSelection(key({ shiftKey: true }), true, 'Win32')).toBe(false)
    expect(shouldCopyWindowsTerminalSelection(key({ key: 'v' }), true, 'Win32')).toBe(false)
  })
})
