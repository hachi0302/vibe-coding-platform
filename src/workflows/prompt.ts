import type { Agent } from '../types'
import type { WorkflowDecision, WorkflowDraft, WorkflowKind, WorkflowProject, WorkflowRecord } from './types'

const KIND_LABEL: Record<WorkflowKind, string> = {
  onboarding: '项目接入',
  feature: '新需求迭代',
  bug: '问题定位',
  optimization: '代码优化',
}

function linesForKind(draft: WorkflowDraft): string[] {
  if (draft.kind === 'onboarding') {
    return [
      '先扫描前端、后端、测试、已有文档、规则与 skills；不要生成运维、部署、监控或 CI 运维资料。',
      '第一步加载 `.claude/skills/skill-designer/SKILL.md`；后续所有新增或修订 skill 必须按它选择模式、组织步骤和验证。',
      '输出能力清单：可用、缺失、证据、影响与建议；缺失时生成项目专属规范，但不得覆盖业务代码或真实已有资料。',
      '文档按实际代码层生成到 `docs/frontend/`、`docs/backend/`；不存在的代码层及无证据的 API、数据库、回调、枚举、第三方能力不生成对应长期文档。',
      '检查 `.claude`、`.agents` 与入口文件；以 `.claude` 和 `CLAUDE.md` 为唯一维护源，另一套使用软链接共享。',
      '完整扫描公共组件、工具类、共享模型、API 客户端、测试夹具和框架扩展点，写清真实适用场景与代码证据；已有能力优先复用，不能重复造轮子。',
      '只根据已读代码、配置、日志、数据库或测试证据下结论；缺证据时明确“未配置 / 不可访问 / 无法判断”。禁止为让流程继续而私自添加吞异常、伪造默认值、模拟成功、自动降级或猜测配置等兜底逻辑。',
    ]
  }
  if (draft.kind === 'feature') {
    return [
      '先查已有同类业务、接口、组件、测试和文档，避免重复实现。',
      '先输出前后端、数据、接口、测试与回滚影响的详细设计，再按确认策略进入开发。',
      draft.requireDesignConfirmation
        ? '详细设计完成后必须等待用户明确确认开始开发。'
        : '详细设计完成后自行检查完整性，再进入开发。',
    ]
  }
  if (draft.kind === 'bug') {
    return [
      '先收集与症状相关的代码、调用链、日志和数据库事实；已配置且可访问的对应 skill 必须使用。',
      '只有证据闭环时才称为根因；否则明确高概率假设、缺失证据和下一步。',
      '未经用户确认，不修改代码；根因或方案后等待“确认修复”等自然语言指令。',
    ]
  }
  return [
    '根据文字、截图或原型先定位路由、组件、样式、状态和相关接口。',
    '先给改动范围、用户可见效果、风险与验证方案；未经用户确认，不修改代码。',
  ]
}

export function detectWorkflowKind(text: string): WorkflowKind {
  const source = text.toLowerCase()
  if (/(报错|异常|空指针|bug|故障|定位|根因|修复)/.test(source)) return 'bug'
  if (/(截图|原型|优化|样式|ui|体验|性能优化|重构)/.test(source)) return 'optimization'
  if (/(接入|扫描|初始化|规则|技能|文档.*齐全)/.test(source)) return 'onboarding'
  return 'feature'
}

export function workflowTitle(draft: WorkflowDraft): string {
  const compact = draft.description.replace(/\s+/g, ' ').trim()
  const label = KIND_LABEL[draft.kind]
  return compact ? `${label} · ${compact.slice(0, 28)}${compact.length > 28 ? '…' : ''}` : label
}

export function buildWorkflowPrompt(draft: WorkflowDraft, project: WorkflowProject): string {
  const attachmentLines = draft.attachments.length
    ? ['附件本地引用：', ...draft.attachments.map((item) => `- ${item}`)]
    : ['附件：无']
  return [
    '你正在执行“既有项目 Agent 工作流”。',
    `当前项目：${project.name}`,
    `项目路径：${project.path}`,
    `任务类型：${KIND_LABEL[draft.kind]}`,
    `用户任务：${draft.description.trim()}`,
    ...attachmentLines,
    '执行约束：',
    ...linesForKind(draft).map((item) => `- ${item}`),
    '- 所有分析、命令、证据和结论都在当前会话呈现；不要假设你能访问未授权的日志、数据库、SSH 或云平台。',
    '- 自测要覆盖正常、边界、异常和原 Bug case，并记录真实命令与结果；编译通过不等于自测完成。',
    '- 自测结束后，等待用户选择“提交代码 / 保留改动 / 继续调整”。绝不自动提交或推送。',
    '- 到需要用户决策的阶段时，单独输出一行 `WORKFLOW_CHECKPOINT: {"phase":"awaiting-confirmation","decisionStage":"design|execution","note":"简短说明"}`；自测结束时输出 `WORKFLOW_CHECKPOINT: {"phase":"awaiting-delivery-decision","decisionStage":"delivery","note":"真实自测结论"}`；工作全部结束时输出 `WORKFLOW_CHECKPOINT: {"phase":"completed","note":"交付摘要"}`。只在阶段真实达到时输出，禁止伪造检查点。',
    '请先用简短中文说明：识别到的任务、当前阶段、将读取的项目规则/skills、下一步动作。',
  ].join('\n')
}

/** 既有项目后台初始化契约。无聊天会话，完成状态只以真实文件校验为准。 */
export function buildProjectInitializationPrompt(project: WorkflowProject): string {
  return [
    '你正在执行既有项目的后台初始化。',
    `当前项目：${project.name}`,
    '产品目标：依据真实代码证据，把当前仓库编译为可执行工程上下文，使后续开发、修复、重构与评审能先理解架构边界、复用已有能力并运行真实验证。',
    '安全意图：不得覆盖用户已有内容，不得修改业务源码，不得泄露密钥；证据不足时保留明确诊断，不得猜测或伪造成功。',
    '具体阶段、产物计划、允许路径、JSON 契约与完成判定由初始化引擎追加并负责。',
  ].join('\n')
}

export function buildHandoffPrompt(record: WorkflowRecord, targetAgent: Agent): string {
  return [
    '既有项目 Agent 工作流交接摘要。',
    `目标 Agent：${targetAgent}`,
    `项目：${record.project.name}（${record.project.path}）`,
    `任务：${record.title}`,
    `任务类型：${KIND_LABEL[record.draft.kind]}`,
    `当前阶段：${record.phase}`,
    `原 Agent：${record.agent}`,
    `原会话不可 resume；请以本摘要继续，不要声称拥有原会话上下文。`,
    `用户原始任务：${record.draft.description}`,
    record.notes.length ? `已有记录：\n${record.notes.map((item) => `- ${item}`).join('\n')}` : '已有记录：暂无。',
    '先确认已接收交接，并说明将继续的下一步；仍遵守原任务的确认与提交约束。',
  ].join('\n')
}

export function buildDecisionInput(decision: WorkflowDecision): string {
  const messages: Record<WorkflowDecision, string> = {
    'confirm-development': '我确认详细设计，可以开始开发。请按当前项目规则实现并自测。',
    'confirm-execution': '我确认当前修复/优化方案，可以执行修改。请按当前项目规则实现并自测。',
    commit: '我选择提交代码。请按当前项目提交规则先展示真实改动摘要和建议提交信息，确认条件满足后再提交；不要自动推送。',
    keep: '我选择保留当前改动，不执行任何 Git 提交或推送。请给出本次结论、改动和自测 recap。',
    adjust: '我想继续调整，请保持当前会话并等待我的补充要求，不要提交代码。',
  }
  return messages[decision]
}

export function normalizeInitialInput(input: string): string {
  return `${input.replace(/[\r\n]+$/g, '')}\r`
}
