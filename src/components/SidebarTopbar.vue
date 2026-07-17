<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue'
import { t } from '../i18n'
import {
  IconSidebar,
  IconTrashOpen,
  IconChart,
  IconExportHistory,
  IconMore,
  IconPriceTag,
} from './icons'

defineProps<{
  showTrash: boolean
  showStats?: boolean
  showHistory?: boolean
  showPricing?: boolean
  hasTrash: boolean
}>()

const emit = defineEmits<{
  (e: 'toggle-sidebar'): void
  (e: 'open-trash'): void
  (e: 'open-stats'): void
  (e: 'open-history'): void
  (e: 'open-pricing'): void
}>()

// More-menu dropdown：原本独占一颗 IconExportHistory 按钮，现在折成一颗
// ⋯ 按钮 + 二选一菜单（Export history / Live pricing），给后续再加新入口留位置。
const menuOpen = ref(false)
const menuWrapEl = ref<HTMLElement>()
function toggleMenu(e: Event) {
  e.stopPropagation()
  menuOpen.value = !menuOpen.value
}
function pickHistory() {
  menuOpen.value = false
  emit('open-history')
}
function pickPricing() {
  menuOpen.value = false
  emit('open-pricing')
}
function onDocClick(e: MouseEvent) {
  if (!menuOpen.value) return
  if (menuWrapEl.value && menuWrapEl.value.contains(e.target as Node)) return
  menuOpen.value = false
}
onMounted(() => document.addEventListener('click', onDocClick))
onUnmounted(() => document.removeEventListener('click', onDocClick))
</script>

<template>
  <div class="topbar-sidebar-zone">
    <div class="topbar-icons">
      <button
        class="top-btn"
        v-tooltip="t('sidebar.toggle')"
        @click="emit('toggle-sidebar')"
      >
        <IconSidebar />
      </button>
    </div>
    <div class="topbar-icons">
      <button
        class="top-btn"
        :class="{ active: showStats }"
        v-tooltip="t('sidebar.stats')"
        @click="emit('open-stats')"
      >
        <IconChart />
      </button>
      <button
        class="top-btn topbar-trash-btn"
        :class="{ active: showTrash }"
        v-tooltip="t('sidebar.trash')"
        @click="emit('open-trash')"
      >
        <IconTrashOpen />
        <span v-if="hasTrash" class="trash-dot" aria-hidden="true" />
      </button>
      <div ref="menuWrapEl" class="topbar-more-wrap">
        <button
          type="button"
          class="top-btn"
          :class="{ active: menuOpen || showHistory || showPricing }"
          v-tooltip="t('sidebar.more')"
          :aria-expanded="menuOpen"
          aria-haspopup="menu"
          @click="toggleMenu"
        >
          <IconMore />
        </button>
        <div v-if="menuOpen" class="topbar-more-menu" role="menu">
          <button
            type="button"
            class="topbar-more-item"
            :class="{ active: showHistory }"
            role="menuitem"
            @click="pickHistory"
          >
            <span class="topbar-more-icon"><IconExportHistory /></span>
            <span>{{ t('sidebar.history') }}</span>
          </button>
          <button
            type="button"
            class="topbar-more-item"
            :class="{ active: showPricing }"
            role="menuitem"
            @click="pickPricing"
          >
            <span class="topbar-more-icon"><IconPriceTag /></span>
            <span>{{ t('sidebar.pricing') }}</span>
          </button>
        </div>
      </div>
    </div>
  </div>
</template>
