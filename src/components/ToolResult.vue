<script setup lang="ts">
import { computed } from 'vue'
import type { Block } from '../types'
import { t } from '../i18n'
import DiffBlock from './DiffBlock.vue'
import CollapsibleBox from './CollapsibleBox.vue'
import { IconChevronRight } from './icons'
import { highlightJsonInPlace, looksLikeJson } from '../jsonHighlight'
import { highlightDiff, looksLikeDiff } from '../diffHighlight'

const props = withDefaults(defineProps<{ block: Block; inUser?: boolean; persistOpen?: boolean }>(), {
  persistOpen: undefined,
})
const emit = defineEmits<{ toggle: [open: boolean] }>()

// 结果文本的渲染优先级：
//   1. structured diff（block.diff，有 hunks）→ DiffBlock（保留交互）
//   2. 文本形态的 unified diff（Bash 跑 git diff / 工具吐 patch）→ 行级染色
//   3. JSON（含 Read .json 文件的 cat-n 行号格式）→ token 上色
//   4. 其它 → 原样 <pre>
// 判断顺序很重要：JSON 文件的 diff 既像 diff 又像 JSON，应该按 diff 渲染。
const diffHtml = computed(() => {
  const txt = props.block.text ?? ''
  if (!looksLikeDiff(txt)) return null
  return highlightDiff(txt)
})
const jsonHtml = computed(() => {
  const txt = props.block.text ?? ''
  if (!looksLikeJson(txt)) return null
  return highlightJsonInPlace(txt)
})

function baseName(p?: string): string {
  if (!p) return ''
  const parts = p.split('/').filter(Boolean)
  return parts.length ? parts[parts.length - 1] : p
}

const label = computed(() => {
  if (props.block.diff)
    return t('tool.resultDiff', { file: baseName(props.block.filePath) })
  return props.block.isError ? t('tool.resultError') : t('tool.result')
})

const diffStat = computed(() => {
  if (!props.block.diff) return ''
  let add = 0
  let del = 0
  for (const h of props.block.diff)
    for (const l of h.lines) {
      if (l.kind === 'add') add++
      else if (l.kind === 'del') del++
    }
  return `+${add} −${del}`
})

const hasRenderableText = computed(() => {
  if (props.block.diff) return true
  return !!(props.block.text ?? '').trim()
})
</script>

<template>
  <details
    v-if="hasRenderableText"
    class="block-card"
    :class="{ 'in-user': inUser, 'auto-open': !!block.diff }"
    :open="persistOpen ?? !!block.diff"
    @toggle="emit('toggle', ($event.target as HTMLDetailsElement).open)"
  >
    <summary class="block-summary">
      <span class="chev"><IconChevronRight /></span>
      <span class="label" :class="{ error: block.isError }">{{ label }}</span>
      <span v-if="diffStat" class="diff-stat">{{ diffStat }}</span>
    </summary>
    <div class="block-body">
      <DiffBlock v-if="block.diff" :hunks="block.diff" :file-path="block.filePath" class="diff-scroll" />
      <CollapsibleBox v-else :max-height="400">
        <pre v-if="diffHtml" class="lang-diff" v-html="diffHtml" />
        <pre v-else-if="jsonHtml" class="lang-json" v-html="jsonHtml" />
        <pre v-else>{{ block.text }}</pre>
      </CollapsibleBox>
    </div>
  </details>
</template>
