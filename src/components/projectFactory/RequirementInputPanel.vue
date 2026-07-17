<script setup lang="ts">
import type {
  AgentAnalysisPayload,
  RequirementInputKind,
  RequirementMaterialBundle,
  StructurePreference,
} from '../../projectFactory/types'
import { IconChat, IconFolder } from '../icons'

const props = defineProps<{
  kind: RequirementInputKind
  text: string
  sourceValue: string
  structurePreference: StructurePreference
  followUp: string
  material?: RequirementMaterialBundle
  analysis?: AgentAnalysisPayload
  busy?: boolean
  materialLoading?: boolean
  error?: string
}>()

const emit = defineEmits<{
  (e: 'update:kind', value: RequirementInputKind): void
  (e: 'update:text', value: string): void
  (e: 'update:sourceValue', value: string): void
  (e: 'update:structurePreference', value: StructurePreference): void
  (e: 'update:followUp', value: string): void
  (e: 'choose-file'): void
  (e: 'choose-folder'): void
  (e: 'analyze'): void
  (e: 'reanalyze'): void
  (e: 'confirm-analysis'): void
}>()

const inputKinds: Array<{
  value: RequirementInputKind
  label: string
  hint: string
  icon: typeof IconChat
}> = [
  { value: 'text', label: '直接描述', hint: '自己输入需求', icon: IconChat },
  { value: 'local', label: '选择本机资料', hint: '读取文件或整个文件夹', icon: IconFolder },
]

const canAnalyze = () => props.kind === 'text' ? Boolean(props.text.trim()) : Boolean(props.material)
</script>

<template>
  <section class="pf-panel pf-input-panel">
    <div class="pf-panel-head">
      <div>
        <h2>新项目工厂</h2>
        <p>输入需求或选择本机资料，先确认分析结论，再进入技术方案。</p>
      </div>
      <span class="pf-step">01 / 需求</span>
    </div>

    <div class="pf-input-grid">
      <div class="pf-form-column">
        <div class="pf-field">
          <span>从哪里开始</span>
          <div class="pf-source-grid" role="tablist" aria-label="需求来源">
            <button
              v-for="item in inputKinds"
              :key="item.value"
              type="button"
              class="pf-source-option"
              :class="{ active: kind === item.value }"
              @click="emit('update:kind', item.value)"
            >
              <component :is="item.icon" aria-hidden="true" />
              <span>{{ item.label }}</span>
              <small>{{ item.hint }}</small>
            </button>
          </div>
        </div>

        <label v-if="kind === 'text'" class="pf-field pf-requirement-text">
          <span>项目需求</span>
          <textarea
            :value="text"
            rows="7"
            placeholder="描述要解决的问题、主要用户和核心功能。可以先写一句话，分析后再继续补充。"
            @input="emit('update:text', ($event.target as HTMLTextAreaElement).value)"
          />
        </label>

        <div v-else class="pf-field pf-local-material">
          <span>本机资料</span>
          <div class="pf-local-picker">
            <button
              data-testid="choose-local-file"
              type="button"
              class="pf-button secondary"
              :disabled="materialLoading"
              @click="emit('choose-file')"
            >选择文件</button>
            <button
              data-testid="choose-local-folder"
              type="button"
              class="pf-button secondary"
              :disabled="materialLoading"
              @click="emit('choose-folder')"
            >选择文件夹</button>
          </div>
          <div v-if="materialLoading" class="pf-material-status">正在读取资料…</div>
          <div v-else-if="material" class="pf-material-summary">
            <strong>{{ material.sourceLabel }}</strong>
            <span>已读取 {{ material.files.filter(item => item.included).length }} / {{ material.files.length }} 个文件</span>
            <small v-if="material.warnings.length">{{ material.warnings.length }} 项资料未提取正文，分析时会明确列出。</small>
          </div>
          <div v-else class="pf-material-status">可以选择一个文件，也可以选择包含多份需求资料的文件夹；文件夹会递归读取。</div>
        </div>

        <div class="pf-field pf-structure-preference">
          <span>项目结构</span>
          <div class="pf-segmented" role="group" aria-label="项目结构">
            <button type="button" :class="{ active: structurePreference === 'auto' }" @click="emit('update:structurePreference', 'auto')">自动推荐</button>
            <button type="button" :class="{ active: structurePreference === 'single-app' }" @click="emit('update:structurePreference', 'single-app')">单体项目</button>
            <button type="button" :class="{ active: structurePreference === 'frontend-backend' }" @click="emit('update:structurePreference', 'frontend-backend')">前后端分离</button>
          </div>
          <small>不确定时保持自动推荐；这是技术方案的约束，不会替代你的需求。</small>
        </div>

        <div v-if="!analysis" class="pf-actions">
          <p v-if="error" class="pf-error pf-input-error">{{ error }}</p>
          <button
            type="button"
            class="pf-button primary"
            :disabled="busy || materialLoading || !canAnalyze()"
            @click="emit('analyze')"
          >{{ busy ? '分析中…' : '分析需求' }}</button>
        </div>

        <section v-else class="pf-requirement-analysis" aria-label="需求分析结论">
          <div class="pf-review-head">
            <div>
              <span>需求分析结论</span>
              <h3>{{ analysis.recommended.title }}</h3>
            </div>
            <small>确认无误后再进入技术选型</small>
          </div>

          <p v-if="analysis.recommended.reasons.length" class="pf-review-summary">
            {{ analysis.recommended.reasons.join('；') }}
          </p>

          <div v-if="analysis.recognizedConstraints.length" class="pf-review-section">
            <strong>已识别信息</strong>
            <ul>
              <li v-for="item in analysis.recognizedConstraints" :key="item.id">
                {{ item.label }}：{{ item.value }}
              </li>
            </ul>
          </div>

          <div v-if="analysis.clarifyingQuestions.length" class="pf-review-section">
            <strong>还需要你确认</strong>
            <ul>
              <li v-for="item in analysis.clarifyingQuestions" :key="item.id">{{ item.label }}</li>
            </ul>
          </div>

          <div v-if="analysis.assumptions.length" class="pf-review-section muted">
            <strong>当前假设</strong>
            <ul><li v-for="item in analysis.assumptions" :key="item">{{ item }}</li></ul>
          </div>

          <label class="pf-field pf-follow-up">
            <span>补充、纠正或追问</span>
            <textarea
              data-testid="analysis-follow-up"
              :value="followUp"
              rows="4"
              placeholder="例如：只需要管理员和运营两个角色；或者追问为什么这样理解。"
              @input="emit('update:followUp', ($event.target as HTMLTextAreaElement).value)"
            />
          </label>

          <p v-if="error" class="pf-error">{{ error }}</p>
          <div class="pf-review-actions">
            <button
              data-testid="reanalyze-requirement"
              type="button"
              class="pf-button secondary"
              :disabled="busy || !followUp.trim()"
              @click="emit('reanalyze')"
            >{{ busy ? '继续分析中…' : '根据补充继续分析' }}</button>
            <button
              data-testid="confirm-requirement-analysis"
              type="button"
              class="pf-button primary"
              :disabled="busy"
              @click="emit('confirm-analysis')"
            >分析没问题，进入下一步</button>
          </div>
        </section>
      </div>

      <aside class="pf-flow-rail" aria-label="创建步骤">
        <div class="active"><b>01</b><span>需求</span></div>
        <div><b>02</b><span>选型</span></div>
        <div><b>03</b><span>环境</span></div>
        <div><b>04</b><span>预览</span></div>
        <div><b>05</b><span>创建</span></div>
      </aside>
    </div>
  </section>
</template>
