import { describe, expect, it, beforeEach } from 'vitest'
import {
  createViewTab,
  viewTabs,
  migrateViewTabsProjectKey,
  visibleViewTabs,
} from '../src/viewTabs'

describe('migrateViewTabsProjectKey', () => {
  beforeEach(() => {
    viewTabs.value = []
  })

  it('moves every view tab from the old project key to the new one', () => {
    const a = createViewTab({ type: 'session', agent: 'claude', projectKey: 'worktree:/p/wt' })
    const b = createViewTab({ type: 'chat', agent: 'claude', projectKey: 'worktree:/p/wt' })
    const other = createViewTab({ type: 'session', agent: 'claude', projectKey: 'other' })

    migrateViewTabsProjectKey('worktree:/p/wt', 'realkey')

    expect(a.projectKey).toBe('realkey')
    expect(b.projectKey).toBe('realkey')
    expect(other.projectKey).toBe('other') // 无关项目不动
    // 迁移后旧 key 查不到、新 key 查得到 —— 这正是「点 List tab 后标签栏消失」的根因修复。
    expect(visibleViewTabs('claude', 'worktree:/p/wt')).toHaveLength(0)
    expect(visibleViewTabs('claude', 'realkey')).toHaveLength(2)
  })

  it('is a no-op when old and new keys are equal', () => {
    const a = createViewTab({ type: 'session', agent: 'claude', projectKey: 'k' })
    migrateViewTabsProjectKey('k', 'k')
    expect(a.projectKey).toBe('k')
  })
})
