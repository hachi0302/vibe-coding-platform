<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { t } from '../i18n'
import { IconChevronRight } from './icons'

const props = withDefaults(
  defineProps<{
    maxHeight?: number
    enabled?: boolean
  }>(),
  { maxHeight: 320, enabled: true },
)

const expanded = ref(false)
const overflowing = ref(false)
const innerEl = ref<HTMLElement>()

let ro: ResizeObserver | null = null

function measure() {
  if (!props.enabled) {
    overflowing.value = false
    return
  }
  const el = innerEl.value
  if (!el) return
  overflowing.value = el.scrollHeight - 1 > props.maxHeight
}

onMounted(() => {
  nextTick(measure)
  if (typeof ResizeObserver !== 'undefined' && innerEl.value) {
    ro = new ResizeObserver(() => measure())
    ro.observe(innerEl.value)
  }
})
onBeforeUnmount(() => {
  ro?.disconnect()
})
watch(
  () => props.enabled,
  () => {
    if (!props.enabled) expanded.value = false
    nextTick(measure)
  },
)
</script>

<template>
  <slot v-if="!enabled" />
  <div
    v-else
    class="collapsible-box"
    :class="{ collapsed: overflowing && !expanded }"
  >
    <div
      ref="innerEl"
      class="collapsible-inner"
      :style="
        overflowing && !expanded ? { maxHeight: maxHeight + 'px' } : undefined
      "
    >
      <slot />
    </div>
    <button
      v-if="overflowing"
      class="collapsible-toggle"
      type="button"
      @click="expanded = !expanded"
    >
      <span class="chev" :class="{ open: expanded }"><IconChevronRight /></span>
      <span>{{
        expanded ? t('chat.collapse.less') : t('chat.collapse.more')
      }}</span>
    </button>
  </div>
</template>
