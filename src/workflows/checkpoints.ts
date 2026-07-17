import type { WorkflowDecisionStage, WorkflowPhase } from './types'

export interface WorkflowCheckpoint {
  phase: WorkflowPhase
  decisionStage?: WorkflowDecisionStage
  note?: string
}

const phases = new Set<WorkflowPhase>([
  'starting',
  'onboarding',
  'initialization-analyzed',
  'initialization-documented',
  'initialization-generated',
  'initialization-ready',
  'designing',
  'investigating',
  'optimizing',
  'awaiting-confirmation',
  'implementing',
  'testing',
  'awaiting-delivery-decision',
  'completed',
])

const stages = new Set<WorkflowDecisionStage>(['design', 'execution', 'delivery'])

function asCheckpoint(value: unknown): WorkflowCheckpoint | undefined {
  if (!value || typeof value !== 'object') return undefined
  const candidate = value as Record<string, unknown>
  if (typeof candidate.phase !== 'string' || !phases.has(candidate.phase as WorkflowPhase)) return undefined
  const decisionStage = typeof candidate.decisionStage === 'string' && stages.has(candidate.decisionStage as WorkflowDecisionStage)
    ? candidate.decisionStage as WorkflowDecisionStage
    : undefined
  if (candidate.decisionStage !== undefined && !decisionStage) return undefined
  return {
    phase: candidate.phase as WorkflowPhase,
    ...(decisionStage ? { decisionStage } : {}),
    ...(typeof candidate.note === 'string' && candidate.note.trim() ? { note: candidate.note.trim() } : {}),
  }
}

export function parseWorkflowCheckpoints(output: string): WorkflowCheckpoint[] {
  return output
    .split(/\r?\n/)
    .flatMap((line) => {
      const match = line.match(/WORKFLOW_CHECKPOINT\s*:\s*(\{.*\})\s*$/)
      if (!match) return []
      try {
        const checkpoint = asCheckpoint(JSON.parse(match[1]))
        return checkpoint ? [checkpoint] : []
      } catch {
        return []
      }
    })
}
