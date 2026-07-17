<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import { t } from '../../i18n'
import {
  tuiSearch,
  tuiSearchCount,
  tuiSearchIndex,
  tuiNavigate,
  setTuiSearchFocuser,
} from '../../tuiToolbar'
import {
  IconSearch,
  IconChevronUp,
  IconChevronDown,
  IconClose,
} from '../icons'
import { useDebouncedSearch } from '../../useDebouncedSearch'

const searchInput = ref<HTMLInputElement>()

const {
  draft: searchDraft,
  commit: commitSearch,
  onInput: onSearchInput,
  onCompositionStart: onSearchCompStart,
  onCompositionEnd: onSearchCompEnd,
} = useDebouncedSearch(tuiSearch, 280)

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
  setTuiSearchFocuser(focusSearch)
})
onUnmounted(() => {
  window.removeEventListener('keydown', onFindShortcut)
  setTuiSearchFocuser(null)
})

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Enter') {
    e.preventDefault()
    if (tuiSearchCount.value === 0) return
    tuiNavigate(e.shiftKey ? -1 : 1)
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
</script>

<template>
  <div class="chat-topbar">
    <div class="ct-search" :class="{ active: hasQuery }">
      <span class="ct-search-ic"><IconSearch /></span>
      <input
        ref="searchInput"
        :value="searchDraft"
        type="text"
        class="ct-search-input"
        :placeholder="t('tui.tb.search.placeholder')"
        spellcheck="false"
        autocomplete="off"
        @input="onSearchInput"
        @compositionstart="onSearchCompStart"
        @compositionend="onSearchCompEnd"
        @keydown="onKeydown"
      />
      <template v-if="hasQuery">
        <span class="ct-search-count" :class="{ none: tuiSearchCount === 0 }">
          {{
            tuiSearchCount === 0
              ? t('chat.tb.search.none')
              : t('chat.tb.search.count', { cur: tuiSearchIndex, total: tuiSearchCount })
          }}
        </span>
        <button
          class="ct-btn"
          :disabled="tuiSearchCount === 0"
          v-tooltip="t('chat.tb.search.prev')"
          @click="tuiNavigate(-1)"
        >
          <IconChevronUp />
        </button>
        <button
          class="ct-btn"
          :disabled="tuiSearchCount === 0"
          v-tooltip="t('chat.tb.search.next')"
          @click="tuiNavigate(1)"
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
