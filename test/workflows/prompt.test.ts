import { describe, expect, it } from 'vitest'
import {
  buildDecisionInput,
  buildHandoffPrompt,
  buildProjectInitializationPrompt,
  buildWorkflowPrompt,
  detectWorkflowKind,
  normalizeInitialInput,
} from '../../src/workflows/prompt'
import type { WorkflowDraft, WorkflowRecord } from '../../src/workflows/types'
import { parseWorkflowCheckpoints } from '../../src/workflows/checkpoints'

const project = {
  key: 'demo',
  name: 'Demo Service',
  path: '/tmp/demo-service',
}

const draft: WorkflowDraft = {
  kind: 'bug',
  description: '登录接口偶发空指针，帮我定位后等我确认再修复。',
  attachments: ['/tmp/login-error.png'],
  requireDesignConfirmation: false,
}

describe('workflow prompt', () => {
  it.each([
    ['接口空指针，帮我定位根因', 'bug'],
    ['根据这张截图优化登录页交互', 'optimization'],
    ['新增退款申请列表和详情页', 'feature'],
    ['检查这个仓库的文档和技能是否齐全', 'onboarding'],
  ] as const)('classifies %s as %s', (input, expected) => {
    expect(detectWorkflowKind(input)).toBe(expected)
  })

  it('builds a project-scoped bug prompt with evidence and confirmation gates', () => {
    const prompt = buildWorkflowPrompt(draft, project)

    expect(prompt).toContain('/tmp/demo-service')
    expect(prompt).toContain('问题定位')
    expect(prompt).toContain('根因')
    expect(prompt).toContain('未经用户确认，不修改代码')
    expect(prompt).toContain('/tmp/login-error.png')
  })

  it('creates a handoff without pretending the target agent can resume the old session', () => {
    const record: WorkflowRecord = {
      id: 'wf-1',
      project,
      draft,
      title: 'Bug · 登录接口空指针',
      phase: 'awaiting-confirmation',
      agent: 'claude',
      createdAt: 1,
      updatedAt: 2,
      notes: ['已读取登录接口与异常栈。'],
    }

    const prompt = buildHandoffPrompt(record, 'codex')

    expect(prompt).toContain('交接摘要')
    expect(prompt).toContain('目标 Agent：codex')
    expect(prompt).toContain('不可 resume')
    expect(prompt).toContain('登录接口空指针')
  })

  it('turns completion decisions into natural-language requests instead of shell commands', () => {
    const decision = buildDecisionInput('commit')

    expect(decision).toContain('请按当前项目提交规则')
    expect(decision).not.toContain('git commit')
    expect(buildDecisionInput('keep')).toContain('保留当前改动')
  })

  it('normalizes the first terminal input to exactly one carriage return', () => {
    expect(normalizeInitialInput('请开始分析')).toBe('请开始分析\r')
    expect(normalizeInitialInput('请开始分析\n')).toBe('请开始分析\r')
  })

  it('builds one stable product-intent prompt without duplicating the Rust artifact contract', () => {
    const input = buildProjectInitializationPrompt(project)

    expect(input.startsWith('/init')).toBe(false)
    expect(input).toContain('当前项目：Demo Service')
    expect(input).toContain('可执行工程上下文')
    expect(input).toContain('真实代码证据')
    expect(input).toContain('不得覆盖用户已有内容')
    expect(input).not.toContain(project.path)
    expect(input).not.toContain('docs/backend/latest/接口文档')
    expect(input).not.toContain('.claude/rules/公共')
    expect(input).not.toMatch(/允许(?:输出|写入|生成).*(?:docs\/|\.claude\/)/)
    expect(input).not.toContain('WORKFLOW_CHECKPOINT')
    expect(input).not.toContain('initialization-ready')
  })

  it('parses the initialization-ready checkpoint only after the agent has real artifacts', () => {
    expect(parseWorkflowCheckpoints(
      'WORKFLOW_CHECKPOINT: {"phase":"initialization-ready","note":"已扫描并生成真实项目资产"}',
    )).toEqual([{ phase: 'initialization-ready', note: '已扫描并生成真实项目资产' }])
  })
})
