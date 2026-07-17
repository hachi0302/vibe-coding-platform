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

  it('builds a headless initialization contract without chat commands or checkpoints', () => {
    const input = buildProjectInitializationPrompt(project)

    expect(input.startsWith('/init')).toBe(false)
    expect(input).toContain('后台非会话')
    expect(input).toContain('第一步读取并执行 `.claude/skills/skill-designer/SKILL.md`')
    expect(input).toContain('版本号、任务序号、模块编号')
    expect(input).toContain('禁止写死 `01`')
    expect(input).toContain('其他长期文档不得留下 `{{占位符}}`')
    expect(input).toContain('不得把通用模板原文不加分析地复制成项目规则')
    expect(input).toContain('使用 skill-designer 生成并校验项目专属')
    expect(input).not.toContain('生成 `worktree`')
    expect(input).not.toContain('项目专属 `worktree`')
    expect(input).toContain('前端项目不得生成物理模型')
    expect(input).toContain('只有扫描到真实服务端路由')
    expect(input).toContain('所有后端项目都生成项目专属 `backend-log-diagnose`')
    expect(input).toContain('检测到数据库连接配置时生成项目专属 `database-read-diagnose`')
    expect(input).toContain('检测到真实第三方客户端或 SDK 调用时')
    expect(input).not.toContain('运维接入说明')
    expect(input).not.toContain('WORKFLOW_CHECKPOINT')
    expect(input).not.toContain('initialization-ready')
    expect(input).not.toContain('完成 /init 后再补全')
  })

  it('parses the initialization-ready checkpoint only after the agent has real artifacts', () => {
    expect(parseWorkflowCheckpoints(
      'WORKFLOW_CHECKPOINT: {"phase":"initialization-ready","note":"已扫描并生成真实项目资产"}',
    )).toEqual([{ phase: 'initialization-ready', note: '已扫描并生成真实项目资产' }])
  })
})
