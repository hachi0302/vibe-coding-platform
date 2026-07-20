import type {
  ExistingProjectArtifactTotals,
  ExistingProjectInitStatus,
  ExistingProjectInitializationPhase,
} from './types'

export type ProjectInitializationPhase = ExistingProjectInitializationPhase

export interface ProjectInitializationProgress {
  phase: ProjectInitializationPhase
  percent: number
  detail: string
  runId?: string
  attempt?: number
  sequence?: number
  recoverable?: boolean
}

const progressByPhase: Record<ProjectInitializationPhase, ProjectInitializationProgress> = {
  scan: { phase: 'scan', percent: 5, detail: '正在扫描项目结构与安全快照' },
  plan: { phase: 'plan', percent: 18, detail: '正在规划有证据支撑的项目产物' },
  documents: { phase: 'documents', percent: 34, detail: '正在生成项目文档' },
  rules: { phase: 'rules', percent: 50, detail: '正在生成项目规则' },
  skills: { phase: 'skills', percent: 64, detail: '正在生成项目专属 skills' },
  install: { phase: 'install', percent: 78, detail: '正在检查冲突并安装已验证产物' },
  verify: { phase: 'verify', percent: 90, detail: '正在校验安装结果与所有权清单' },
  complete: { phase: 'complete', percent: 100, detail: '初始化完成' },
  failed: { phase: 'failed', percent: 0, detail: '初始化失败，请根据错误提示修正后重试' },
  interrupted: { phase: 'interrupted', percent: 0, detail: '初始化已中断，可从上次有效节点继续' },
  conflict: { phase: 'conflict', percent: 0, detail: '检测到用户文件冲突，请处理后重试' },
}

export function initializationProgressFor(phase: ProjectInitializationPhase): ProjectInitializationProgress {
  return progressByPhase[phase]
}

export type ProjectInitializationAction = 'start' | 'resume' | 'attention' | 'complete'

export function initializationActionForStatus(status: ExistingProjectInitStatus): ProjectInitializationAction {
  if (status.status === 'current-v4') return 'complete'
  if (!status.status && status.initialized && status.markerVersion !== 'v3') return 'complete'
  if (status.status === 'needs-attention') return status.recoverable ? 'resume' : 'attention'
  if (status.status === 'incomplete' && status.runId && status.recoverable !== false) return 'resume'
  return 'start'
}

function statusPhase(status: ExistingProjectInitStatus): ProjectInitializationPhase {
  if (status.conflicts?.length) return 'conflict'
  if (status.phase && status.phase in progressByPhase) return status.phase
  if (status.status === 'current-v4') return 'complete'
  if (status.status === 'needs-attention') return 'failed'
  return 'scan'
}

export function initializationProgressFromStatus(
  status: ExistingProjectInitStatus,
): ProjectInitializationProgress {
  const phase = statusPhase(status)
  const fallback = initializationProgressFor(phase)
  return {
    ...fallback,
    percent: Math.max(0, Math.min(100, status.percent ?? fallback.percent)),
    detail: status.detail
      ?? status.conflicts?.[0]?.detail
      ?? status.issues?.[0]?.detail
      ?? fallback.detail,
    runId: status.runId,
    attempt: status.attempt,
    sequence: status.sequence,
    recoverable: status.recoverable,
  }
}

export function initializationCompletionDetail(totals?: ExistingProjectArtifactTotals): string {
  if (!totals) return '初始化完成。'
  const skillLabel = totals.skills === 1 ? 'skill' : 'skills'
  return `初始化完成：${totals.documents} 份文档、${totals.rules} 条规则、${totals.skills} 个 ${skillLabel} 已通过校验。`
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
  return !['complete', 'failed', 'interrupted', 'conflict'].includes(phase)
}

export function isInitializationTaskCardVisible(
  phase: ProjectInitializationPhase,
  minimized: boolean,
): boolean {
  return minimized && isInitializationTaskVisible(phase)
}

export const projectInitializationSteps = [
  { phase: 'scan', label: '扫描项目' },
  { phase: 'plan', label: '规划产物' },
  { phase: 'documents', label: '生成文档' },
  { phase: 'rules', label: '生成规则' },
  { phase: 'skills', label: '生成 skills' },
  { phase: 'install', label: '安全安装' },
  { phase: 'verify', label: '验证结果' },
  { phase: 'complete', label: '初始化完成' },
] as const
