import { describe, expect, it } from 'vitest'
import {
  applyTerminalInputLineState,
  isSlashCommandInput,
  shouldTerminalInputStartTurn,
} from '../src/tabStatus'

describe('terminal input status inference', () => {
  it('does not mark known slash commands as a user turn', () => {
    for (const input of [
      '/copy',
      '/status',
      '/diff',
      '/model gpt-5',
      '/permissions',
      '/plan',
      '/goal pause',
      '/side quick question',
      '/btw quick question',
      '  /theme',
    ]) {
      expect(isSlashCommandInput(input)).toBe(true)
      expect(shouldTerminalInputStartTurn('codex', input)).toBe(false)
      expect(shouldTerminalInputStartTurn('claude', input)).toBe(false)
    }
  })

  it('does not optimistically start turns for any slash input', () => {
    expect(isSlashCommandInput('/unknown maybe a future command')).toBe(true)
    expect(shouldTerminalInputStartTurn('codex', '/unknown maybe a future command')).toBe(false)
  })

  it('keeps normal prompts eligible to start a turn', () => {
    expect(shouldTerminalInputStartTurn('codex', 'fix this bug')).toBe(true)
    expect(shouldTerminalInputStartTurn('claude', 'fix this bug')).toBe(true)
  })

  it('ignores empty terminal input', () => {
    expect(shouldTerminalInputStartTurn('codex', '')).toBe(false)
    expect(shouldTerminalInputStartTurn('codex', '   ')).toBe(false)
  })

  it('extracts submitted terminal lines from chunked and pasted input', () => {
    expect(applyTerminalInputLineState('/cop', 'y\r')).toEqual({
      nextLine: '',
      submittedLines: ['/copy'],
    })
    expect(applyTerminalInputLineState('', 'fix bug\r')).toEqual({
      nextLine: '',
      submittedLines: ['fix bug'],
    })
  })

  it('tracks basic terminal line editing before submit', () => {
    expect(applyTerminalInputLineState('/stats', '\b\b\batus\r')).toEqual({
      nextLine: '',
      submittedLines: ['/status'],
    })
    expect(applyTerminalInputLineState('/copy', '\x15/status\r')).toEqual({
      nextLine: '',
      submittedLines: ['/status'],
    })
  })

  it('ignores terminal control sequences before an empty submit', () => {
    expect(applyTerminalInputLineState('', '\x1b[I\r')).toEqual({
      nextLine: '',
      submittedLines: [''],
    })
    expect(applyTerminalInputLineState('', '\x1b[200~\x1b[201~\r')).toEqual({
      nextLine: '',
      submittedLines: [''],
    })
    expect(applyTerminalInputLineState('', '\x1b[A\r')).toEqual({
      nextLine: '',
      submittedLines: [''],
    })
  })
})
