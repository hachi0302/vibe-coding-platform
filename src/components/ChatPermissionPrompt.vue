<script setup lang="ts">
// 交互式工具权限对话框 —— 对齐 Claude Code CLI 的「Allow Claude to run X?」气泡：
// 工具名 + 命令/路径预览 + 三个选项（始终允许 / 允许本次 / 拒绝）。纯展示组件：
// 不碰会话状态，只把用户的三选一以 `choose` 事件抛给 ChatView（再交给 respondPermission）。
import { computed } from 'vue'
import { t } from '../i18n'
import type { Agent, ChatPermissionRequest } from '../types'
import { permissionCommandPreview, permissionHasSuggestions, type PermissionChoice } from '../chatPermission'
import { IconShieldCheck, IconCheck, IconClose } from './icons'

const props = defineProps<{ request: ChatPermissionRequest; agent?: Agent }>()
const emit = defineEmits<{ (e: 'choose', choice: PermissionChoice): void }>()

const preview = computed(() => permissionCommandPreview(props.request))
const hasSuggestions = computed(() => permissionHasSuggestions(props.request))
const isCodex = computed(() => props.agent === 'codex')
const title = computed(() =>
  isCodex.value
    ? t('chat.permission.codex.title')
    : t('chat.permission.title', { tool: props.request.toolName }),
)
const alwaysAllowHint = computed(() =>
  isCodex.value
    ? t('chat.permission.codex.alwaysAllowHint')
    : t('chat.permission.alwaysAllowHint'),
)
const input = computed<Record<string, unknown>>(() =>
  props.request.input && typeof props.request.input === 'object' && !Array.isArray(props.request.input)
    ? (props.request.input as Record<string, unknown>)
    : {},
)
const environment = computed(() =>
  typeof input.value.environment === 'string' ? input.value.environment : '',
)
const reason = computed(() =>
  typeof input.value.reason === 'string' ? input.value.reason : '',
)
</script>

<template>
  <div class="perm-prompt" role="alertdialog" aria-modal="false">
    <div class="perm-head">
      <IconShieldCheck class="perm-shield" />
      <span class="perm-title">{{ title }}</span>
    </div>
    <div v-if="isCodex && (environment || reason)" class="perm-meta">
      <div v-if="environment" class="perm-meta-row">
        <span>{{ t('chat.permission.environment') }}</span>
        <strong>{{ environment }}</strong>
      </div>
      <div v-if="reason" class="perm-meta-row">
        <span>{{ t('chat.permission.reason') }}</span>
        <em>{{ reason }}</em>
      </div>
    </div>
    <pre v-if="preview" class="perm-cmd">{{ preview }}</pre>
    <div v-if="request.description" class="perm-desc">{{ request.description }}</div>
    <div class="perm-actions">
      <button class="perm-btn perm-allow" type="button" @click="emit('choose', 'allow-once')">
        <IconCheck />
        <span>{{ t('chat.permission.allowOnce') }}</span>
      </button>
      <button
        v-if="hasSuggestions"
        class="perm-btn perm-always"
        type="button"
        v-tooltip="alwaysAllowHint"
        @click="emit('choose', 'always-allow')"
      >
        <span>{{ t('chat.permission.alwaysAllow') }}</span>
      </button>
      <button class="perm-btn perm-deny" type="button" @click="emit('choose', 'deny')">
        <IconClose />
        <span>{{ t('chat.permission.deny') }}</span>
      </button>
    </div>
  </div>
</template>

<style scoped>
.perm-prompt {
  margin: 8px 0 4px;
  padding: 12px 14px;
  border: 1px solid var(--border);
  border-left: 3px solid var(--brand);
  border-radius: 10px;
  background: var(--surface-hover);
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.perm-head {
  display: flex;
  align-items: center;
  gap: 8px;
}
.perm-shield {
  width: 16px;
  height: 16px;
  color: var(--brand);
  flex: none;
}
.perm-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text);
}
.perm-cmd {
  margin: 0;
  padding: 8px 10px;
  border-radius: 6px;
  background: var(--surface);
  border: 1px solid var(--border);
  font-family: var(--font-mono, ui-monospace, monospace);
  font-size: 12px;
  line-height: 1.5;
  color: var(--text);
  white-space: pre-wrap;
  word-break: break-all;
  max-height: 9.5em;
  overflow: auto;
}
.perm-meta {
  display: grid;
  gap: 6px;
  font-size: 12px;
  color: var(--text-dim);
}
.perm-meta-row {
  display: grid;
  grid-template-columns: 92px minmax(0, 1fr);
  gap: 8px;
  align-items: baseline;
}
.perm-meta-row strong {
  color: var(--text);
  font-weight: 600;
}
.perm-meta-row em {
  color: var(--text);
  font-style: italic;
  min-width: 0;
}
.perm-desc {
  font-size: 12px;
  color: var(--text-dim);
  line-height: 1.45;
}
.perm-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
  margin-top: 2px;
}
.perm-btn {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border-radius: 7px;
  border: 1px solid var(--border);
  background: var(--surface);
  color: var(--text);
  font-size: 12px;
  font-weight: 500;
  cursor: pointer;
  transition: background 0.12s, border-color 0.12s, color 0.12s;
}
.perm-btn svg {
  width: 14px;
  height: 14px;
}
.perm-btn:hover {
  background: var(--surface-hover);
}
/* 主操作（允许本次）：中性反色，与设计系统的主按钮一致（Codex 风，不用品牌色填充）。 */
.perm-allow {
  background: var(--text);
  color: var(--surface);
  border-color: var(--text);
}
.perm-allow:hover {
  opacity: 0.9;
  background: var(--text);
}
.perm-deny:hover {
  border-color: var(--danger, #d9534f);
  color: var(--danger, #d9534f);
}
</style>
