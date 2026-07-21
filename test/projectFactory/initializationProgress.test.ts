import { describe, expect, it } from 'vitest'
import * as initializationUi from '../../src/projectFactory/initializationProgress'

const { initializationProgressFor } = initializationUi

describe('project initialization progress', () => {
  it('maps every v4 checkpoint to a truthful progress stage', () => {
    expect(initializationProgressFor('scan')).toMatchObject({
      phase: 'scan',
      detail: '正在扫描项目结构与安全快照',
    })
    expect(initializationProgressFor('plan')).toMatchObject({
      phase: 'plan',
      detail: '正在规划有证据支撑的项目产物',
    })
    expect(initializationProgressFor('skills')).toMatchObject({
      phase: 'skills',
      detail: '正在生成项目专属 skills',
    })
    expect(initializationProgressFor('install')).toMatchObject({
      phase: 'install',
      detail: '正在检查冲突并安装项目产物',
    })
    expect(initializationProgressFor('verify')).toMatchObject({
      phase: 'verify',
      detail: '正在确认安装结果与所有权清单',
    })
    expect(initializationProgressFor('complete')).toMatchObject({
      phase: 'complete',
      percent: 100,
      detail: '初始化完成',
    })
  })

  it('classifies legacy, recoverable, conflict, and current-v4 statuses', () => {
    const actionFor = (initializationUi as Record<string, unknown>).initializationActionForStatus

    expect(actionFor).toBeTypeOf('function')
    const decide = actionFor as (status: unknown) => string
    expect(decide({ status: 'legacy-v3', initialized: true, markerVersion: 'v3' })).toBe('start')
    expect(decide({
      status: 'incomplete',
      runId: 'run-1',
      phase: 'rules',
      recoverable: true,
    })).toBe('resume')
    expect(decide({
      status: 'needs-attention',
      runId: 'run-2',
      phase: 'conflict',
      recoverable: false,
      conflicts: [{ path: 'CLAUDE.md', detail: '文件已由用户修改' }],
    })).toBe('attention')
    expect(decide({ status: 'current-v4', initialized: true, markerVersion: 'v4' })).toBe('complete')
  })

  it('uses a stable public summary while restoring a recoverable run', () => {
    const progressFromStatus = (initializationUi as Record<string, unknown>).initializationProgressFromStatus

    expect(progressFromStatus).toBeTypeOf('function')
    const progress = (progressFromStatus as (status: unknown) => unknown)({
      status: 'incomplete',
      runId: 'run-3',
      phase: 'interrupted',
      percent: 61,
      detail: '规则阶段完成后进程退出，可从 skills 阶段继续',
      attempt: 2,
      sequence: 17,
      recoverable: true,
      issues: [{ code: 'rules.missing-verification', detail: '规则缺少验证命令' }],
      conflicts: [],
      warnings: ['上次进程已退出'],
      artifactTotals: { documents: 3, rules: 2, skills: 1, total: 6 },
    })

    expect(progress).toMatchObject({
      phase: 'interrupted',
      percent: 61,
      detail: '初始化已中断，可从上次有效节点继续',
      runId: 'run-3',
      attempt: 2,
      sequence: 17,
      recoverable: true,
      issues: [{ code: 'rules.missing-verification', detail: '规则缺少验证命令' }],
      conflicts: [],
      warnings: ['上次进程已退出'],
      artifactTotals: { documents: 3, rules: 2, skills: 1, total: 6 },
    })
  })

  it('formats completion from report totals by artifact type', () => {
    const completionDetail = (initializationUi as Record<string, unknown>).initializationCompletionDetail

    expect(completionDetail).toBeTypeOf('function')
    expect((completionDetail as (totals: unknown) => string)({
      documents: 3,
      rules: 2,
      skills: 1,
      total: 6,
    })).toBe('初始化完成：已安装 3 份文档、2 条规则、1 个 skill。')
    expect(() => (completionDetail as (totals?: unknown) => string)()).toThrow(
      '初始化完成结果缺少 artifactTotals，无法确认产物数量。',
    )
  })

  it('guards unsupported agents without suggesting a chat or permission flow', () => {
    const guard = (initializationUi as Record<string, unknown>).initializationAgentGuardMessage

    expect(guard).toBeTypeOf('function')
    const message = (guard as (agent: string) => string | null)('agy')
    expect(message).toBe('项目初始化仅支持选择 Claude 或 Codex。')
    expect(message).not.toMatch(/GUI|会话|权限|\/init/)
    expect((guard as (agent: string) => string | null)('codex')).toBeNull()
    expect((guard as (agent: string) => string | null)('claude')).toBeNull()
  })

  it('keeps only running initialization phases in the background task list', () => {
    const isVisible = (initializationUi as Record<string, unknown>).isInitializationTaskVisible

    expect(isVisible).toBeTypeOf('function')
    expect((isVisible as (phase: string) => boolean)('scan')).toBe(true)
    expect((isVisible as (phase: string) => boolean)('documents')).toBe(true)
    expect((isVisible as (phase: string) => boolean)('complete')).toBe(false)
    expect((isVisible as (phase: string) => boolean)('failed')).toBe(false)
    expect((isVisible as (phase: string) => boolean)('interrupted')).toBe(false)
    expect((isVisible as (phase: string) => boolean)('conflict')).toBe(false)
  })

  it('shows the sidebar task card only after running progress is minimized', () => {
    const isCardVisible = (initializationUi as Record<string, unknown>).isInitializationTaskCardVisible

    expect(isCardVisible).toBeTypeOf('function')
    expect((isCardVisible as (phase: string, minimized: boolean) => boolean)('scan', false)).toBe(false)
    expect((isCardVisible as (phase: string, minimized: boolean) => boolean)('scan', true)).toBe(true)
    expect((isCardVisible as (phase: string, minimized: boolean) => boolean)('complete', true)).toBe(false)
    expect((isCardVisible as (phase: string, minimized: boolean) => boolean)('failed', true)).toBe(false)
  })
})
