<script setup lang="ts">
import { reactive, watch } from 'vue'
import AgentAnalysisProgressPanel from './AgentAnalysisProgressPanel.vue'
import type { AgentAnalysisProgress, ClarifyingQuestion, RecognizedConstraint } from '../../projectFactory/types'

const props = defineProps<{
  questions: ClarifyingQuestion[]
  recognizedConstraints?: RecognizedConstraint[]
  analyzing?: boolean
  progress?: AgentAnalysisProgress
  elapsedSeconds?: number
}>()
const emit = defineEmits<{
  (e: 'submit', answers: Record<string, string[]>): void
  (e: 'skip'): void
  (e: 'back'): void
}>()

const answers = reactive<Record<string, string[]>>({})

watch(() => props.questions, questions => {
  for (const key of Object.keys(answers)) delete answers[key]
  for (const question of questions) {
    const recommended = question.options
      .filter(option => option.recommended)
      .map(option => option.value)
    if (recommended.length) answers[question.id] = recommended
  }
}, { immediate: true })

function selectOption(question: ClarifyingQuestion, value: string) {
  if (question.selectionMode === 'multiple') {
    const values = answers[question.id] ?? []
    answers[question.id] = values.includes(value)
      ? values.filter(item => item !== value)
      : [...values, value]
    return
  }
  answers[question.id] = [value]
}

function submit() {
  emit('submit', Object.fromEntries(
    Object.entries(answers).filter(([, values]) => values.length > 0),
  ))
}
</script>

<template>
  <section class="pf-panel pf-question-panel">
    <div class="pf-panel-head">
      <div>
        <h2>补充几个关键信息</h2>
        <p>仅展示智能体无法确定、且会影响选型的决策。通常不超过 3 项，最多 10 项。</p>
      </div>
      <span class="pf-step">02 / 补充</span>
    </div>
    <div class="pf-question-list">
      <div v-if="recognizedConstraints?.length" class="pf-recognized-context">
        <strong>已从需求识别</strong>
        <div>
          <span v-for="constraint in recognizedConstraints" :key="constraint.id">{{ constraint.label }}：{{ constraint.value }}</span>
        </div>
      </div>
      <div v-for="question in questions" :key="question.id" class="pf-question">
        <div>
          <div class="pf-question-label">{{ question.label }}</div>
          <p v-if="question.description" class="pf-question-description">{{ question.description }}</p>
        </div>
        <div class="pf-segmented" role="group" :aria-label="question.label">
          <button
            v-for="option in question.options"
            :key="option.value"
            type="button"
            :data-option="`${question.id}:${option.value}`"
            :class="{ active: (answers[question.id] ?? []).includes(option.value) }"
            :disabled="analyzing"
            @click="selectOption(question, option.value)"
          >{{ option.label }}<small v-if="option.recommended">推荐</small></button>
        </div>
      </div>
    </div>
    <AgentAnalysisProgressPanel
      v-if="analyzing && progress"
      inline
      :progress="progress"
      :elapsed-seconds="elapsedSeconds ?? 0"
    />
    <div v-else class="pf-actions">
      <button type="button" class="pf-button secondary" @click="emit('back')">返回修改</button>
      <button type="button" class="pf-button text" @click="emit('skip')">采用推荐继续</button>
      <button type="button" class="pf-button primary" @click="submit">生成方案</button>
    </div>
  </section>
</template>
