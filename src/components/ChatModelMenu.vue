<script setup lang="ts">
// 模型选择器（底栏右侧文字触发）—— 对齐 Claude Code「Models」菜单（Image#4/#6）：
// 标题「Models」+ 不可用项置灰（Fable 5）+ 主列表（1/2/3 快捷键 + 勾选）+
// 「More models ›」右侧弹出子菜单（Opus 4.7 / 4.6）+「Fast mode」区（headless 无 flag，禁用）。
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { t } from '../i18n'
import { modelLabel, modelMenuFor, type ModelMenuOptions } from '../chatComposerOptions'
import type { Agent } from '../types'
import { IconCheck, IconChevronRight } from './icons'

const props = defineProps<{
  agent: Agent
  selected: string | undefined
  displayValue?: string
  menuOptions?: ModelMenuOptions
}>()
const emit = defineEmits<{ (e: 'pick', value: string): void }>()

const open = ref(false)
const moreOpen = ref(false)
const moreFlipRight = ref(false)
const moreFlipUp = ref(false)
const rootEl = ref<HTMLElement>()
const moreWrapEl = ref<HTMLElement>()
let moreTimer: ReturnType<typeof setTimeout> | undefined

// More models 子菜单：hover 触发；打开时按可用空间自动决定往右还是往左弹（避开窗口边缘裁切）。
function openMore() {
  if (moreTimer) {
    clearTimeout(moreTimer)
    moreTimer = undefined
  }
  // 默认往「左」弹：主菜单在底栏右侧，左侧空间充足；与主菜单留 8px 间隙、顶部对齐（用户要求，
  // 别和主菜单连在一起）。仅当左侧也塞不下（主菜单贴到窗口左缘）时才回退往右。
  const r = moreWrapEl.value?.getBoundingClientRect()
  moreFlipRight.value = !!r && r.left - 170 < 8
  const itemH = 36
  const subH = (cfg.value.more.length * itemH) + 12
  moreFlipUp.value = !!r && r.top + subH > window.innerHeight - 8
  moreOpen.value = true
}
function scheduleCloseMore() {
  if (moreTimer) clearTimeout(moreTimer)
  moreTimer = setTimeout(() => (moreOpen.value = false), 150)
}
function cancelCloseMore() {
  if (moreTimer) {
    clearTimeout(moreTimer)
    moreTimer = undefined
  }
}

const cfg = computed(() => modelMenuFor(props.agent, props.menuOptions))
// 触发器标签 + 菜单勾选都认这个「实际生效」值：用户改过 → selected；否则续聊回填的
// displayValue（lastModel）。否则勾选只看 selected，会出现「触发器显示 Sonnet 4.6 但
// 菜单里一项都没打勾」的割裂（session.model 续聊时仍为 undefined）。
const effectiveValue = computed(() => props.displayValue ?? props.selected)
const currentLabel = computed(
  () => modelLabel(props.agent, effectiveValue.value, props.menuOptions) || t('chat.composer.model.label'),
)

function toggle() {
  open.value = !open.value
  if (!open.value) moreOpen.value = false
}
// 供 composer 的 `/model` 指令程序化展开（底部模型面板）。
function openMenu() {
  open.value = true
}
defineExpose({ openMenu })
function pick(v: string) {
  open.value = false
  moreOpen.value = false
  emit('pick', v)
}
function onDocMouseDown(e: MouseEvent) {
  if (rootEl.value && !rootEl.value.contains(e.target as Node)) {
    open.value = false
    moreOpen.value = false
  }
}
function onKeydown(e: KeyboardEvent) {
  if (!open.value) return
  if (e.key === 'Escape') {
    open.value = false
    moreOpen.value = false
    return
  }
  const n = Number(e.key)
  if (n >= 1 && n <= cfg.value.primary.length) {
    e.preventDefault()
    pick(cfg.value.primary[n - 1].value)
  }
}
watch(open, (v) => {
  if (v) {
    document.addEventListener('mousedown', onDocMouseDown)
    document.addEventListener('keydown', onKeydown)
  } else {
    document.removeEventListener('mousedown', onDocMouseDown)
    document.removeEventListener('keydown', onKeydown)
  }
})
onBeforeUnmount(() => {
  document.removeEventListener('mousedown', onDocMouseDown)
  document.removeEventListener('keydown', onKeydown)
  if (moreTimer) clearTimeout(moreTimer)
})
</script>

<template>
  <div ref="rootEl" class="mm-root">
    <button class="mm-trigger" v-tooltip="t('chat.composer.model.label')" @click="toggle">
      <span>{{ currentLabel }}</span>
      <svg class="mm-caret" viewBox="0 0 10 6" aria-hidden="true">
        <path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.4" />
      </svg>
    </button>

    <div v-if="open" class="mm-menu" role="listbox">
      <div class="mm-section">{{ t('chat.composer.model.header') }}</div>

      <!-- 不可用（置灰，不可点）：标签 + 说明左对齐贴在一起 -->
      <div v-for="m in cfg.unavailable" :key="m.value" class="mm-item disabled mm-unavailable">
        <span class="mm-label">{{ m.label }}</span>
        <span class="mm-note">{{ t('chat.composer.model.unavailable') }}</span>
      </div>

      <!-- 主列表（勾选 + 数字快捷键，均靠右） -->
      <button
        v-for="(m, i) in cfg.primary"
        :key="m.value"
        class="mm-item"
        :class="{ active: m.value === effectiveValue }"
        role="option"
        @click="pick(m.value)"
      >
        <span class="mm-label">{{ m.label }}</span>
        <span class="mm-check"><IconCheck v-if="m.value === effectiveValue" /></span>
        <span class="mm-key">{{ i + 1 }}</span>
      </button>

      <!-- More models（右侧弹出子菜单） -->
      <template v-if="cfg.more.length">
        <div class="mm-divider" />
        <div
          ref="moreWrapEl"
          class="mm-more-wrap"
          @mouseenter="openMore"
          @mouseleave="scheduleCloseMore"
        >
          <div class="mm-item" :class="{ 'is-open': moreOpen }" @click="openMore">
            <span class="mm-label">{{ t('chat.composer.model.more') }}</span>
            <span class="mm-more-arrow"><IconChevronRight /></span>
          </div>
          <div
            v-if="moreOpen"
            class="mm-submenu"
            :class="{ right: moreFlipRight, 'flip-up': moreFlipUp }"
            role="listbox"
            @mouseenter="cancelCloseMore"
            @mouseleave="scheduleCloseMore"
          >
            <button
              v-for="m in cfg.more"
              :key="m.value"
              class="mm-item"
              :class="{ active: m.value === effectiveValue }"
              role="option"
              @click="pick(m.value)"
            >
              <span class="mm-label">{{ m.label }}</span>
              <!-- 子菜单没有数字列，无需预留勾选位：仅选中项才渲染 √，盒子贴住文字（对齐 Claude 客户端）。 -->
              <span v-if="m.value === effectiveValue" class="mm-check"><IconCheck /></span>
            </button>
          </div>
        </div>
      </template>

      <!-- Fast mode（headless 暂无对应 flag，置灰禁用，仅作视觉对齐） -->
      <template v-if="cfg.showFastMode">
        <div class="mm-divider" />
        <div class="mm-section">{{ t('chat.composer.model.fastMode') }}</div>
        <div class="mm-item disabled" v-tooltip="t('chat.composer.model.fastModeUnavailable')">
          <span class="mm-label">{{ t('chat.composer.model.enableFastMode') }}</span>
          <span class="mm-toggle" aria-disabled="true"><span class="mm-knob" /></span>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.mm-root {
  position: relative;
  display: inline-flex;
}
.mm-trigger {
  display: inline-flex;
  align-items: center;
  gap: 3px;
  border: none;
  background: transparent;
  cursor: pointer;
  font: inherit;
  font-size: 12px;
  color: var(--text);
  padding: 2px 4px;
  border-radius: 6px;
}
.mm-trigger:hover {
  background: var(--surface-hover);
}
.mm-caret {
  width: 9px;
  height: 6px;
  opacity: 0.6;
}
.mm-menu {
  position: absolute;
  bottom: calc(100% + 8px);
  right: 0;
  min-width: 248px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: var(--shadow-md);
  padding: 6px;
  z-index: 30;
}
.mm-section {
  font-size: 11px;
  color: var(--text-mute);
  padding: 5px 8px 6px;
}
.mm-item {
  width: 100%;
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 7px 8px;
  border: none;
  background: transparent;
  border-radius: 8px;
  cursor: pointer;
  text-align: left;
  color: var(--text);
  font-size: 13.5px;
}
.mm-item:hover:not(.disabled) {
  background: var(--surface-hover);
}
.mm-item.is-open {
  background: var(--surface-hover);
}
.mm-item.disabled {
  cursor: default;
  color: var(--text-mute);
}
.mm-check {
  width: 14px;
  flex: none;
  display: inline-flex;
  color: var(--text);
}
.mm-check :deep(svg) {
  width: 14px;
  height: 14px;
}
.mm-label {
  flex: 1;
  white-space: nowrap;
}
/* 不可用行：标签不撑开，让「Currently unavailable」紧贴标签左对齐（对齐参考图）。 */
.mm-unavailable .mm-label {
  flex: 0 0 auto;
  color: var(--text);
  opacity: 0.82;
}
.mm-note {
  color: var(--text-mute);
  font-size: 12px;
}
.mm-key {
  color: var(--text-mute);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
.mm-divider {
  height: 1px;
  background: var(--border);
  margin: 5px 4px;
}
.mm-more-wrap {
  position: relative;
}
.mm-more-arrow {
  display: inline-flex;
  color: var(--text-mute);
}
.mm-more-arrow :deep(svg) {
  width: 14px;
  height: 14px;
}
.mm-submenu {
  position: absolute;
  /* 默认往「左」弹，与主菜单留明显间隙（别连在一起）；顶部对齐「More models」行往下展开。
     注意：100% 是相对 .mm-more-wrap，而 wrap 被 .mm-menu 的 6px padding 内缩了，所以偏移要
     额外 +6px 抵消 —— 16px ≈ 离主菜单边框约 10px 的可见间隙。 */
  right: calc(100% + 16px);
  left: auto;
  top: 0;
  min-width: 140px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: var(--shadow-md);
  padding: 6px;
  z-index: 31;
}
/* 左侧空间不足（主菜单贴到窗口左缘）→ 回退往右弹，同样留明显间隙（同理 +6px 抵消 padding） */
.mm-submenu.right {
  right: auto;
  left: calc(100% + 16px);
}
.mm-submenu.flip-up {
  top: auto;
  bottom: 0;
}
.mm-toggle {
  width: 30px;
  height: 18px;
  border-radius: 999px;
  background: var(--border);
  position: relative;
  flex: none;
  opacity: 0.7;
}
.mm-knob {
  position: absolute;
  top: 2px;
  left: 2px;
  width: 14px;
  height: 14px;
  border-radius: 50%;
  background: var(--surface);
  box-shadow: 0 1px 2px rgba(0, 0, 0, 0.2);
}
</style>
