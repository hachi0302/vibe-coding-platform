<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import type { TrashItem } from '../../types'
import { t } from '../../i18n'
import { shortName } from '../../format'
import {
  trashSearch,
  trashProject,
  trashProjects,
} from '../../trashToolbar'
import { useDebouncedSearch } from '../../useDebouncedSearch'
import {
  IconSearch,
  IconClose,
  IconCheck,
  IconChevronDown,
} from '../icons'

// `.ct-actions` 原本住在这里（排序 / 批量选择 / 批量恢复 / 取消），现已挪到
// TrashView 的 .list-head-actions 里，跟 Empty Trash 汇成一排，避免顶栏 +
// body header 两层 icon-only 按钮在同一垂直线上视觉冲突。
const props = defineProps<{ items: TrashItem[] }>()

// 搜索防抖 + IME 组合保护：见 useDebouncedSearch 的注释。
const {
  draft: searchDraft,
  commit: commitSearch,
  onInput: onSearchInput,
  onCompositionStart: onSearchCompStart,
  onCompositionEnd: onSearchCompEnd,
} = useDebouncedSearch(trashSearch, 220)
const hasQuery = computed(() => searchDraft.value.length > 0)
const projects = computed(() => trashProjects(props.items))

function clearSearch() {
  commitSearch('')
}

// ⌘F / Ctrl+F：回收站打开时拦截系统 Find，聚焦搜索框并全选。
// 只检测当前平台对应的修饰键，避免 macOS 上 Ctrl+F（光标右移）被误抢。
const searchInput = ref<HTMLInputElement>()
const isMac = /Mac/i.test(navigator.platform)
function onFindShortcut(e: KeyboardEvent) {
  if (e.key !== 'f' && e.key !== 'F') return
  const want = isMac ? e.metaKey : e.ctrlKey
  const other = isMac ? e.ctrlKey : e.metaKey
  if (!want || other || e.shiftKey || e.altKey) return
  e.preventDefault()
  searchInput.value?.focus()
  searchInput.value?.select()
}

// 项目筛选下拉 —— 与 ChatTopbar 的 scope 下拉共用 .ct-scope-* 样式。
// 下拉项与按钮只显示项目短名（projectLabel 通常是一长串绝对路径）。
const projMenuOpen = ref(false)
const projMenuEl = ref<HTMLElement>()
const projLabel = computed(() =>
  trashProject.value === 'all'
    ? t('trash.tb.allProjects')
    : shortName(trashProject.value),
)
function toggleProjMenu(e: Event) {
  e.stopPropagation()
  projMenuOpen.value = !projMenuOpen.value
}
function pickProject(p: string) {
  trashProject.value = p
  projMenuOpen.value = false
}
function onDocClick(e: MouseEvent) {
  if (!projMenuOpen.value) return
  if (projMenuEl.value && projMenuEl.value.contains(e.target as Node)) return
  projMenuOpen.value = false
}
onMounted(() => {
  document.addEventListener('click', onDocClick)
  window.addEventListener('keydown', onFindShortcut)
})
onUnmounted(() => {
  document.removeEventListener('click', onDocClick)
  window.removeEventListener('keydown', onFindShortcut)
})
</script>

<template>
  <div class="chat-topbar">
    <div class="ct-search" :class="{ active: hasQuery }">
      <div ref="projMenuEl" class="ct-scope-wrap">
        <button
          type="button"
          class="ct-scope-btn"
          :class="{ active: projMenuOpen }"
          v-tooltip:right="t('trash.tb.projectFilter')"
          @click="toggleProjMenu"
        >
          <span class="ct-scope-label">{{ projLabel }}</span>
          <IconChevronDown class="ct-scope-chev" />
        </button>
        <div v-if="projMenuOpen" class="ct-scope-menu" role="menu">
          <button
            type="button"
            class="ct-scope-item"
            :class="{ active: trashProject === 'all' }"
            role="menuitemradio"
            :aria-checked="trashProject === 'all'"
            @click="pickProject('all')"
          >
            <span class="ct-scope-check">
              <IconCheck v-if="trashProject === 'all'" />
            </span>
            <span>{{ t('trash.tb.allProjects') }}</span>
          </button>
          <button
            v-for="p in projects"
            :key="p"
            type="button"
            class="ct-scope-item"
            :class="{ active: trashProject === p }"
            role="menuitemradio"
            :aria-checked="trashProject === p"
            @click="pickProject(p)"
          >
            <span class="ct-scope-check">
              <IconCheck v-if="trashProject === p" />
            </span>
            <span>{{ shortName(p) }}</span>
          </button>
        </div>
      </div>
      <span class="ct-search-ic"><IconSearch /></span>
      <input
        ref="searchInput"
        :value="searchDraft"
        type="text"
        class="ct-search-input"
        :placeholder="t('trash.tb.searchPlaceholder')"
        spellcheck="false"
        autocomplete="off"
        @input="onSearchInput"
        @compositionstart="onSearchCompStart"
        @compositionend="onSearchCompEnd"
      />
      <button
        v-if="hasQuery"
        class="ct-btn"
        v-tooltip="t('chat.tb.search.clear')"
        @click="clearSearch"
      >
        <IconClose />
      </button>
    </div>
  </div>
</template>
