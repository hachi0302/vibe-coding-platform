<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, ref } from 'vue'
import { open } from '@tauri-apps/plugin-dialog'
import RequirementInputPanel from '../components/projectFactory/RequirementInputPanel.vue'
import RecommendationPanel from '../components/projectFactory/RecommendationPanel.vue'
import EnvironmentPanel from '../components/projectFactory/EnvironmentPanel.vue'
import ProjectPreviewPanel from '../components/projectFactory/ProjectPreviewPanel.vue'
import CreateResultPanel from '../components/projectFactory/CreateResultPanel.vue'
import AgentAnalysisProgressPanel from '../components/projectFactory/AgentAnalysisProgressPanel.vue'
import { toStackRecommendationResult } from '../projectFactory/agentAnalysis'
import { toolsForRecommendation } from '../projectFactory/envLabels'
import { buildPreview } from '../projectFactory/previewBuilder'
import { analyzeWithAgent, checkEnvironment, createProject, installTool, listenAnalysisProgress, readRequirementMaterials } from '../projectFactory/api'
import type {
  AgentChoice,
  AgentAnalysisPayload,
  AgentAnalysisProgress,
  CreateProjectResult,
  EnvCheckItem,
  RequirementContext,
  RequirementInputKind,
  RequirementMaterialBundle,
  StackRecommendation,
  StackRecommendationResult,
  StructurePreference,
} from '../projectFactory/types'
import type { BackgroundTaskSummary } from '../projectFactory/backgroundTask'

type Stage = 'input' | 'analyzing' | 'recommendation' | 'environment' | 'preview' | 'creating' | 'result'

const emit = defineEmits<{
  (e: 'open-path', path: string): void
  (e: 'task-progress', task: BackgroundTaskSummary): void
  (e: 'task-finished'): void
  (e: 'minimize-analysis'): void
}>()

const stage = ref<Stage>('input')
const kind = ref<RequirementInputKind>('text')
const projectName = ref('')
const frontendProjectName = ref('')
const backendProjectName = ref('')
const text = ref('')
const sourceValue = ref('')
const material = ref<RequirementMaterialBundle>()
const materialLoading = ref(false)
const followUp = ref('')
const structurePreference = ref<StructurePreference>('auto')
const context = ref<RequirementContext>({ text: '' })
const analysisPayload = ref<AgentAnalysisPayload>()
const recommendation = ref<StackRecommendationResult>()
const selectedRecommendation = ref<StackRecommendation>()
const environment = ref<EnvCheckItem[]>([])
const environmentLoading = ref(false)
const environmentError = ref('')
const installingToolId = ref('')
const parentPath = ref('')
const agentChoice = ref<AgentChoice>('both')
const createResult = ref<CreateProjectResult>()
const createError = ref('')
const analysisLoading = ref(false)
const analysisError = ref('')
const analysisElapsedSeconds = ref(0)
const analysisProgress = ref<AgentAnalysisProgress>({
  phase: 'prepare', percent: 0, detail: '正在整理需求与已有约束',
})
let stopAnalysisTimer: ReturnType<typeof setInterval> | undefined
let unlistenAnalysisProgress: (() => void) | undefined

function publishAnalysisProgress() {
  emit('task-progress', {
    kind: 'analysis',
    title: '技术方案分析中',
    detail: analysisProgress.value.detail,
    percent: analysisProgress.value.percent,
    elapsedSeconds: analysisElapsedSeconds.value,
  })
}

const externalServices = computed(() => {
  const active = selectedRecommendation.value ?? recommendation.value?.recommended
  if (!active) return []
  return active.decisions.filter(decision => decision.status === 'adopt' && (decision.provision === 'external-platform' || decision.provision === 'existing-platform'))
})

const preview = computed(() => {
  const active = selectedRecommendation.value ?? recommendation.value?.recommended
  if (!recommendation.value || !active) return null
  return buildPreview({
    projectName: projectName.value.trim(),
    parentPath: parentPath.value.trim() || '未选择路径',
    frontendProjectName: frontendProjectName.value.trim(),
    backendProjectName: backendProjectName.value.trim(),
    conciseRequirement: confirmedRequirementSnapshot(),
    recognizedConstraints: analysisPayload.value?.recognizedConstraints ?? [],
    assumptions: analysisPayload.value?.assumptions ?? [],
    recommendation: active,
    profile: recommendation.value.profile,
    agentChoice: agentChoice.value,
  })
})

function buildContext(): RequirementContext {
  const inputText = kind.value === 'text'
    ? text.value.trim()
    : material.value?.text.trim() ?? ''
  return {
    text: inputText,
    structurePreference: structurePreference.value,
  }
}

function compactText(value: string, maxLength = 600) {
  const compact = value.replace(/\s+/g, ' ').trim()
  return compact.length > maxLength ? `${compact.slice(0, maxLength).trimEnd()}…` : compact
}

function confirmedRequirementSnapshot() {
  const payload = analysisPayload.value
  const active = selectedRecommendation.value ?? recommendation.value?.recommended
  if (!payload || !active) return ''

  const recognizedSummary = payload.recognizedConstraints
    .map(item => `${item.label}：${compactText(item.value, 180)}`)
    .filter(Boolean)
    .join('；')
  if (recognizedSummary) return compactText(recognizedSummary)

  // 直接描述可用原始输入的精简版兜底；本机资料绝不把文件正文写入创建请求。
  if (kind.value === 'text') {
    const directSummary = compactText(context.value.text)
    if (directSummary) return directSummary
  }

  const reasonSummary = active.reasons.slice(0, 3).map(item => compactText(item, 180)).join('；')
  return compactText(`${payload.projectName}：${reasonSummary || active.title}`)
}

async function analyze() {
  context.value = buildContext()
  analysisError.value = ''
  await requestAnalysis(context.value)
}

function storeAnalysis(payload: AgentAnalysisPayload, next: RequirementContext) {
  analysisPayload.value = payload
  context.value = next
  recommendation.value = toStackRecommendationResult(payload, next)
  selectedRecommendation.value = recommendation.value.recommended
  projectName.value = recommendation.value.projectName
  frontendProjectName.value = ''
  backendProjectName.value = ''
  stage.value = 'input'
}

async function requestAnalysis(next: RequirementContext) {
  stage.value = 'analyzing'
  analysisLoading.value = true
  analysisError.value = ''
  analysisElapsedSeconds.value = 0
  analysisProgress.value = { phase: 'prepare', percent: 8, detail: '正在整理需求与已有约束' }
  publishAnalysisProgress()
  stopAnalysisTimer = setInterval(() => {
    analysisElapsedSeconds.value += 1
    publishAnalysisProgress()
  }, 1000)
  try {
    await nextTick()
    await new Promise<void>(resolve => window.setTimeout(resolve, 40))
    unlistenAnalysisProgress = await listenAnalysisProgress(progress => {
      analysisProgress.value = progress
      publishAnalysisProgress()
    })
    const payload = await analyzeWithAgent(next)
    storeAnalysis(payload, next)
  } catch (error) {
    analysisError.value = `智能体分析失败：${String(error)}`
    stage.value = 'input'
  } finally {
    analysisLoading.value = false
    if (stopAnalysisTimer) clearInterval(stopAnalysisTimer)
    stopAnalysisTimer = undefined
    unlistenAnalysisProgress?.()
    unlistenAnalysisProgress = undefined
    emit('task-finished')
  }
}

async function reanalyze() {
  const feedback = followUp.value.trim()
  if (!feedback) return
  await requestAnalysis({
    ...context.value,
    text: `${context.value.text}\n\n## 用户补充、纠正或追问\n${feedback}`,
    clarificationAnswers: [{ questionId: 'user-follow-up', values: [feedback] }],
  })
  followUp.value = ''
}

function confirmAnalysis() {
  if (!analysisPayload.value || !recommendation.value) return
  stage.value = 'recommendation'
}

async function loadMaterial(path: string) {
  materialLoading.value = true
  analysisError.value = ''
  try {
    sourceValue.value = path
    material.value = await readRequirementMaterials(path)
    analysisPayload.value = undefined
    recommendation.value = undefined
  } catch (error) {
    material.value = undefined
    analysisError.value = `读取本机资料失败：${String(error)}`
  } finally {
    materialLoading.value = false
  }
}

async function chooseFile() {
  const selected = await open({ directory: false, multiple: false })
  if (typeof selected === 'string') await loadMaterial(selected)
}

async function chooseFolder() {
  const selected = await open({ directory: true, multiple: false })
  if (typeof selected === 'string') await loadMaterial(selected)
}

async function chooseParentPath() {
  const selected = await open({ directory: true, multiple: false })
  if (typeof selected === 'string') parentPath.value = selected
}

async function loadEnvironment() {
  const active = selectedRecommendation.value ?? recommendation.value?.recommended
  if (!active) return
  environmentLoading.value = true
  environmentError.value = ''
  try {
    const tools = toolsForRecommendation(active)
    environment.value = await checkEnvironment(tools.map(tool => tool.toolId))
  } catch (error) {
    environment.value = []
    environmentError.value = `环境检查失败：${String(error)}`
  } finally {
    environmentLoading.value = false
  }
}

function selectRecommendation(value: StackRecommendation) {
  selectedRecommendation.value = value
}

async function useRecommendation(value: StackRecommendation) {
  selectedRecommendation.value = value
  stage.value = 'environment'
  await loadEnvironment()
}

async function install(toolId: string) {
  installingToolId.value = toolId
  environmentError.value = ''
  try {
    await installTool(toolId)
    await loadEnvironment()
  } catch (error) {
    environmentError.value = `安装失败：${String(error)}`
  } finally {
    installingToolId.value = ''
  }
}

async function create() {
  const active = selectedRecommendation.value ?? recommendation.value?.recommended
  if (!recommendation.value || !analysisPayload.value || !active) return
  const conciseRequirement = confirmedRequirementSnapshot()
  stage.value = 'creating'
  createError.value = ''
  try {
    createResult.value = await createProject({
      projectName: projectName.value.trim(),
      parentPath: parentPath.value.trim(),
      frontendProjectName: frontendProjectName.value.trim() || undefined,
      backendProjectName: backendProjectName.value.trim() || undefined,
      conciseRequirement,
      recognizedConstraints: analysisPayload.value.recognizedConstraints,
      assumptions: analysisPayload.value.assumptions,
      recommendation: active,
      profile: { ...recommendation.value.profile, summary: conciseRequirement },
      agentChoice: agentChoice.value,
    })
  } catch (error) {
    createResult.value = undefined
    createError.value = String(error)
  } finally {
    stage.value = 'result'
  }
}

onBeforeUnmount(() => {
  if (stopAnalysisTimer) clearInterval(stopAnalysisTimer)
  unlistenAnalysisProgress?.()
})
</script>

<template>
  <div class="project-factory-view">
    <RequirementInputPanel
      v-if="stage === 'input'"
      v-model:kind="kind"
      v-model:text="text"
      v-model:source-value="sourceValue"
      v-model:structure-preference="structurePreference"
      v-model:follow-up="followUp"
      :material="material"
      :material-loading="materialLoading"
      :analysis="analysisPayload"
      :busy="analysisLoading"
      :error="analysisError"
      @choose-file="chooseFile"
      @choose-folder="chooseFolder"
      @analyze="analyze"
      @reanalyze="reanalyze"
      @confirm-analysis="confirmAnalysis"
    />
    <AgentAnalysisProgressPanel
      v-else-if="stage === 'analyzing'"
      :progress="analysisProgress"
      :elapsed-seconds="analysisElapsedSeconds"
      minimizable
      @minimize="emit('minimize-analysis')"
    />
    <RecommendationPanel
      v-else-if="stage === 'recommendation' && recommendation"
      :result="recommendation"
      :selected-id="selectedRecommendation?.id"
      @back="stage = 'input'"
      @select="selectRecommendation"
      @use="useRecommendation"
    />
    <EnvironmentPanel
      v-else-if="stage === 'environment'"
      :items="environment"
      :services="externalServices"
      :loading="environmentLoading"
      :installing-tool-id="installingToolId"
      :error="environmentError"
      @back="stage = 'recommendation'"
      @refresh="loadEnvironment"
      @install="install"
      @continue="stage = 'preview'"
    />
    <ProjectPreviewPanel
      v-else-if="(stage === 'preview' || stage === 'creating') && preview && recommendation"
      v-model:project-name="projectName"
      v-model:parent-path="parentPath"
      v-model:frontend-project-name="frontendProjectName"
      v-model:backend-project-name="backendProjectName"
      v-model:agent-choice="agentChoice"
      :preview="preview"
      :recommendation="selectedRecommendation ?? recommendation.recommended"
      :creating="stage === 'creating'"
      :error="createError"
      @back="stage = 'environment'"
      @choose-path="chooseParentPath"
      @create="create"
    />
    <CreateResultPanel
      v-else-if="stage === 'result'"
      :result="createResult"
      :error="createError"
      @open="($event) => $emit('open-path', $event)"
      @back="stage = 'preview'"
    />
  </div>
</template>

<style>
.project-factory-view { height: 100%; overflow: auto; padding: 24px; background: var(--bg); }
.pf-panel { width: min(920px, 100%); margin: 0 auto; padding: 22px; border: 1px solid var(--border); border-radius: 8px; background: var(--surface); }
.pf-panel-head { display: flex; align-items: flex-start; justify-content: space-between; gap: 20px; margin-bottom: 22px; padding-bottom: 16px; border-bottom: 1px solid var(--border); }
.pf-panel h2, .pf-panel h3, .pf-panel p { margin: 0; }
.pf-panel h2 { font-size: 18px; line-height: 1.3; letter-spacing: 0; }
.pf-panel h3 { margin-top: 5px; font-size: 15px; letter-spacing: 0; }
.pf-panel-head p, .pf-recommendation p, .pf-question legend { margin-top: 6px; color: var(--text-dim); line-height: 1.6; }
.pf-step, .pf-rec-label { color: var(--text-mute); font-size: 12px; white-space: nowrap; }
.pf-field { display: flex; flex-direction: column; gap: 7px; margin: 0 0 18px; color: var(--text-dim); font-size: 12px; }
.pf-field input, .pf-field textarea { width: 100%; border: 1px solid var(--border); border-radius: 6px; outline: 0; background: var(--surface-2); color: var(--text); font: inherit; padding: 9px 10px; resize: vertical; }
.pf-field input:focus, .pf-field textarea:focus { border-color: var(--brand); }
.pf-field small { color: var(--text-mute); line-height: 1.45; }
.pf-file-row { display: flex; gap: 8px; }
.pf-file-row input { min-width: 0; }
.pf-segmented { display: flex; flex-wrap: wrap; gap: 5px; }
.pf-segmented button { padding: 6px 9px; border: 1px solid var(--border); border-radius: 5px; color: var(--text-dim); background: var(--surface-2); font-size: 12px; }
.pf-segmented button:hover, .pf-segmented button.active { border-color: var(--brand); color: var(--text); background: var(--brand-soft); }
.pf-actions { display: flex; justify-content: flex-end; align-items: center; gap: 8px; margin-top: 22px; }
.pf-button { min-height: 30px; padding: 0 11px; border-radius: 6px; font-size: 12px; transition: background .12s, color .12s, border-color .12s; }
.pf-button:disabled { opacity: .45; cursor: not-allowed; }
.pf-button.primary { background: var(--brand); color: #fff; }
.pf-button.primary:hover:not(:disabled) { filter: brightness(1.08); }
.pf-button.secondary { border: 1px solid var(--border); background: var(--surface-2); color: var(--text); }
.pf-button.secondary:hover:not(:disabled) { background: var(--surface-hover); }
.pf-button.text { color: var(--text-dim); }
.pf-button.text:hover:not(:disabled) { color: var(--text); }
.pf-button.compact { min-height: 26px; padding: 0 8px; }
.pf-question-list { display: grid; gap: 18px; }
.pf-question { margin: 0; padding: 0; border: 0; }
.pf-question legend { padding: 0; font-size: 13px; color: var(--text); }
.pf-recommendation { padding: 14px; border: 1px solid var(--border); border-radius: 7px; background: var(--surface-2); }
.pf-recommendation.primary { border-color: var(--brand); }
.pf-recommendation ul { margin: 10px 0 0; padding-left: 18px; color: var(--text-dim); font-size: 13px; line-height: 1.6; }
.pf-stack-line { display: flex; flex-wrap: wrap; gap: 6px; margin-top: 12px; }
.pf-stack-line span { padding: 3px 6px; border-radius: 4px; background: var(--surface-hover); color: var(--text-dim); font-size: 11px; }
.pf-preference-note { margin-top: 10px !important; color: var(--brand) !important; font-size: 12px; }
.pf-alternative-list { display: grid; gap: 7px; margin-top: 20px; }
.pf-muted-line { margin-top: 15px; color: var(--text-mute); font-size: 12px; }
.pf-env-list { border-top: 1px solid var(--border); }
.pf-env-item { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 54px; border-bottom: 1px solid var(--border); }
.pf-env-item > div { display: grid; gap: 3px; }
.pf-env-item strong { font-size: 13px; font-weight: 500; }
.pf-env-item span { color: var(--text-mute); font-size: 12px; }
.pf-ok { color: var(--brand) !important; }
.pf-empty { padding: 20px 0; color: var(--text-mute); font-size: 13px; text-align: center; }
.pf-error { margin: 0 0 14px !important; padding: 9px 10px; border: 1px solid var(--danger); border-radius: 6px; background: var(--danger-soft); color: var(--danger); font-size: 12px; line-height: 1.45; }
.pf-preview-grid { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 10px; }
.pf-preview-grid > div { display: flex; flex-direction: column; gap: 7px; min-height: 86px; padding: 12px; border: 1px solid var(--border); border-radius: 6px; background: var(--surface-2); }
.pf-preview-grid span { color: var(--text-mute); font-size: 11px; }
.pf-preview-grid strong { color: var(--text); font-size: 13px; font-weight: 500; }
.pf-preview-grid code, .pf-result-path { overflow-wrap: anywhere; color: var(--text-dim); font: 12px/1.5 ui-monospace, SFMono-Regular, Menlo, monospace; }
.pf-result-panel { text-align: center; padding: 58px 22px; }
.pf-result-mark { display: inline-flex; padding: 5px 9px; border-radius: 5px; background: var(--brand-soft); color: var(--brand); font-size: 12px; }
.pf-result-mark.danger { background: var(--danger-soft); color: var(--danger); }
.pf-result-panel h2 { margin-top: 14px; }
.pf-result-panel p { margin-top: 8px; color: var(--text-dim); }
.pf-result-path { display: block; margin: 16px auto 0; max-width: 620px; padding: 9px; background: var(--surface-2); border: 1px solid var(--border); border-radius: 6px; }
.pf-result-panel .pf-actions { justify-content: center; }
@media (max-width: 720px) { .project-factory-view { padding: 12px; } .pf-panel { padding: 16px; } .pf-panel-head { display: block; } .pf-step { display: inline-block; margin-top: 8px; } .pf-preview-grid { grid-template-columns: 1fr; } .pf-file-row { flex-direction: column; } }
</style>
<style src="../projectFactory/style.css"></style>
