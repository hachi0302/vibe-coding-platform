import { describe, expect, it } from 'vitest'
import {
  ensureLayout,
  splitPane,
  panes,
  leafPaneIds,
  currentAgent,
  currentProjectKey,
  migratePaneProjectKey,
} from '../src/panes'

describe('migratePaneProjectKey', () => {
  it('re-keys the layout and every pane, and follows currentProjectKey', () => {
    const oldKey = 'worktree:/proj/wt'
    const newKey = 'realkey'

    const layout = ensureLayout('claude', oldKey)
    const created = splitPane(layout.focusedPaneId, 'row') // 分成 2 格
    expect(created).not.toBeNull()

    const paneIds = [...panes.values()]
      .filter((p) => p.agent === 'claude' && p.projectKey === oldKey)
      .map((p) => p.id)
      .sort()
    expect(paneIds.length).toBeGreaterThanOrEqual(2)

    currentAgent.value = 'claude'
    currentProjectKey.value = oldKey

    migratePaneProjectKey('claude', oldKey, newKey)

    // 每个 pane 的 projectKey 都迁到新 key
    for (const id of paneIds) expect(panes.get(id)!.projectKey).toBe(newKey)
    // 正停在旧 key 的视图 → currentProjectKey 一并切过去（否则 currentLayout 会新建空布局）
    expect(currentProjectKey.value).toBe(newKey)
    // 新 key 解析到的布局保留了原来的分屏结构（同一批 pane），这正是「点 List 后分屏/tab 消失」的修复
    const migrated = ensureLayout('claude', newKey)
    expect(leafPaneIds(migrated.tree).sort()).toEqual(paneIds)
    // 旧 key 已被腾空 → 再访问得到一个全新的单格布局，pane 也是新的
    const freshOld = ensureLayout('claude', oldKey)
    expect(freshOld.tree.kind).toBe('leaf')
    expect(paneIds).not.toContain(leafPaneIds(freshOld.tree)[0])
  })

  it('is a no-op when old and new keys are equal', () => {
    const key = 'stable'
    const before = leafPaneIds(ensureLayout('codex', key).tree)
    migratePaneProjectKey('codex', key, key)
    expect(leafPaneIds(ensureLayout('codex', key).tree)).toEqual(before)
  })
})
