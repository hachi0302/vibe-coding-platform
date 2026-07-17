import type { Agent } from '../types'

export type WorkflowKind = 'onboarding' | 'feature' | 'bug' | 'optimization'

export type WorkflowPhase =
  | 'starting'
  | 'onboarding'
  | 'initialization-analyzed'
  | 'initialization-documented'
  | 'initialization-generated'
  | 'initialization-ready'
  | 'designing'
  | 'investigating'
  | 'optimizing'
  | 'awaiting-confirmation'
  | 'implementing'
  | 'testing'
  | 'awaiting-delivery-decision'
  | 'completed'

export type WorkflowDecision = 'confirm-development' | 'confirm-execution' | 'commit' | 'keep' | 'adjust'

export type WorkflowDecisionStage = 'design' | 'execution' | 'delivery'

export interface WorkflowProject {
  key: string
  name: string
  path: string
}

export interface WorkflowDraft {
  kind: WorkflowKind
  description: string
  attachments: string[]
  requireDesignConfirmation: boolean
}

export interface WorkflowRecord {
  id: string
  project: WorkflowProject
  draft: WorkflowDraft
  title: string
  phase: WorkflowPhase
  decisionStage?: WorkflowDecisionStage
  agent: Agent
  createdAt: number
  updatedAt: number
  notes: string[]
}
