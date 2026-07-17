<script setup lang="ts">
import type { DiffHunk } from '../types'

defineProps<{ hunks: DiffHunk[]; filePath?: string }>()
</script>

<template>
  <div class="diff" :data-file="filePath || undefined">
    <template v-for="(h, hi) in hunks" :key="hi">
      <div v-if="hi > 0" class="diff-sep">···</div>
      <div
        v-for="(ln, li) in h.lines"
        :key="li"
        class="diff-line"
        :class="ln.kind"
      >
        <span class="diff-no">{{ ln.kind === 'add' ? ln.newNo : ln.oldNo }}</span>
        <span class="diff-sign">{{
          ln.kind === 'add' ? '+' : ln.kind === 'del' ? '-' : ' '
        }}</span>
        <span class="diff-text">{{ ln.text }}</span>
      </div>
    </template>
  </div>
</template>
