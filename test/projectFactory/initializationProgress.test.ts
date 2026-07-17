import { describe, expect, it } from 'vitest'
import * as initializationUi from '../../src/projectFactory/initializationProgress'

const { initializationProgressFor } = initializationUi

describe('project initialization progress', () => {
  it('maps only real initialization milestones to the shared four-step progress display', () => {
    expect(initializationProgressFor('analyze')).toMatchObject({
      phase: 'analyze',
      percent: 8,
      detail: '正在分析项目代码、配置与已有资料',
    })
    expect(initializationProgressFor('rules')).toMatchObject({
      phase: 'rules',
      percent: 62,
      detail: '正在生成项目规则与 skills',
    })
    expect(initializationProgressFor('complete')).toMatchObject({
      phase: 'complete',
      percent: 100,
      detail: '初始化完成',
    })
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
    expect((isVisible as (phase: string) => boolean)('analyze')).toBe(true)
    expect((isVisible as (phase: string) => boolean)('documents')).toBe(true)
    expect((isVisible as (phase: string) => boolean)('complete')).toBe(false)
    expect((isVisible as (phase: string) => boolean)('failed')).toBe(false)
  })
})
