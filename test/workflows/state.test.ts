import { beforeEach, describe, expect, it } from 'vitest'
import { getWorkflow, loadProjectWorkflows, saveWorkflow, setWorkflowAgent, setWorkflowPhase } from '../../src/workflows/state'
import type { WorkflowRecord } from '../../src/workflows/types'

const first: WorkflowRecord = {
  id: 'wf-a',
  project: { key: 'project-a', name: 'Project A', path: '/tmp/a' },
  draft: {
    kind: 'feature',
    description: '新增导出能力',
    attachments: [],
    requireDesignConfirmation: true,
  },
  title: '新需求 · 新增导出能力',
  phase: 'designing',
  agent: 'codex',
  createdAt: 1,
  updatedAt: 1,
  notes: [],
}

describe('workflow state', () => {
  beforeEach(() => localStorage.clear())

  it('keeps workflow records isolated by project key', () => {
    saveWorkflow(first)
    saveWorkflow({ ...first, id: 'wf-b', project: { key: 'project-b', name: 'Project B', path: '/tmp/b' } })

    expect(loadProjectWorkflows('project-a')).toEqual([first])
    expect(loadProjectWorkflows('project-b')).toHaveLength(1)
  })

  it('updates only the requested workflow phase and keeps the record serializable', () => {
    saveWorkflow(first)
    const updated = setWorkflowPhase(
      'project-a',
      'wf-a',
      'awaiting-confirmation',
      '等待用户确认修复方案。',
      'execution',
    )

    expect(updated?.phase).toBe('awaiting-confirmation')
    expect(updated?.decisionStage).toBe('execution')
    expect(updated?.notes).toContain('等待用户确认修复方案。')
    expect(loadProjectWorkflows('project-a')[0]).toMatchObject({ id: 'wf-a', phase: 'awaiting-confirmation' })
  })

  it('records the target agent for a handoff without creating a second workflow', () => {
    saveWorkflow(first)

    const updated = setWorkflowAgent('project-a', 'wf-a', 'claude', '已交接给 Claude。')

    expect(updated).toMatchObject({ id: 'wf-a', agent: 'claude' })
    expect(updated?.notes).toContain('已交接给 Claude。')
    expect(loadProjectWorkflows('project-a')).toHaveLength(1)
  })

  it('finds an active workflow by its stable project and workflow identifiers', () => {
    saveWorkflow(first)

    expect(getWorkflow('project-a', 'wf-a')).toMatchObject({ id: 'wf-a' })
    expect(getWorkflow('project-b', 'wf-a')).toBeUndefined()
  })
})
