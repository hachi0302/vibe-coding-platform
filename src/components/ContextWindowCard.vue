<script setup lang="ts">
// `/context` 的可视化面板（参考 Claude 客户端的 Context window 卡片）：
//   折叠态 = 标题 + 「已用/总量 (%)」+ 分段进度条；
//   展开态 = 额外列出各类目（彩色圆点 + token + 占比）与可再展开的明细分区（Memory Files / Skills / …）。
// 数据来自 contextUsage.ts 的 parseContextUsage —— 本组件只负责呈现，不做解析。
import { computed, ref } from 'vue'
import { t } from '../i18n'
import type { ContextUsage, ContextCategory, ContextCategoryKind } from '../contextUsage'
import { IconContextWindow, IconChevronRight } from './icons'

const props = defineProps<{ usage: ContextUsage }>()

const expanded = ref(false)

// 蓝色族（深→浅）按 token 占比由大到小分配给「真实占用」类目；缓冲/延迟/空闲走中性灰。
const BLUES = [
  'var(--ctx-blue-1)',
  'var(--ctx-blue-2)',
  'var(--ctx-blue-3)',
  'var(--ctx-blue-4)',
  'var(--ctx-blue-5)',
  'var(--ctx-blue-6)',
  'var(--ctx-blue-7)',
]
function grayFor(kind: ContextCategoryKind): string {
  if (kind === 'buffer') return 'var(--ctx-gray-buffer)'
  if (kind === 'deferred') return 'var(--ctx-gray-deferred)'
  return 'var(--ctx-track)' // free
}

// 展示顺序：真实占用（按占比降序）→ 缓冲 → 空闲 → 延迟，贴近 Claude 客户端的分组观感。
const KIND_RANK: Record<ContextCategoryKind, number> = { used: 0, buffer: 1, free: 2, deferred: 3 }

interface Row extends ContextCategory {
  color: string
}
const rows = computed<Row[]>(() => {
  const sorted = [...props.usage.categories].sort((a, b) => {
    const r = KIND_RANK[a.kind] - KIND_RANK[b.kind]
    return r !== 0 ? r : b.percent - a.percent
  })
  let usedIdx = 0
  return sorted.map((c) => ({
    ...c,
    color: c.kind === 'used' ? BLUES[Math.min(usedIdx++, BLUES.length - 1)] : grayFor(c.kind),
  }))
})

// 进度条的彩色段 = 除「空闲」外的所有类目；空闲即条的浅色底（未填充部分）。
const segments = computed(() => rows.value.filter((r) => r.kind !== 'free'))

function tokenColIdx(cols: string[]): number {
  const i = cols.findIndex((c) => /token/i.test(c))
  return i >= 0 ? i : cols.length - 1
}
function primaryColIdx(cols: string[]): number {
  const i = cols.findIndex((c) => /path/i.test(c))
  return i >= 0 ? i : 0
}
</script>

<template>
  <div class="ctx-card" :class="{ open: expanded }">
    <button
      type="button"
      class="ctx-head"
      :aria-expanded="expanded"
      @click="expanded = !expanded"
    >
      <span class="ctx-title">
        <IconContextWindow class="ctx-title-icon" />
        {{ t('chat.context.title') }}
      </span>
      <span class="ctx-summary">
        {{ usage.usedLabel }} / {{ usage.totalLabel }} ({{ usage.percent }}%)
      </span>
      <IconChevronRight class="ctx-chev" :class="{ open: expanded }" />
    </button>

    <div class="ctx-bar">
      <span
        v-for="(s, i) in segments"
        :key="i"
        class="ctx-seg"
        :style="{ width: s.percent + '%', background: s.color }"
        v-tooltip="`${s.name} · ${s.percentLabel}`"
      />
    </div>

    <div v-if="expanded" class="ctx-body">
      <div v-for="(c, i) in rows" :key="'c' + i" class="ctx-row">
        <span class="ctx-dot" :style="{ background: c.color }" />
        <span class="ctx-name">{{ c.name }}</span>
        <span class="ctx-tokens">{{ c.tokensLabel }}</span>
        <span class="ctx-pct">{{ c.percentLabel }}</span>
      </div>

      <details v-for="(d, di) in usage.details" :key="'d' + di" class="ctx-detail">
        <summary class="ctx-detail-head">
          <IconChevronRight class="ctx-detail-chev" />
          <span class="ctx-name">{{ d.name }}</span>
          <span class="ctx-tokens">{{ d.tokensLabel ?? '' }}</span>
          <span class="ctx-pct">{{ d.count }}</span>
        </summary>
        <div class="ctx-detail-body">
          <div v-for="(r, ri) in d.rows" :key="ri" class="ctx-item">
            <span class="ctx-item-main" v-tooltip="r[primaryColIdx(d.columns)]">{{
              r[primaryColIdx(d.columns)]
            }}</span>
            <span class="ctx-item-sub">{{ r[tokenColIdx(d.columns)] }}</span>
          </div>
        </div>
      </details>
    </div>
  </div>
</template>

<style scoped>
.ctx-card {
  width: 100%;
  max-width: 560px;
  border: 1px solid var(--border);
  border-radius: 12px;
  background: var(--surface-hover);
  padding: 12px 14px;
  font-size: 13px;
}

.ctx-head {
  display: flex;
  align-items: center;
  gap: 10px;
  width: 100%;
  background: none;
  border: none;
  padding: 0;
  cursor: pointer;
  color: var(--text);
  text-align: left;
}
.ctx-title {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-weight: 600;
  font-size: 14px;
}
.ctx-title-icon {
  width: 16px;
  height: 16px;
  color: var(--text-mute);
}
.ctx-summary {
  margin-left: auto;
  color: var(--text-mute);
  font-variant-numeric: tabular-nums;
}
.ctx-chev {
  width: 16px;
  height: 16px;
  color: var(--text-mute);
  transition: transform 0.15s ease;
}
.ctx-chev.open {
  transform: rotate(90deg);
}

.ctx-bar {
  display: flex;
  gap: 1px;
  height: 8px;
  margin-top: 10px;
  border-radius: 999px;
  overflow: hidden;
  background: var(--ctx-track);
}
.ctx-seg {
  height: 100%;
  min-width: 1px;
}

.ctx-body {
  margin-top: 12px;
  display: flex;
  flex-direction: column;
}
.ctx-row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 3px 0;
  line-height: 1.5;
}
.ctx-dot {
  width: 10px;
  height: 10px;
  border-radius: 3px;
  flex: none;
}
.ctx-name {
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ctx-tokens {
  margin-left: auto;
  color: var(--text-mute);
  font-variant-numeric: tabular-nums;
}
.ctx-pct {
  width: 56px;
  text-align: right;
  color: var(--text-mute);
  font-variant-numeric: tabular-nums;
}

.ctx-detail {
  border-top: 1px solid var(--border);
  margin-top: 4px;
  padding-top: 4px;
}
.ctx-detail-head {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 3px 0;
  cursor: pointer;
  list-style: none;
}
.ctx-detail-head::-webkit-details-marker {
  display: none;
}
.ctx-detail-chev {
  width: 13px;
  height: 13px;
  color: var(--text-mute);
  flex: none;
  transition: transform 0.15s ease;
}
.ctx-detail[open] > .ctx-detail-head .ctx-detail-chev {
  transform: rotate(90deg);
}
.ctx-detail-head .ctx-name {
  color: var(--text-mute);
}
.ctx-detail-body {
  padding: 2px 0 6px 23px;
  display: flex;
  flex-direction: column;
}
.ctx-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 2px 0;
  font-size: 12.5px;
}
.ctx-item-main {
  color: var(--text);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.ctx-item-sub {
  margin-left: auto;
  color: var(--text-mute);
  font-variant-numeric: tabular-nums;
  flex: none;
}
</style>
