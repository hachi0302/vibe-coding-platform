<script setup lang="ts">
import { computed } from 'vue'
import type { AgentAnalysisProgress } from '../../projectFactory/types'
import { IconMinimize } from '../icons'

export interface ProgressStep {
  phase: string
  label: string
}

interface SharedProgress {
  phase: string
  percent: number
  detail: string
}

const props = defineProps<{
  progress: AgentAnalysisProgress | SharedProgress
  elapsedSeconds: number
  inline?: boolean
  steps?: readonly ProgressStep[]
  title?: string
  description?: string
  stepLabel?: string
  note?: string
  progressLabel?: string
  minimizable?: boolean
}>()

defineEmits<{ (e: 'minimize'): void }>()

const defaultSteps: ProgressStep[] = [
  { phase: 'prepare', label: '整理需求' },
  { phase: 'codex', label: '分析方案' },
  { phase: 'claude', label: '生成推荐' },
  { phase: 'validate', label: '校验结果' },
]

const steps = computed(() => props.steps?.length ? props.steps : defaultSteps)
const activeIndex = computed(() => {
  const index = steps.value.findIndex(step => step.phase === props.progress.phase)
  return index >= 0 ? index : props.progress.percent >= 100 ? steps.value.length : 0
})
const defaultStatusText = {
  prepare: '正在整理需求与已有约束',
  codex: '正在比较候选技术方案',
  claude: '正在生成候选技术方案',
  validate: '正在校验推荐结果',
}
const statusText = computed(() => props.progress.detail || defaultStatusText[props.progress.phase as keyof typeof defaultStatusText] || '正在处理')
</script>

<template>
  <section :class="['pf-analysis-panel', { 'pf-panel': !inline, 'pf-analysis-inline': inline }]" aria-live="polite">
    <div v-if="!inline" class="pf-panel-head">
      <div>
        <h2>{{ title ?? '正在分析技术方案' }}</h2>
        <p>{{ description ?? '正在读取项目约束并比较可创建的技术模板，请保持此页面打开。' }}</p>
      </div>
      <div class="pf-progress-head-actions">
        <span class="pf-step">{{ stepLabel ?? '02 / 分析' }}</span>
        <button
          v-if="minimizable"
          type="button"
          class="pf-minimize-progress"
          data-testid="minimize-progress"
          aria-label="缩小到后台"
          title="缩小到后台"
          @click="$emit('minimize')"
        >
          <IconMinimize />
          <span>缩小</span>
        </button>
      </div>
    </div>

    <div class="pf-analysis-body">
      <div class="pf-analysis-status">
        <span class="pf-analysis-indicator" aria-hidden="true" />
        <div>
          <strong>{{ statusText }}</strong>
          <span>已用时 {{ elapsedSeconds }} 秒</span>
        </div>
      </div>

      <div class="pf-progress-track" role="progressbar" :aria-label="progressLabel ?? '技术方案分析进度'" :aria-valuenow="progress.percent" aria-valuemin="0" aria-valuemax="100">
        <span data-testid="analysis-progress-fill" :style="{ width: `${Math.min(100, Math.max(0, progress.percent))}%` }" />
      </div>

      <ol class="pf-analysis-steps">
        <li v-for="(step, index) in steps" :key="step.phase" :class="{ active: index === activeIndex, completed: index < activeIndex }">
          <i>{{ index + 1 }}</i>
          <span>{{ step.label }}</span>
        </li>
      </ol>

      <p class="pf-analysis-note">{{ note ?? '分析过程不会修改项目文件，完成后会展示可采用、可选和后续引入的技术决策。' }}</p>
    </div>
  </section>
</template>
