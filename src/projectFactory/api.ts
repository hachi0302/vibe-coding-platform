import { invoke } from '@tauri-apps/api/core'
import { listen } from '@tauri-apps/api/event'
import type { AgentAnalysisPayload, AgentAnalysisProgress, CreateProjectRequest, CreateProjectResult, EnvCheckItem, ExistingProjectInitPreparation, ExistingProjectInitializationProgress, ExistingProjectInitResult, ExistingProjectInitStatus, RequirementContext, RequirementMaterialBundle } from './types'

export const checkEnvironment = (toolIds: string[]) =>
  invoke<EnvCheckItem[]>('project_factory_check_env', { toolIds })

export const installTool = (toolId: string) =>
  invoke<void>('project_factory_install_tool', { toolId })

export const analyzeWithAgent = (context: RequirementContext) =>
  invoke<AgentAnalysisPayload>('project_factory_analyze_with_agent', {
    request: {
      text: context.text,
      projectName: context.projectName,
      structurePreference: context.structurePreference,
      clarificationAnswers: context.clarificationAnswers,
    },
  })

export const listenAnalysisProgress = (handler: (progress: AgentAnalysisProgress) => void) =>
  listen<AgentAnalysisProgress>('project-factory://analysis-progress', event => handler(event.payload))

export const readRequirementMaterials = (path: string) =>
  invoke<RequirementMaterialBundle>('project_factory_read_requirement_materials', { path })

export const createProject = (request: CreateProjectRequest) =>
  invoke<CreateProjectResult>('project_factory_create_project', { request })

export const prepareExistingProjectInitialization = (projectPath: string) =>
  invoke<ExistingProjectInitPreparation>('project_factory_prepare_existing_project_initialization', { projectPath })

export const finalizeExistingProjectInitialization = (projectPath: string) =>
  invoke<ExistingProjectInitResult>('project_factory_finalize_existing_project_initialization', { projectPath })

export const existingProjectInitStatus = (projectPath: string) =>
  invoke<ExistingProjectInitStatus>('project_factory_existing_project_init_status', { projectPath })

export const initializeExistingProject = (projectPath: string, agent: 'claude' | 'codex', prompt: string) =>
  invoke<ExistingProjectInitResult>('project_factory_initialize_existing_project', {
    projectPath,
    agent,
    prompt,
  })

export const listenInitializationProgress = (
  handler: (progress: ExistingProjectInitializationProgress) => void,
) => listen<ExistingProjectInitializationProgress>(
  'project-factory://initialization-progress',
  event => handler(event.payload),
)
