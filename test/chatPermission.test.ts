import { describe, expect, it } from 'vitest'
import {
  buildPermissionDecision,
  permissionCommandPreview,
  permissionHasSuggestions,
} from '../src/chatPermission'
import type { ChatPermissionRequest } from '../src/types'

const req = (over: Partial<ChatPermissionRequest> = {}): ChatPermissionRequest => ({
  requestId: 'r1',
  toolName: 'Bash',
  input: { command: 'echo hi' },
  ...over,
})

const suggestions = [{ type: 'addRules', rules: [{ toolName: 'Bash' }], behavior: 'allow', destination: 'localSettings' }]

describe('permissionCommandPreview', () => {
  it('shows the shell command for Bash', () => {
    expect(permissionCommandPreview(req({ input: { command: 'rm -rf build' } }))).toBe('rm -rf build')
  })

  it('shows the file path for file tools', () => {
    expect(permissionCommandPreview(req({ toolName: 'Write', input: { file_path: '/a/b.ts', content: 'x' } }))).toBe(
      '/a/b.ts',
    )
  })

  it('falls back across path/pattern/url for other tools', () => {
    expect(permissionCommandPreview(req({ toolName: 'Grep', input: { pattern: 'TODO' } }))).toBe('TODO')
    expect(permissionCommandPreview(req({ toolName: 'WebFetch', input: { url: 'https://x.dev' } }))).toBe('https://x.dev')
  })

  it('returns undefined when nothing salient is present', () => {
    expect(permissionCommandPreview(req({ toolName: 'Bash', input: {} }))).toBeUndefined()
    expect(permissionCommandPreview(req({ toolName: 'Bash', input: null }))).toBeUndefined()
  })
})

describe('permissionHasSuggestions', () => {
  it('is true only for a non-empty suggestions array', () => {
    expect(permissionHasSuggestions(req({ permissionSuggestions: suggestions }))).toBe(true)
    expect(permissionHasSuggestions(req({ permissionSuggestions: [] }))).toBe(false)
    expect(permissionHasSuggestions(req({ permissionSuggestions: undefined }))).toBe(false)
  })
})

describe('buildPermissionDecision', () => {
  it('allow-once echoes the original input as updatedInput, no permissions', () => {
    const d = buildPermissionDecision(req({ input: { command: 'ls' } }), 'allow-once')
    expect(d).toEqual({ behavior: 'allow', updatedInput: { command: 'ls' } })
    expect(d).not.toHaveProperty('updatedPermissions')
  })

  it('always-allow attaches the CLI rule suggestions as updatedPermissions', () => {
    const d = buildPermissionDecision(req({ input: { command: 'ls' }, permissionSuggestions: suggestions }), 'always-allow')
    expect(d.behavior).toBe('allow')
    expect(d.updatedInput).toEqual({ command: 'ls' })
    expect(d.updatedPermissions).toBe(suggestions)
  })

  it('always-allow without suggestions degrades to a plain allow', () => {
    const d = buildPermissionDecision(req({ permissionSuggestions: [] }), 'always-allow')
    expect(d).not.toHaveProperty('updatedPermissions')
    expect(d.behavior).toBe('allow')
  })

  it('deny does not interrupt the turn — it feeds the refusal back to the model', () => {
    const d = buildPermissionDecision(req(), 'deny')
    expect(d.behavior).toBe('deny')
    expect(d.interrupt).toBe(false)
    expect(typeof d.message).toBe('string')
  })

  it('falls back to an empty object input when the request carries none', () => {
    const d = buildPermissionDecision(req({ input: undefined }), 'allow-once')
    expect(d.updatedInput).toEqual({})
  })
})
