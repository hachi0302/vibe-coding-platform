import { describe, expect, it } from 'vitest'
import { systemSlashCommands } from '../src/chatSystemCommands'
import { setLang } from '../src/settings'

describe('systemSlashCommands', () => {
  it('exposes the full set for Claude with /-prefixed titles and a system kind/origin', () => {
    setLang('en')
    const cmds = systemSlashCommands('claude')
    expect(cmds.map((c) => c.name)).toEqual([
      'model',
      'export',
      'rename',
      'clear',
      'fork',
      'compact',
      'goal',
      'plan',
      'btw',
      'context',
      'reload-skills',
    ])
    expect(cmds.every((c) => c.kind === 'system' && c.origin === 'system')).toBe(true)
    expect(cmds.every((c) => c.title === `/${c.name}`)).toBe(true)
    expect(cmds.every((c) => c.description.length > 0)).toBe(true)
  })

  it('limits non-Claude agents to the universal client actions', () => {
    setLang('en')
    const names = systemSlashCommands('codex').map((c) => c.name)
    expect(names).toEqual([
      'model', 'export', 'rename', 'clear', 'compact',
      'goal', 'plan', 'review', 'archive',
    ])
    expect(names).not.toContain('fork')
    expect(names).not.toContain('btw')
    expect(names).not.toContain('context')
  })

  it('localizes descriptions via t()', () => {
    setLang('zh')
    const model = systemSlashCommands('claude').find((c) => c.name === 'model')
    expect(model?.description).toContain('模型')
    setLang('en')
  })
})
