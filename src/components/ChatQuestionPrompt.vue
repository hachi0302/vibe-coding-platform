<script setup lang="ts">
// 结构化提问卡片 —— 对齐 Claude Code 的 AskUserQuestion 工具：模型抛出一组选择题，用户在此
// 单选 / 多选 / 选 Other 自填 / 看并排预览，提交后把答案回写控制协议。纯展示组件：不碰会话
// 状态，只把用户的选择（或取消）以 submit / cancel 事件抛给 ChatView（再交给 respondQuestion）。
// 多问题时一次只展示一题，用 上一题 / 下一题 翻页（对齐原生体验），最后一题才出现提交。
import { computed, reactive, ref } from 'vue'
import { t } from '../i18n'
import type { ChatQuestionItem, ChatQuestionRequest } from '../types'
import {
  allQuestionsAnswered,
  questionAnswered,
  questionHasPreview,
  type QuestionSelection,
} from '../chatQuestion'
import { IconHelpCircle, IconCheck, IconClose, IconChevronRight, IconArrowLeft } from './icons'

const props = defineProps<{ request: ChatQuestionRequest }>()
const emit = defineEmits<{
  (e: 'submit', selections: QuestionSelection[]): void
  (e: 'cancel'): void
}>()

interface QState {
  labels: string[] // 选中的结构化选项 label
  otherOn: boolean // 是否选了 Other
  otherText: string // Other 自填文本
  previewIdx: number // 并排预览当前展示哪个选项（仅单选 + 有 preview 时用）
}

// 每条提问一份本地选择态。预览初始指向第一个带 preview 的选项（没有则 0）。
const state = reactive<QState[]>(
  props.request.questions.map((q) => ({
    labels: [],
    otherOn: false,
    otherText: '',
    previewIdx: Math.max(
      0,
      q.options.findIndex((o) => o.preview && o.preview.trim().length > 0),
    ),
  })),
)

const cur = ref(0) // 当前展示第几题（翻页）
const total = computed(() => props.request.questions.length)
const current = computed(() => props.request.questions[cur.value])
const isLast = computed(() => cur.value >= total.value - 1)

const selections = computed<QuestionSelection[]>(() =>
  state.map((s) => ({
    labels: [...s.labels],
    otherText: s.otherOn ? s.otherText : undefined,
  })),
)

const currentAnswered = computed(() => questionAnswered(selections.value[cur.value]))
const canSubmit = computed(() => allQuestionsAnswered(props.request, selections.value))

const answeredAt = (i: number) => questionAnswered(selections.value[i])
const hasPreview = (q: ChatQuestionItem) => questionHasPreview(q)

function isChecked(qi: number, label: string): boolean {
  return state[qi].labels.includes(label)
}

/** 选一个结构化选项：多选切换，单选独占（并清掉 Other）。带 preview 时联动预览。 */
function toggleOption(qi: number, q: ChatQuestionItem, label: string, oi: number) {
  const s = state[qi]
  if (q.multiSelect) {
    s.labels = s.labels.includes(label) ? s.labels.filter((l) => l !== label) : [...s.labels, label]
  } else {
    s.labels = [label]
    s.otherOn = false
  }
  if (q.options[oi]?.preview?.trim()) s.previewIdx = oi
}

/** 选 Other：多选独立切换，单选独占（并清掉结构化选项）。 */
function toggleOther(qi: number, q: ChatQuestionItem) {
  const s = state[qi]
  if (q.multiSelect) {
    s.otherOn = !s.otherOn
  } else {
    s.otherOn = true
    s.labels = []
  }
}

/** 在 Other 框里打字即自动选中 Other（单选则独占）。 */
function onOtherInput(qi: number, q: ChatQuestionItem) {
  const s = state[qi]
  if (!s.otherText.trim()) return
  if (q.multiSelect) {
    s.otherOn = true
  } else {
    s.otherOn = true
    s.labels = []
  }
}

/** 鼠标移到带 preview 的选项 → 切换预览面板；移到无 preview 的选项 / Other 则不动（保持上次）。 */
function hoverPreview(qi: number, oi: number) {
  if (props.request.questions[qi].options[oi]?.preview?.trim()) state[qi].previewIdx = oi
}

function previewContent(qi: number, q: ChatQuestionItem): string {
  const opt = q.options[state[qi].previewIdx]
  return opt?.preview?.trim() ? opt.preview : ''
}

function next() {
  if (currentAnswered.value && !isLast.value) cur.value += 1
}
function back() {
  if (cur.value > 0) cur.value -= 1
}
function submit() {
  if (canSubmit.value) emit('submit', selections.value)
}
/** 回车：当前题已答 → 最后一题提交，否则翻到下一题。 */
function proceed() {
  if (!currentAnswered.value) return
  if (isLast.value) submit()
  else next()
}
</script>

<template>
  <div class="q-prompt" role="alertdialog" aria-modal="false">
    <div class="q-head">
      <IconHelpCircle class="q-icon" />
      <span class="q-title">{{ t('chat.question.title') }}</span>
      <span v-if="total > 1" class="q-progress">{{ cur + 1 }} / {{ total }}</span>
    </div>

    <!-- 多问题时的进度点：已答实心、当前高亮、未到的留空。 -->
    <div v-if="total > 1" class="q-steps">
      <span
        v-for="i in total"
        :key="i"
        class="q-step"
        :class="{ done: answeredAt(i - 1), active: i - 1 === cur }"
      ></span>
    </div>

    <div class="q-item">
      <div class="q-text">{{ current.question }}</div>
      <div v-if="current.multiSelect" class="q-multi">{{ t('chat.question.multiHint') }}</div>

      <div class="q-body" :class="{ split: hasPreview(current) }">
        <div class="q-options">
          <button
            v-for="(opt, oi) in current.options"
            :key="oi"
            type="button"
            class="q-opt"
            :class="{ on: isChecked(cur, opt.label) }"
            @click="toggleOption(cur, current, opt.label, oi)"
            @mouseenter="hoverPreview(cur, oi)"
          >
            <span class="q-opt-text">
              <span class="q-opt-label">{{ opt.label }}</span>
              <span v-if="opt.description" class="q-opt-desc">{{ opt.description }}</span>
            </span>
            <span class="q-mark" :class="current.multiSelect ? 'box' : 'dot'"></span>
          </button>

          <div class="q-opt q-other" :class="{ on: state[cur].otherOn }">
            <button type="button" class="q-other-toggle" @click="toggleOther(cur, current)">
              <span class="q-opt-label">{{ t('chat.question.other') }}</span>
              <span class="q-mark" :class="current.multiSelect ? 'box' : 'dot'"></span>
            </button>
            <input
              v-model="state[cur].otherText"
              class="q-other-input"
              type="text"
              :placeholder="t('chat.question.otherPlaceholder')"
              @input="onOtherInput(cur, current)"
              @keydown.enter.prevent="proceed"
            />
          </div>
        </div>

        <div v-if="hasPreview(current)" class="q-preview">
          <pre v-if="previewContent(cur, current)">{{ previewContent(cur, current) }}</pre>
          <div v-else class="q-preview-hint">{{ t('chat.question.previewHint') }}</div>
        </div>
      </div>
    </div>

    <div class="q-actions">
      <button v-if="cur > 0" class="q-btn q-back" type="button" @click="back">
        <IconArrowLeft />
        <span>{{ t('chat.question.back') }}</span>
      </button>
      <div class="q-actions-right">
        <button class="q-btn q-cancel" type="button" @click="emit('cancel')">
          <IconClose />
          <span>{{ t('chat.question.cancel') }}</span>
        </button>
        <button
          v-if="!isLast"
          class="q-btn q-primary q-next"
          type="button"
          :disabled="!currentAnswered"
          @click="next"
        >
          <span>{{ t('chat.question.next') }}</span>
          <IconChevronRight />
        </button>
        <button
          v-else
          class="q-btn q-primary q-submit"
          type="button"
          :disabled="!canSubmit"
          @click="submit"
        >
          <IconCheck />
          <span>{{ t('chat.question.submit') }}</span>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.q-prompt {
  margin: 8px 0 4px;
  padding: 14px 16px;
  border: 1px solid var(--border);
  border-radius: 14px;
  background: var(--surface);
  display: flex;
  flex-direction: column;
  gap: 12px;
}
.q-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.q-icon {
  width: 16px;
  height: 16px;
  color: var(--brand);
  flex: none;
}
.q-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}
/* 进度计数（N / M），靠右、品牌淡色 —— 小点缀。 */
.q-progress {
  margin-left: auto;
  font-size: 11px;
  font-weight: 600;
  color: var(--brand);
  background: color-mix(in srgb, var(--brand) 14%, transparent);
  border-radius: 9px;
  padding: 1px 8px;
}
/* 进度点 */
.q-steps {
  display: flex;
  gap: 6px;
}
.q-step {
  width: 18px;
  height: 4px;
  border-radius: 2px;
  background: var(--border);
  transition: background 0.12s;
}
.q-step.done {
  background: color-mix(in srgb, var(--brand) 55%, var(--border));
}
.q-step.active {
  background: var(--brand);
}
.q-item {
  display: flex;
  flex-direction: column;
  gap: 10px;
}
.q-text {
  font-size: 14.5px;
  font-weight: 600;
  color: var(--text);
  line-height: 1.4;
}
.q-multi {
  font-size: 11.5px;
  color: var(--text-dim);
  margin-top: -4px;
}
.q-body.split {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1.1fr);
  gap: 12px;
  align-items: start;
}
.q-options {
  display: flex;
  flex-direction: column;
  gap: 8px;
}
/* 整行卡片式选项：文字靠左、标记靠右。 */
.q-opt {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
  text-align: left;
  padding: 11px 14px;
  border-radius: 10px;
  border: 1px solid transparent;
  background: var(--surface-hover);
  color: var(--text);
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s;
}
.q-opt:hover {
  background: color-mix(in srgb, var(--surface-hover), var(--text) 6%);
}
.q-opt.on {
  border-color: var(--text);
}
.q-opt-text {
  display: flex;
  flex-direction: column;
  gap: 3px;
  min-width: 0;
}
.q-opt-label {
  font-size: 13.5px;
  font-weight: 600;
  line-height: 1.3;
}
.q-opt-desc {
  font-size: 12.5px;
  color: var(--text-dim);
  line-height: 1.4;
}
/* 标记（靠右）：单选圆点 / 多选方块，选中后实心回填。 */
.q-mark {
  flex: none;
  width: 18px;
  height: 18px;
  border: 1.5px solid var(--border-strong, var(--border));
  background: var(--surface);
  position: relative;
}
.q-mark.dot {
  border-radius: 50%;
}
.q-mark.box {
  border-radius: 5px;
}
.q-opt.on .q-mark,
.q-other.on .q-mark {
  border-color: var(--text);
  background: var(--text);
}
.q-opt.on .q-mark::after,
.q-other.on .q-mark::after {
  content: '';
  position: absolute;
  inset: 4px;
  border-radius: inherit;
  background: var(--surface);
}
/* Other 行：开关按钮（标签 + 靠右标记）+ 常显自填输入框。 */
.q-other {
  flex-direction: column;
  align-items: stretch;
  gap: 10px;
  cursor: default;
}
.q-other:hover {
  background: var(--surface-hover);
}
.q-other-toggle {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
  background: none;
  border: none;
  padding: 0;
  color: inherit;
  cursor: pointer;
}
.q-other-input {
  width: 100%;
  padding: 9px 11px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  font-size: 13px;
  outline: none;
}
.q-other-input:focus {
  border-color: var(--text);
}
.q-preview {
  border: 1px solid var(--border);
  border-radius: 10px;
  background: var(--surface);
  overflow: hidden;
  min-height: 100%;
}
.q-preview pre {
  margin: 0;
  padding: 10px 12px;
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 11.5px;
  line-height: 1.5;
  color: var(--text);
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 16em;
  overflow: auto;
}
.q-preview-hint {
  padding: 12px;
  font-size: 11.5px;
  color: var(--text-dim);
  line-height: 1.5;
}
.q-actions {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 2px;
}
/* 右侧操作组（取消 + 下一题/提交）始终靠右，左侧只放“上一题”。 */
.q-actions-right {
  display: flex;
  gap: 8px;
  margin-left: auto;
}
.q-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 7px 16px;
  border-radius: 8px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s, color 0.12s, opacity 0.12s;
}
.q-btn svg {
  width: 14px;
  height: 14px;
}
.q-btn:hover:not(:disabled) {
  background: var(--surface-hover);
}
/* 主操作（下一题 / 提交）：中性反色，与设计系统主按钮一致（Codex 风，不用品牌色填充）。 */
.q-primary {
  background: var(--text);
  color: var(--surface);
  border-color: var(--text);
}
.q-primary:hover:not(:disabled) {
  opacity: 0.9;
  background: var(--text);
}
.q-primary:disabled {
  opacity: 0.45;
  cursor: not-allowed;
}
.q-cancel:hover {
  border-color: var(--danger, #d9534f);
  color: var(--danger, #d9534f);
}
</style>
