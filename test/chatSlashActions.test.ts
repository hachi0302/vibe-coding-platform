import { describe, expect, it } from 'vitest'
import { parseChatSlashAction } from '../src/chatSlashActions'

describe('parseChatSlashAction', () => {
  it('routes "/btw <prompt>" carrying the prompt', () => {
    expect(parseChatSlashAction('/btw what does foo do?')).toEqual({
      kind: 'btw',
      prompt: 'what does foo do?',
    })
  })

  it('routes a bare "/btw" with no prompt (tolerating whitespace)', () => {
    expect(parseChatSlashAction('/btw')).toEqual({ kind: 'btw', prompt: undefined })
    expect(parseChatSlashAction('  /btw   ')).toEqual({ kind: 'btw', prompt: undefined })
  })

  it('classifies the arg-less client commands', () => {
    expect(parseChatSlashAction('/export')).toEqual({ kind: 'export' })
    expect(parseChatSlashAction('/rename')).toEqual({ kind: 'rename' })
    expect(parseChatSlashAction('/clear')).toEqual({ kind: 'clear' })
    expect(parseChatSlashAction('/fork')).toEqual({ kind: 'fork' })
    expect(parseChatSlashAction('/model')).toEqual({ kind: 'model' })
  })

  it('is case-insensitive and tolerates surrounding whitespace', () => {
    expect(parseChatSlashAction('  /EXPORT  ')).toEqual({ kind: 'export' })
    expect(parseChatSlashAction('/Fork')).toEqual({ kind: 'fork' })
    expect(parseChatSlashAction('\t/Model\n')).toEqual({ kind: 'model' })
  })

  it('does NOT intercept $-prefixed commands (Codex $ is for skills, not system commands)', () => {
    expect(parseChatSlashAction('$model')).toBeNull()
    expect(parseChatSlashAction('$export')).toBeNull()
    expect(parseChatSlashAction('$rename')).toBeNull()
  })

  it('/model /export /rename intercept even with trailing text', () => {
    expect(parseChatSlashAction('/model opus')).toEqual({ kind: 'model' })
    expect(parseChatSlashAction('/export now')).toEqual({ kind: 'export' })
    expect(parseChatSlashAction('/rename sd')).toEqual({ kind: 'rename' })
  })

  it('/clear and /fork require exact match (no trailing text)', () => {
    expect(parseChatSlashAction('/clear all')).toBeNull()
    expect(parseChatSlashAction('/fork please')).toBeNull()
  })

  it('passes through real CLI commands and plain prose (send normally)', () => {
    expect(parseChatSlashAction('/compact')).toBeNull()
    expect(parseChatSlashAction('/context')).toBeNull()
    expect(parseChatSlashAction('/reload-skills')).toBeNull()
    expect(parseChatSlashAction('hello /export world')).toBeNull()
    expect(parseChatSlashAction('please export the chat')).toBeNull()
    expect(parseChatSlashAction('')).toBeNull()
  })
})
