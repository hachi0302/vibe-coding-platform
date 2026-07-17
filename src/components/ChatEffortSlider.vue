<script setup lang="ts">
// effort 选择器（底栏右侧）—— 对齐 Claude Code「Effort」滑杆（Image#7）：
// 标题「Effort <Level>」+「?」帮助 + Faster↔Smarter 离散滑杆（low…max / minimal…high）。
// 触发器为文字小盒；上开浮层；点档位或在轨道上拖动选择；最右档高亮 accent。
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { t } from '../i18n'
import { effortLevelsFor, effortLabel } from '../chatComposerOptions'
import type { Agent } from '../types'
import { IconHelpCircle, IconChevronDown } from './icons'

const props = defineProps<{
  agent: Agent
  model: string | undefined
  selected: string | undefined
  /** 用户未显式改档（selected=undefined）时展示的兜底档：Claude settings.json 的全局
   *  effortLevel —— CLI 不带 --effort 即用它，故它才是「真实生效」的档，而非滑杆假定的最低档。 */
  defaultLevel?: string
}>()
const emit = defineEmits<{ (e: 'pick', value: string): void }>()

const open = ref(false)
const rootEl = ref<HTMLElement>()
const trackEl = ref<HTMLElement>()
const dragging = ref(false)

// 档位随模型变（Opus 4.7/4.8 在 max 之后多一档 ultracode）。
const levels = computed(() => effortLevelsFor(props.agent, props.model))
// 实际展示的档：用户改过 → selected；否则 → 运行时默认（defaultLevel）；都没有/不在当前
// 模型档位里 → 回落最低档。滑杆位置、标题、ultracode 判定全部以它为准。
const effective = computed(() => {
  if (props.selected && levels.value.includes(props.selected)) return props.selected
  if (props.defaultLevel && levels.value.includes(props.defaultLevel)) return props.defaultLevel
  return levels.value[0]
})
const index = computed(() => Math.max(0, levels.value.indexOf(effective.value)))
const currentLabel = computed(() => effortLabel(effective.value))
// ultracode = max effort + 自动跑 workflows，标题旁加一行小字说明它实际是什么。
const isUltracode = computed(() => effective.value === 'ultracode')
function pct(i: number) {
  const n = levels.value.length
  return n <= 1 ? 0 : (i / (n - 1)) * 100
}

function toggle() {
  open.value = !open.value
}
function setIndex(i: number) {
  const lv = levels.value[i]
  if (lv && lv !== props.selected) emit('pick', lv)
}
function nearestIndex(clientX: number) {
  const el = trackEl.value
  if (!el) return index.value
  const r = el.getBoundingClientRect()
  const ratio = Math.min(1, Math.max(0, (clientX - r.left) / r.width))
  return Math.round(ratio * (levels.value.length - 1))
}
function onTrackDown(e: PointerEvent) {
  dragging.value = true
  ;(e.target as HTMLElement).setPointerCapture?.(e.pointerId)
  setIndex(nearestIndex(e.clientX))
}
function onTrackMove(e: PointerEvent) {
  if (dragging.value) setIndex(nearestIndex(e.clientX))
}
function onTrackUp() {
  dragging.value = false
}

function onDocMouseDown(e: MouseEvent) {
  if (rootEl.value && !rootEl.value.contains(e.target as Node)) open.value = false
}
watch(open, (v) => {
  if (v) document.addEventListener('mousedown', onDocMouseDown)
  else document.removeEventListener('mousedown', onDocMouseDown)
})
onBeforeUnmount(() => document.removeEventListener('mousedown', onDocMouseDown))
</script>

<template>
  <div ref="rootEl" class="es-root">
    <button class="es-trigger" :class="{ open }" v-tooltip="t('chat.composer.effort.label')" @click="toggle">
      {{ currentLabel }}
      <IconChevronDown class="es-caret" />
    </button>

    <div v-if="open" class="es-pop">
      <div class="es-head">
        <span class="es-title">{{ t('chat.composer.effort.header') }}</span>
        <span class="es-level">
          {{ currentLabel }}
          <span v-if="isUltracode" class="es-level-note">{{ t('chat.composer.effort.ultracodeNote') }}</span>
        </span>
        <span class="es-help" v-tooltip="t('chat.composer.effort.hint')"><IconHelpCircle /></span>
      </div>
      <div class="es-ends">
        <span>{{ t('chat.composer.effort.faster') }}</span>
        <span>{{ t('chat.composer.effort.smarter') }}</span>
      </div>
      <div
        ref="trackEl"
        class="es-track"
        @pointerdown="onTrackDown"
        @pointermove="onTrackMove"
        @pointerup="onTrackUp"
      >
        <div class="es-rail" />
        <div
          class="es-fill"
          :class="{ 'es-fill-ultra': isUltracode }"
          :style="{ width: pct(index) + '%' }"
        />
        <!-- 起点(0%)那颗点不画：它会和填充段的圆角起点 / 最左把手重叠，看起来像"重复的圆点"。 -->
        <span
          v-for="(lv, i) in levels"
          v-show="i > 0"
          :key="lv"
          class="es-dot"
          :class="{ last: i === levels.length - 1 }"
          :style="{ left: pct(i) + '%' }"
        />
        <span class="es-handle" :style="{ left: pct(index) + '%' }" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.es-root {
  position: relative;
  display: inline-flex;
}
/* 底栏最右触发器：无边框、无底色；hover 浅灰、展开（open）灰底。文字 + 向下小箭头。 */
.es-trigger {
  display: inline-flex;
  align-items: center;
  gap: 2px;
  border: none;
  background: transparent;
  cursor: pointer;
  font: inherit;
  font-size: 12px;
  color: var(--text);
  padding: 2px 6px 2px 8px;
  border-radius: 7px;
}
/* 下拉指示小箭头：弱化灰、展开时轻微上翻。 */
.es-caret {
  width: 13px;
  height: 13px;
  color: var(--text-mute);
  transition: transform 0.15s ease;
}
.es-trigger.open .es-caret {
  transform: rotate(180deg);
}
.es-trigger:hover {
  background: var(--surface-hover);
}
/* 展开时保持灰底（排在 :hover 后 → 同特指度下展开态压住 hover）。 */
.es-trigger.open {
  background: var(--surface-active);
}
.es-pop {
  position: absolute;
  bottom: calc(100% + 8px);
  right: 0;
  width: 320px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: var(--shadow-md);
  padding: 12px 16px 18px;
  z-index: 30;
}
.es-head {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-bottom: 14px;
}
.es-title {
  color: var(--text-mute);
  font-size: 13px;
}
.es-level {
  color: var(--text);
  font-size: 14px;
  font-weight: 600;
  flex: 1;
  /* 标题 + (xhigh + workflows) 注解保持单行，不换行 */
  white-space: nowrap;
}
/* ultracode 标题后的小字注解：(xhigh + workflows)，弱化灰、常规字重 */
.es-level-note {
  margin-left: 6px;
  font-size: 12px;
  font-weight: 400;
  color: var(--text-mute);
}
.es-help {
  display: inline-flex;
  color: var(--text-mute);
  cursor: help;
}
.es-help :deep(svg) {
  width: 15px;
  height: 15px;
}
.es-ends {
  display: flex;
  justify-content: space-between;
  color: var(--text-mute);
  font-size: 12.5px;
  margin-bottom: 8px;
}
.es-track {
  position: relative;
  height: 22px;
  cursor: pointer;
  touch-action: none;
}
.es-rail {
  position: absolute;
  top: 50%;
  left: 0;
  right: 0;
  height: 6px;
  transform: translateY(-50%);
  border-radius: 999px;
  background: var(--surface-hover);
}
.es-fill {
  position: absolute;
  top: 50%;
  left: 0;
  height: 6px;
  transform: translateY(-50%);
  border-radius: 999px;
  /* 已选段比轨道明显更深一档灰，对齐 Claude 客户端可见的填充段（--border 与轨道几乎同色，读不出来）。 */
  background: var(--text-mute);
  opacity: 0.32;
}
/* ultracode：对齐 Claude 客户端 —— 填充段换成流动的紫色渐变 + 闪烁星点动画。 */
.es-fill-ultra {
  opacity: 1;
  overflow: hidden;
  background: linear-gradient(90deg, #6d5ef0 0%, #9a6cf2 45%, #c08bf6 100%);
  background-size: 220% 100%;
  animation: es-ultra-flow 3s linear infinite;
}
.es-fill-ultra::after {
  content: '';
  position: absolute;
  inset: 0;
  border-radius: inherit;
  /* 细密白色星点：径向小点平铺，再用 opacity 呼吸做"闪烁"。 */
  background-image: radial-gradient(circle, rgba(255, 255, 255, 0.95) 0.5px, transparent 1.2px);
  background-size: 7px 7px;
  animation: es-ultra-twinkle 1.6s ease-in-out infinite;
}
@keyframes es-ultra-flow {
  0% {
    background-position: 0% 0;
  }
  100% {
    background-position: 220% 0;
  }
}
@keyframes es-ultra-twinkle {
  0%,
  100% {
    opacity: 0.25;
  }
  50% {
    opacity: 0.6;
  }
}
@media (prefers-reduced-motion: reduce) {
  .es-fill-ultra,
  .es-fill-ultra::after {
    animation: none;
  }
}
.es-dot {
  position: absolute;
  top: 50%;
  width: 5px;
  height: 5px;
  border-radius: 50%;
  background: var(--text-mute);
  opacity: 0.5;
  transform: translate(-50%, -50%);
  pointer-events: none;
}
.es-dot.last {
  background: var(--accent, #7c6cf2);
  opacity: 1;
}
.es-handle {
  position: absolute;
  top: 50%;
  width: 18px;
  height: 18px;
  border-radius: 50%;
  background: var(--surface);
  border: 1px solid var(--border);
  box-shadow: 0 1px 3px rgba(0, 0, 0, 0.25);
  transform: translate(-50%, -50%);
  pointer-events: none;
}
</style>
