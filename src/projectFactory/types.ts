export type RequirementInputKind = 'text' | 'local'

export type SystemType =
  | 'web-h5'
  | 'admin'
  | 'mini-program'
  | 'app'
  | 'desktop'
  | 'backend-api'
  | 'fullstack'
  | 'cli'

export type ProjectScale = 'prototype' | 'small-production' | 'large-maintained'
export type Audience = 'internal-staff' | 'external-users' | 'merchant-customer' | 'developer'
export type TechPreference = 'none' | 'java' | 'node' | 'python' | 'vue' | 'react' | 'other'
export type InfrastructurePreference = 'existing-mysql' | 'existing-platform' | 'new-platform' | 'unknown'
export type ProductType = 'saas' | 'ecommerce' | 'consumer' | 'enterprise' | 'internal-system' | 'developer-tool' | 'ai-agent' | 'content' | 'api-platform' | 'other'
export type FrontendPreference = 'auto' | 'vue' | 'react' | 'nextjs' | 'none'
export type BackendPreference = 'auto' | 'java' | 'node' | 'python' | 'go' | 'rust' | 'dotnet' | 'none'
export type AgentCapability = 'none' | 'assistant' | 'rag' | 'workflow-agent'
export type ArchitecturePreference = 'auto' | 'modular-monolith' | 'frontend-backend' | 'microservices'
export type DeploymentPreference = 'auto' | 'single-host' | 'replicated' | 'kubernetes'
export type DataPlatformPreference = 'auto' | 'mysql' | 'postgresql' | 'mongodb' | 'sqlite'
export type CachePreference = 'auto' | 'none' | 'redis'
export type MessagingPreference = 'auto' | 'none' | 'rabbitmq' | 'kafka' | 'rocketmq'
export type ConfigurationPreference = 'auto' | 'none' | 'environment' | 'nacos' | 'apollo'
export type ProjectStructure = 'single-app' | 'frontend-backend' | 'monorepo'
export type StructurePreference = 'auto' | 'single-app' | 'frontend-backend'
export type AgentChoice = 'claude' | 'codex' | 'both'

export interface RequirementContext {
  text: string
  projectName?: string
  audience?: Audience
  scale?: ProjectScale
  preference?: TechPreference
  infrastructure?: InfrastructurePreference
  structurePreference?: StructurePreference
  productType?: ProductType
  frontendPreference?: FrontendPreference
  backendPreference?: BackendPreference
  agentCapability?: AgentCapability
  architecture?: ArchitecturePreference
  deployment?: DeploymentPreference
  dataPlatform?: DataPlatformPreference
  cachePreference?: CachePreference
  messagingPreference?: MessagingPreference
  configurationPreference?: ConfigurationPreference
  clarificationAnswers?: ClarificationAnswer[]
}

export interface RecognizedConstraint {
  id: string
  label: string
  value: string
}

export interface ClarificationAnswer {
  questionId: string
  values: string[]
}

export interface ProjectInput extends RequirementContext {
  kind: RequirementInputKind
  filePath?: string
  url?: string
}

export interface FeatureSet {
  seo: boolean
  mobileFirst: boolean
  auth: boolean
  fileUpload: boolean
  realtime: boolean
  paymentOrOrder: boolean
  adminConsole: boolean
  crossPlatform: boolean
  offline: boolean
}

export interface ProjectProfile {
  summary: string
  systemType: SystemType
  audience?: Audience
  scale?: ProjectScale
  preference?: TechPreference
  features: FeatureSet
}

export interface StackRecommendation {
  id: string
  title: string
  status: 'recommended' | 'alternative' | 'not-recommended'
  frontend: string[]
  backend: string[]
  database: string[]
  cache: string[]
  messaging: string[]
  decisions: TechnologyDecision[]
  structure: ProjectStructure
  packageManager?: 'pnpm' | 'npm' | 'maven' | 'gradle' | 'pip' | 'go' | 'cargo' | 'dotnet'
  reasons: string[]
  tradeoffs: string[]
  preferenceMatched: boolean
}

export type TechnologyDecisionStatus = 'adopt' | 'optional' | 'defer' | 'not-needed'
export type TechnologyProvision = 'project' | 'external-platform' | 'existing-platform' | 'not-applicable'

export interface TechnologyDecision {
  category: string
  title: string
  status: TechnologyDecisionStatus
  choices: string[]
  reason: string
  provision: TechnologyProvision
  trigger?: string
}

export interface StackRecommendationResult {
  profile: ProjectProfile
  recommended: StackRecommendation
  alternatives: StackRecommendation[]
  notRecommended: StackRecommendation[]
  provider?: 'codex' | 'claude'
  assumptions?: string[]
  projectName: string
  projectNameReason: string
}

export interface AgentAnalysisPayload {
  provider: 'codex' | 'claude'
  recommended: Omit<StackRecommendation, 'status'>
  alternatives: Array<Omit<StackRecommendation, 'status'>>
  notRecommended: Array<Omit<StackRecommendation, 'status'>>
  assumptions: string[]
  projectName: string
  projectNameReason: string
  recognizedConstraints: RecognizedConstraint[]
  clarifyingQuestions: ClarifyingQuestion[]
}

export type AgentAnalysisPhase = 'prepare' | 'codex' | 'claude' | 'validate'

export interface AgentAnalysisProgress {
  phase: AgentAnalysisPhase
  percent: number
  detail: string
}

export type ClarificationSelectionMode = 'single' | 'multiple'

export interface ClarifyingOption {
  value: string
  label: string
  description?: string
  recommended?: boolean
}

export interface ClarifyingQuestion {
  id: string
  label: string
  description?: string
  selectionMode?: ClarificationSelectionMode
  options: ClarifyingOption[]
}

export interface NormalizedRequirement {
  sourceKind: RequirementInputKind
  sourceLabel: string
  text: string
  warnings: string[]
}

export interface RequirementMaterialFile {
  relativePath: string
  absolutePath: string
  kind: string
  included: boolean
  detail: string
}

export interface RequirementMaterialBundle {
  rootPath: string
  sourceLabel: string
  text: string
  files: RequirementMaterialFile[]
  warnings: string[]
}

export interface EnvCheckItem {
  toolId: string
  label: string
  required: boolean
  installed: boolean
  compatible: boolean
  version?: string
  detail?: string
}

export interface ProjectPreview {
  projectName: string
  parentPath: string
  targetPaths: Array<{ label: string; path: string }>
  directories: string[]
  files: string[]
  agentFiles: string[]
  agentMode: 'claude' | 'codex' | 'symlink'
}

export interface CreateProjectRequest {
  projectName: string
  parentPath: string
  frontendProjectName?: string
  backendProjectName?: string
  conciseRequirement: string
  recognizedConstraints: RecognizedConstraint[]
  assumptions: string[]
  recommendation: StackRecommendation
  profile: ProjectProfile
  agentChoice: AgentChoice
}

export interface CreateProjectResult {
  projectPaths: string[]
  agentMode: 'claude' | 'codex' | 'symlink' | 'copy'
  message: string
  verification: {
    status: 'passed' | 'failed' | 'skipped' | 'pending'
    checks: string[]
    detail: string
  }
}

export interface ExistingProjectInitResult {
  projectPath: string
  layers: {
    frontend: boolean
    backend: boolean
  }
  detectedStack: string[]
  generated: string[]
}

export interface ExistingProjectInitPreparation {
  projectPath: string
  layers: {
    frontend: boolean
    backend: boolean
  }
  detectedStack: string[]
  existingDocs: string[]
  existingAgentMaterial: string[]
}

export interface ExistingProjectInitStatus {
  initialized: boolean
  markerVersion?: string
}

export interface ExistingProjectInitializationProgress {
  projectPath: string
  phase: 'analyze' | 'documents' | 'rules' | 'validate' | 'complete' | 'failed'
  percent: number
  detail: string
}
