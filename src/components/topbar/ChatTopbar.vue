<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { t } from '../../i18n'
import {
  search,
  searchCount,
  searchIndex,
  searchScope,
  navigate,
  setSearchFocuser,
} from '../../chatToolbar'
import {
  IconSearch,
  IconChevronUp,
  IconChevronDown,
  IconClose,
  IconCheck,
} from '../icons'
import type { SearchScope } from '../../chatToolbar'
import { useDebouncedSearch } from '../../useDebouncedSearch'

// 「会话统计」和「折叠/展开 Tool calls」原本住在这里的 .ct-actions 里 ——
// 与 chat-head 的 5 个会话级 icon 按钮垂直叠两行，扫描时眼睛要在两层
// 间来回找。现已挪进 chat-head 里，topbar 只保留 scope+search 一条横线。
const searchInput = ref<HTMLInputElement>()

// 防抖 + IME 组合保护。长会话里 search 触发的高亮 + 计数遍历较重，延时给到 280ms。
const {
  draft: searchDraft,
  commit: commitSearch,
  onInput: onSearchInput,
  onCompositionStart: onSearchCompStart,
  onCompositionEnd: onSearchCompEnd,
} = useDebouncedSearch(search, 280)

// ⌘F / Ctrl+F：聊天页打开时（即本组件挂载时）拦截系统 Find，聚焦搜索框并全选。
// 只检测当前平台对应的修饰键，避免 macOS 上 Ctrl+F（光标右移）被误抢。
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
function focusSearch() {
  searchInput.value?.focus()
  searchInput.value?.select()
}
onMounted(() => {
  window.addEventListener('keydown', onFindShortcut)
  // 暴露 focus 入口给原生菜单的「Find in Session…」用。⌘F window 监听仍保留，
  // 因为 macOS 菜单 accelerator 触发时 webview keydown 通常被吃掉、两者不会重叠。
  setSearchFocuser(focusSearch)
})
onUnmounted(() => {
  window.removeEventListener('keydown', onFindShortcut)
  setSearchFocuser(null)
})

function onKeydown(e: KeyboardEvent) {
  // Enter / Shift+Enter 在搜索框里跳下一个 / 上一个
  if (e.key === 'Enter') {
    e.preventDefault()
    if (searchCount.value === 0) return
    navigate(e.shiftKey ? -1 : 1)
  } else if (e.key === 'Escape') {
    e.preventDefault()
    clearSearch()
  }
}

function clearSearch() {
  commitSearch('')
  searchInput.value?.blur()
}

const hasQuery = computed(() => searchDraft.value.length > 0)

// 自定义 scope 下拉（替代原生 <select>），跟导出菜单使用同一套样式
const scopeMenuOpen = ref(false)
const scopeMenuEl = ref<HTMLElement>()
const SCOPES: { value: SearchScope; key: string }[] = [
  { value: 'all', key: 'chat.tb.scope.all' },
  { value: 'user', key: 'chat.tb.scope.user' },
  { value: 'agent', key: 'chat.tb.scope.agent' },
  { value: 'tools', key: 'chat.tb.scope.tools' },
]
const scopeLabel = computed(() => {
  const found = SCOPES.find((s) => s.value === searchScope.value)
  return t(found?.key ?? 'chat.tb.scope.all')
})
function toggleScopeMenu(e: Event) {
  e.stopPropagation()
  scopeMenuOpen.value = !scopeMenuOpen.value
}
function pickScope(s: SearchScope) {
  searchScope.value = s
  scopeMenuOpen.value = false
}
function onDocClick(e: MouseEvent) {
  if (!scopeMenuOpen.value) return
  if (scopeMenuEl.value && scopeMenuEl.value.contains(e.target as Node)) return
  scopeMenuOpen.value = false
}
onMounted(() => document.addEventListener('click', onDocClick))
onUnmounted(() => document.removeEventListener('click', onDocClick))
</script>

<template>
  <div class="chat-topbar">
    <div class="ct-search" :class="{ active: hasQuery }">
      <div ref="scopeMenuEl" class="ct-scope-wrap">
        <button
          type="button"
          class="ct-scope-btn"
          :class="{ active: scopeMenuOpen }"
          v-tooltip:right="t('chat.tb.scope.tooltip')"
          @click="toggleScopeMenu"
        >
          <span class="ct-scope-label">{{ scopeLabel }}</span>
          <IconChevronDown class="ct-scope-chev" />
        </button>
        <div v-if="scopeMenuOpen" class="ct-scope-menu" role="menu">
          <button
            v-for="s in SCOPES"
            :key="s.value"
            type="button"
            class="ct-scope-item"
            :class="{ active: searchScope === s.value }"
            role="menuitemradio"
            :aria-checked="searchScope === s.value"
            @click="pickScope(s.value)"
          >
            <span class="ct-scope-check">
              <IconCheck v-if="searchScope === s.value" />
            </span>
            <span>{{ t(s.key) }}</span>
          </button>
        </div>
      </div>
      <span class="ct-search-ic"><IconSearch /></span>
      <input
        ref="searchInput"
        :value="searchDraft"
        type="text"
        class="ct-search-input"
        :placeholder="t('chat.tb.search.placeholder')"
        spellcheck="false"
        autocomplete="off"
        @input="onSearchInput"
        @compositionstart="onSearchCompStart"
        @compositionend="onSearchCompEnd"
        @keydown="onKeydown"
      />
      <template v-if="hasQuery">
        <span class="ct-search-count" :class="{ none: searchCount === 0 }">
          {{
            searchCount === 0
              ? t('chat.tb.search.none')
              : t('chat.tb.search.count', { cur: searchIndex, total: searchCount })
          }}
        </span>
        <button
          class="ct-btn"
          :disabled="searchCount === 0"
          v-tooltip="t('chat.tb.search.prev')"
          @click="navigate(-1)"
        >
          <IconChevronUp />
        </button>
        <button
          class="ct-btn"
          :disabled="searchCount === 0"
          v-tooltip="t('chat.tb.search.next')"
          @click="navigate(1)"
        >
          <IconChevronDown />
        </button>
        <button
          class="ct-btn"
          v-tooltip="t('chat.tb.search.clear')"
          @click="clearSearch"
        >
          <IconClose />
        </button>
      </template>
    </div>
  </div>
</template>
