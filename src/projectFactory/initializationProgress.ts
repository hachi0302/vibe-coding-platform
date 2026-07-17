export type ProjectInitializationPhase = 'analyze' | 'documents' | 'rules' | 'validate' | 'complete' | 'failed'

export interface ProjectInitializationProgress {
  phase: ProjectInitializationPhase
  percent: number
  detail: string
}

const progressByPhase: Record<ProjectInitializationPhase, ProjectInitializationProgress> = {
  analyze: { phase: 'analyze', percent: 8, detail: '正在分析项目代码、配置与已有资料' },
  documents: { phase: 'documents', percent: 32, detail: '正在生成并填充项目文档' },
  rules: { phase: 'rules', percent: 62, detail: '正在生成项目规则与 skills' },
  validate: { phase: 'validate', percent: 88, detail: '正在校验生成结果与平台初始化标识' },
  complete: { phase: 'complete', percent: 100, detail: '初始化完成' },
  failed: { phase: 'failed', percent: 0, detail: '初始化失败，请根据错误提示修正后重试' },
}

export function initializationProgressFor(phase: ProjectInitializationPhase): ProjectInitializationProgress {
  return progressByPhase[phase]
}

export function initializationAgentGuardMessage(agent: string): string | null {
  return isProjectInitializationAgent(agent)
    ? null
    : '项目初始化仅支持选择 Claude 或 Codex。'
}

export function isProjectInitializationAgent(agent: string): agent is 'claude' | 'codex' {
  return agent === 'claude' || agent === 'codex'
}

export function isInitializationTaskVisible(phase: ProjectInitializationPhase): boolean {
  return phase !== 'complete' && phase !== 'failed'
}

export const projectInitializationSteps = [
  { phase: 'analyze', label: '分析项目' },
  { phase: 'documents', label: '生成文档' },
  { phase: 'rules', label: '生成规则与 skills' },
  { phase: 'validate', label: '校验完成' },
] as const
