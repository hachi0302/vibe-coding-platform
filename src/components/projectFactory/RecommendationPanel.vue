<script setup lang="ts">
import { computed } from 'vue'
import type { StackRecommendation, StackRecommendationResult, TechnologyDecision } from '../../projectFactory/types'

const props = defineProps<{
  result: StackRecommendationResult
  selectedId?: string
}>()
const emit = defineEmits<{
  (e: 'select', value: StackRecommendation): void
  (e: 'use', value: StackRecommendation): void
  (e: 'back'): void
}>()

const selected = computed(() =>
  [props.result.recommended, ...props.result.alternatives].find(item => item.id === props.selectedId)
  ?? props.result.recommended,
)

const decisionGroups = computed(() => {
  const decisions = selected.value.decisions
  const groups = [
    { title: '前端应用', categories: ['frontend'] },
    { title: '业务后端', categories: ['business-backend', 'runtime', 'data-access'] },
    { title: 'Agent 服务', categories: ['agent'] },
    { title: '数据与基础设施', categories: ['persistence', 'cache', 'messaging', 'configuration'] },
    { title: '部署与工程化', categories: ['architecture', 'deployment', 'observability', 'engineering'] },
  ]
  return groups.map(group => ({
    ...group,
    decisions: decisions.filter(decision => group.categories.includes(decision.category)),
  })).filter(group => group.decisions.length)
})

function select(item: StackRecommendation) {
  emit('select', item)
}

function decisionStatus(decision: TechnologyDecision) {
  return { adopt: '采用', optional: '可选', defer: '后续引入', 'not-needed': '当前不需要' }[decision.status]
}
</script>

<template>
  <section class="pf-panel pf-recommendation-panel">
    <div class="pf-panel-head">
      <div>
        <h2>推荐技术方案</h2>
        <p>根据需求、已有基础设施与技术偏好生成；每项都标明当前决策和引入条件。</p>
      </div>
      <span class="pf-step">03 / 选型</span>
    </div>
    <article
      class="pf-recommendation primary pf-select-card"
      :class="{ selected: selected.id === result.recommended.id }"
      role="button"
      tabindex="0"
      :aria-pressed="selected.id === result.recommended.id"
      @click="select(result.recommended)"
      @keydown.enter.prevent="select(result.recommended)"
      @keydown.space.prevent="select(result.recommended)"
    >
      <div class="pf-rec-label">{{ selected.id === result.recommended.id ? '当前选择 · 系统推荐' : '系统推荐' }}</div>
      <h3>{{ result.recommended.title }}</h3>
      <div class="pf-stack-line">
        <span v-for="item in [...result.recommended.frontend, ...result.recommended.backend, ...result.recommended.database]" :key="item">{{ item }}</span>
      </div>
      <ul><li v-for="reason in result.recommended.reasons" :key="reason">{{ reason }}</li></ul>
      <p v-if="result.recommended.preferenceMatched" class="pf-preference-note">已优先采用你的技术偏好。</p>
    </article>

    <p v-if="result.assumptions?.length" class="pf-analysis-assumptions">基于当前信息：{{ result.assumptions.join('；') }}</p>

    <section class="pf-decision-summary" aria-label="技术选型明细">
      <section v-for="group in decisionGroups" :key="group.title" class="pf-decision-group pf-decision-card">
        <div class="pf-decision-head">
          <h3>{{ group.title }}</h3>
          <span>系统已完成本模块决策</span>
        </div>
        <article v-for="decision in group.decisions" :key="decision.category" class="pf-decision-row">
          <div class="pf-decision-topline">
            <div class="pf-decision-title">
              <span>{{ decision.title }}</span>
              <strong>{{ decision.choices.join('、') || '首期不引入' }}</strong>
            </div>
            <em class="pf-decision-status" :class="decision.status">{{ decisionStatus(decision) }}</em>
          </div>
          <p class="pf-decision-reason"><span>为什么这样选</span>{{ decision.reason }}</p>
          <p v-if="decision.trigger" class="pf-decision-trigger"><span>后续考虑条件</span>{{ decision.trigger }}</p>
        </article>
      </section>
    </section>

    <div v-if="result.alternatives.length" class="pf-alternative-list">
      <div class="pf-rec-label">备选方案</div>
      <article
        v-for="item in result.alternatives"
        :key="item.id"
        class="pf-recommendation pf-select-card"
        :class="{ selected: selected.id === item.id }"
        role="button"
        tabindex="0"
        :aria-pressed="selected.id === item.id"
        @click="select(item)"
        @keydown.enter.prevent="select(item)"
        @keydown.space.prevent="select(item)"
      >
        <div v-if="selected.id === item.id" class="pf-card-state">当前选择</div>
        <h3>{{ item.title }}</h3>
        <p>{{ item.reasons[0] }}</p>
      </article>
    </div>
    <div v-if="result.notRecommended.length" class="pf-muted-line">
      <span>本场景不优先：</span>{{ result.notRecommended.map(item => item.title).join('、') }}
    </div>
    <div class="pf-actions">
      <button type="button" class="pf-button secondary" @click="emit('back')">返回修改</button>
      <button type="button" class="pf-button primary" @click="emit('use', selected)">采用当前方案</button>
    </div>
  </section>
</template>
