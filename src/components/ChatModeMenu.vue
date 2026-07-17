<script setup lang="ts">
// 权限模式选择器（底栏左侧金色 chip）—— 对齐 Claude Code「Mode」菜单（Image#8）：
// 标题「Mode」+ 五档（Ask permissions / Accept edits / Plan mode / Auto mode / Bypass），
// 带 1–5 数字快捷键 + 勾选；上开菜单、点外面关、数字键直选。
import { computed, onBeforeUnmount, ref, watch } from 'vue'
import { t } from '../i18n'
import { permissionModesFor, permissionLabelKey, permissionModeDisabled } from '../chatComposerOptions'
import type { Agent } from '../types'
import { IconCheck } from './icons'

const props = defineProps<{ agent: Agent; selected: string; model?: string; disabled?: boolean }>()
const emit = defineEmits<{ (e: 'pick', value: string): void }>()

const open = ref(false)
const rootEl = ref<HTMLElement>()

const items = computed(() =>
  permissionModesFor(props.agent).map((m, i) => ({
    value: m.value,
    label: t(m.labelKey),
    key: i + 1,
    off: permissionModeDisabled(props.agent, m.value, props.model),
  })),
)
const currentLabel = computed(() => t(permissionLabelKey(props.agent, props.selected)))
// Claude: bypassPermissions 金色危险提示；Codex: fullAccess 同理。
const danger = computed(() => props.selected === 'bypassPermissions' || props.selected === 'fullAccess')

function toggle() {
  if (props.disabled) return
  open.value = !open.value
}
function pick(v: string) {
  open.value = false
  emit('pick', v)
}
function onDocMouseDown(e: MouseEvent) {
  if (rootEl.value && !rootEl.value.contains(e.target as Node)) open.value = false
}
function onKeydown(e: KeyboardEvent) {
  if (!open.value) return
  if (e.key === 'Escape') {
    open.value = false
    return
  }
  const n = Number(e.key)
  if (n >= 1 && n <= items.value.length) {
    e.preventDefault()
    const it = items.value[n - 1]
    if (!it.off) pick(it.value)
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
})
</script>

<template>
  <div ref="rootEl" class="cm-root">
    <button
      class="cm-chip"
      :class="{ disabled, danger }"
      :disabled="disabled"
      v-tooltip="t('chat.composer.mode.header')"
      @click="toggle"
    >
      <span>{{ currentLabel }}</span>
      <svg class="cm-caret" viewBox="0 0 10 6" aria-hidden="true">
        <path d="M1 1l4 4 4-4" fill="none" stroke="currentColor" stroke-width="1.4" />
      </svg>
    </button>

    <div v-if="open" class="cm-menu" role="listbox">
      <div class="cm-section">{{ t('chat.composer.mode.header') }}</div>
      <button
        v-for="it in items"
        :key="it.value"
        class="cm-item"
        :class="{ active: it.value === selected, off: it.off }"
        :disabled="it.off"
        role="option"
        v-tooltip="it.off ? t('chat.composer.permission.autoUnsupported') : ''"
        @click="!it.off && pick(it.value)"
      >
        <span class="cm-label">{{ it.label }}</span>
        <span class="cm-check"><IconCheck v-if="it.value === selected" /></span>
        <span class="cm-key">{{ it.key }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.cm-root {
  position: relative;
  display: inline-flex;
}
.cm-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  border: none;
  cursor: pointer;
  font: inherit;
  font-size: 11.5px;
  border-radius: 6px;
  /* 普通模式：中性无底，hover 才加浅灰底（对齐 Accept edits 等常规档）。 */
  color: var(--text-mute);
  background: transparent;
  padding: 2px 6px 2px 8px;
}
.cm-chip:hover:not(.disabled) {
  background: var(--surface-hover);
}
/* 最高危的 Bypass permissions：保留金色字 + 金色底作为危险提示。 */
.cm-chip.danger {
  color: #9a7b1a;
  background: rgba(212, 167, 44, 0.14);
}
.cm-chip.danger:hover:not(.disabled) {
  background: rgba(212, 167, 44, 0.22);
}
.cm-chip.disabled {
  cursor: default;
  opacity: 0.5;
}
.cm-caret {
  width: 9px;
  height: 6px;
  opacity: 0.7;
}
.cm-menu {
  position: absolute;
  bottom: calc(100% + 6px);
  left: 0;
  min-width: 220px;
  background: var(--surface);
  border: 1px solid var(--border);
  border-radius: 12px;
  box-shadow: var(--shadow-md);
  padding: 6px;
  z-index: 30;
}
.cm-section {
  font-size: 11px;
  color: var(--text-mute);
  padding: 4px 8px 6px;
}
.cm-item {
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
.cm-item:hover:not(.off) {
  background: var(--surface-hover);
}
.cm-item.off {
  cursor: default;
  color: var(--text-mute);
}
.cm-check {
  width: 14px;
  flex: none;
  display: inline-flex;
  color: var(--text);
}
.cm-check :deep(svg) {
  width: 14px;
  height: 14px;
}
.cm-label {
  flex: 1;
}
.cm-key {
  color: var(--text-mute);
  font-size: 12px;
  font-variant-numeric: tabular-nums;
}
</style>
