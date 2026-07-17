import { describe, expect, it } from 'vitest'
import { parseWorkflowCheckpoints } from '../../src/workflows/checkpoints'

describe('workflow checkpoints', () => {
  it('parses an explicit confirmation checkpoint without treating ordinary prose as a command', () => {
    expect(parseWorkflowCheckpoints('分析结束，等待确认。')).toEqual([])
    expect(parseWorkflowCheckpoints([
      '先完成了根因分析。',
      'WORKFLOW_CHECKPOINT: {"phase":"awaiting-confirmation","decisionStage":"execution","note":"方案已给出，等待用户确认。"}',
    ].join('\n'))).toEqual([
      {
        phase: 'awaiting-confirmation',
        decisionStage: 'execution',
        note: '方案已给出，等待用户确认。',
      },
    ])
  })

  it('ignores malformed or unsupported checkpoints instead of guessing a phase', () => {
    expect(parseWorkflowCheckpoints('WORKFLOW_CHECKPOINT: {not-json}')).toEqual([])
    expect(parseWorkflowCheckpoints('WORKFLOW_CHECKPOINT: {"phase":"auto-fix"}')).toEqual([])
  })

  it('keeps project initialization progress tied to explicit agent milestones', () => {
    expect(parseWorkflowCheckpoints([
      'WORKFLOW_CHECKPOINT: {"phase":"initialization-analyzed","note":"已完成源码与配置扫描"}',
      'WORKFLOW_CHECKPOINT: {"phase":"initialization-documented","note":"已写入真实项目文档"}',
      'WORKFLOW_CHECKPOINT: {"phase":"initialization-generated","note":"已写入真实文档和规则"}',
    ].join('\n'))).toEqual([
      { phase: 'initialization-analyzed', note: '已完成源码与配置扫描' },
      { phase: 'initialization-documented', note: '已写入真实项目文档' },
      { phase: 'initialization-generated', note: '已写入真实文档和规则' },
    ])
  })
})
