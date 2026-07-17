import type { Agent } from '../types'
import type { WorkflowDecisionStage, WorkflowDraft, WorkflowPhase, WorkflowProject, WorkflowRecord } from './types'
import { workflowTitle } from './prompt'

const STORAGE_KEY = 'existing-project-workflows:v1'

function readAll(): WorkflowRecord[] {
  try {
    const raw = JSON.parse(localStorage.getItem(STORAGE_KEY) || '[]')
    return Array.isArray(raw) ? raw : []
  } catch {
    return []
  }
}

function writeAll(records: WorkflowRecord[]) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(records))
}

export function loadProjectWorkflows(projectKey: string): WorkflowRecord[] {
  return readAll().filter((record) => record.project.key === projectKey).sort((a, b) => b.updatedAt - a.updatedAt)
}

export function getWorkflow(projectKey: string, workflowId: string): WorkflowRecord | undefined {
  return readAll().find((record) => record.project.key === projectKey && record.id === workflowId)
}

export function saveWorkflow(record: WorkflowRecord) {
  const records = readAll()
  const index = records.findIndex((item) => item.id === record.id && item.project.key === record.project.key)
  if (index >= 0) records.splice(index, 1, record)
  else records.push(record)
  writeAll(records)
}

export function createWorkflow(project: WorkflowProject, draft: WorkflowDraft, agent: Agent): WorkflowRecord {
  const now = Date.now()
  const record: WorkflowRecord = {
    id: `wf-${now.toString(36)}-${Math.random().toString(36).slice(2, 8)}`,
    project,
    draft,
    title: workflowTitle(draft),
    phase: draft.kind === 'onboarding' ? 'onboarding' : draft.kind === 'bug' ? 'investigating' : draft.kind === 'optimization' ? 'optimizing' : 'designing',
    agent,
    createdAt: now,
    updatedAt: now,
    notes: [],
  }
  saveWorkflow(record)
  return record
}

export function setWorkflowPhase(
  projectKey: string,
  workflowId: string,
  phase: WorkflowPhase,
  note?: string,
  decisionStage?: WorkflowDecisionStage,
): WorkflowRecord | undefined {
  const records = readAll()
  const index = records.findIndex((item) => item.project.key === projectKey && item.id === workflowId)
  if (index < 0) return undefined
  const current = records[index]
  const updated: WorkflowRecord = {
    ...current,
    phase,
    ...(decisionStage ? { decisionStage } : { decisionStage: undefined }),
    updatedAt: Date.now(),
    ...(note ? { notes: [...current.notes, note] } : {}),
  }
  records.splice(index, 1, updated)
  writeAll(records)
  return updated
}

export function setWorkflowAgent(
  projectKey: string,
  workflowId: string,
  agent: WorkflowRecord['agent'],
  note?: string,
): WorkflowRecord | undefined {
  const records = readAll()
  const index = records.findIndex((item) => item.project.key === projectKey && item.id === workflowId)
  if (index < 0) return undefined
  const current = records[index]
  const updated: WorkflowRecord = {
    ...current,
    agent,
    updatedAt: Date.now(),
    ...(note ? { notes: [...current.notes, note] } : {}),
  }
  records.splice(index, 1, updated)
  writeAll(records)
  return updated
}

export function latestActiveWorkflow(projectKey: string): WorkflowRecord | undefined {
  return loadProjectWorkflows(projectKey).find((record) => record.phase !== 'completed')
}
