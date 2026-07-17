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
    '你正在执行“既有项目后台非会话深度初始化”。不要创建聊天会话、不要等待用户输入，也不要输出内部工作流检查点。',
    `当前项目：${project.name}`,
    `项目路径：${project.path}`,
    '目标：一次性把当前项目的长期文档、规则和 skills 按真实代码完整建立，使以后开发、修 Bug、代码优化都先复用项目已有框架与公共能力，不走偏、不破坏历史行为。',
    '强制顺序（不得跳步）：',
    '- 第一步读取并执行 `.claude/skills/skill-designer/SKILL.md`。这是从 IPS 原样安装的 skill 设计规范；后续创建或修订任何 skill 都必须用它选择 Generator / Reviewer / Inversion / Pipeline / Tool Wrapper 模式，并按其 references 与 evals 自检。',
    '- 同时逐份读取 `.vibe-coding-platform/init-reference-v3/` 下命中当前代码层的文档、规则与 skill 模板。正式产物必须保持模板章节、表格和 Gate，并用当前项目真实代码填满；禁止把空模板、占位符或示例内容直接复制成长期文档。该临时参考包会在最终校验成功后由平台清理。',
    '- 完整读取项目根目录、全部前端/后端源码、构建与测试脚本、依赖清单、配置、已有 docs、`CLAUDE.md`、`AGENTS.md`、`.claude`、`.agents`；不得只扫少量文件或只读 README 就下结论。',
    '- 保留项目原有业务文档、规则和 skills；只允许替换带明确 `vibe-coding-platform` 旧初始化标识的空壳文件，绝不删除或覆盖真实项目资料。',
    '- 先识别前端、后端或全栈，再只生成命中代码层需要的目录；前端项目不得生成物理模型、后端 API、回调、数据库等后端文档。',
    '文档要求：',
    '- 检测到前端时，真实填充 `docs/frontend/MOC.md`、`docs/frontend/latest/index.md`、`docs/frontend/latest/业务/业务功能总览.md`、`docs/frontend/latest/系统架构/前端架构.md`、`docs/frontend/latest/公共能力/组件与公共能力.md`。',
    '- 检测到后端时，真实填充 `docs/backend/MOC.md`、`docs/backend/latest/index.md`、`docs/backend/latest/业务/业务功能总览.md`、`docs/backend/latest/系统架构/系统架构详解.md`。只有扫描到真实服务端路由（Controller / Router / Handler）时才生成 `docs/backend/latest/接口文档/API接口总览.md`；API 总览必须来自真实入口，不得把前端路由当成 API，也不得写“以后补充”的空表。',
    '- 只有代码或 schema/迁移脚本证明存在数据库时，才生成 `docs/backend/latest/接口文档/物理模型总览.md`；格式只包含“表清单（表名、中文名、用途）+ 每张表字段（字段、类型、是否为空、含义、备注）”。',
    '- 只有存在真实回调入口、跨边界枚举时，才分别生成 `回调接口总览.md`、`枚举值总览.md`；检测到真实第三方客户端或 SDK 调用时，生成 `docs/backend/latest/第三方集成/第三方集成总览.md`，并同步生成项目化第三方规则与 `external-integration` skill。条件不成立就不创建，禁止凑目录。',
    '- `latest/规范约束/详设文档模板.md`、`开发进度文档模板.md` 以及全栈项目的 `前端接入说明模板.md` 是长期保留的正式模板，允许保留模板占位符；它们已由平台在缺失时补入，必须完整保留其章节结构。其他长期文档不得留下 `{{占位符}}`、`待填写`、空表或泛化示例。',
    '- 版本号、任务序号、模块编号等依赖项目的编号必须写入正式模板约束：从当前项目发布/迭代事实取版本，扫描同一编号域内已有文件后按项目原位数递增并查重；详设、进度、前端接入和自测归档保持同一任务序号。禁止写死 `01`、复用已有编号、套用其他项目编号或凭空创建版本。',
    '规则要求：',
    '- 先从当前项目真实代码提取技术栈、分层、公共框架、工具类、错误体系、日志、权限、数据访问、异步、测试、提交与分支约定，再生成项目专属 `.claude/rules/README.md` 及按主题拆分的 rules。README 必须包含“代码 pattern/任务关键词 → 必读规则”映射。',
    '- 公共规则至少覆盖：先读后写、复用优先与影响面、事实与兜底边界、开发流程与文档同步、自测与交付；只有项目根目录真实存在 `.git` 时才加入 Git 协作与历史保护。前端/后端规则仅在对应代码层存在时生成。通用于所有项目的固定纪律直接采用成熟正文；依赖项目的路径、编号、框架、命令和公共能力必须从当前项目填实，不得留下模板字段。不得把通用模板原文不加分析地复制成项目规则。',
    'skills 要求：',
    '- 使用 skill-designer 生成并校验项目专属 `detail-design-writer`、`review-feedback-handler`、`developer`、`problem-diagnose`、`code-review`；前端另有 `frontend-self-test`，后端另有 `backend-self-test`。每个 skill 必须在 frontmatter 声明 `metadata.pattern`，并包含“项目资源”“执行流程”“完成 Gate”“失败处理”；其中的文档、规则、命令、框架、公共能力和同类示例必须来自当前项目真实路径，不能只是几十字说明。Worktree 是平台产品能力，不作为初始化生成或完成校验所需的项目 skill。',
    '- `skill-designer` 本身保持平台安装的原样内容，禁止改名、缩写或重写。不要生成运维、部署、监控或 CI 运维 skill。所有后端项目都生成项目专属 `backend-log-diagnose`，但只登记真实日志来源；检测到关系型数据库实体、迁移或 schema 时生成 `ddl-review`；检测到数据库连接配置时生成项目专属 `database-read-diagnose`，只记录配置键和文件路径，不记录密钥。数据库/日志只有安全只读自测确实成功时才标记“可用”，否则明确“有证据但需配置 / 不适用”，不得伪造连通。',
    '入口与共享要求：',
    '- `CLAUDE.md` 是唯一维护源，写入项目定位、真实模块链路、核心红线、开发流程、文档索引、触发规则、构建与自测命令；`AGENTS.md` 软链接到 `CLAUDE.md`。只有真实存在 `.git` 时才写 Git 协作、分支、提交与 worktree 内容。',
    '- `.claude/{rules,skills,scripts}` 是唯一维护源；`.agents/{rules,skills,scripts}` 分别软链接到 `.claude/` 同名目录。若已有双份内容，先合并保护信息再建立软链接。',
    '- 不创建运维、部署、监控或 CI 运维文档；开发所需的本地启动、构建、测试命令写入项目入口和对应开发/自测规则。',
    '- 任何事实必须引用源码路径、配置、测试或现有文档证据；没有证据就明确未知。禁止为了流程继续而添加吞异常、伪造默认值、模拟成功、自动降级或猜测配置等兜底逻辑。',
    '- 完成后逐项自检：长期文档有真实代码证据，正式模板结构完整，rules/skills 与项目技术栈和公共框架匹配，前后端/数据库目录按实际存在裁剪，skill-designer 与 IPS 标准版本一致，软链接正确，既有资料未被覆盖。',
    '完成上述扫描、生成和自检后直接退出；平台会读取并校验真实文件决定是否完成。',
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
