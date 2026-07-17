<script setup lang="ts">
import type { Agent } from '../types'
import {
  IconChat,
  IconGitBranch,
  IconRefresh,
  IconSplitH,
  IconSplitV,
  IconTerminal,
  agentIcons,
} from './icons'
import { t } from '../i18n'
import { chatSupported } from '../chatComposerOptions'

defineProps<{
  agent: Agent
  hasGit?: boolean
  showRefresh?: boolean
  showSplit?: boolean
}>()

defineEmits<{
  newSession: []
  newGui: []
  newShell: []
  gitChanges: []
  refresh: []
  splitH: []
  splitV: []
}>()
</script>

<template>
  <button type="button" class="new-menu-item" role="menuitem" @click="$emit('newSession')">
    <component :is="agentIcons[agent]" class="new-menu-ic" />
    <span>{{ t('list.action.newSessionTui') }}</span>
  </button>
  <button v-if="chatSupported(agent)" type="button" class="new-menu-item" role="menuitem" @click="$emit('newGui')">
    <IconChat class="new-menu-ic" />
    <span>{{ t('list.action.newSessionGui') }}</span>
  </button>
  <button type="button" class="new-menu-item" role="menuitem" @click="$emit('newShell')">
    <IconTerminal class="new-menu-ic" />
    <span>{{ t('list.action.newTerminal') }}</span>
  </button>
  <button v-if="hasGit" type="button" class="new-menu-item" role="menuitem" @click="$emit('gitChanges')">
    <IconGitBranch class="new-menu-ic" />
    <span>{{ t('list.action.gitChanges') }}</span>
  </button>
  <template v-if="showRefresh">
    <div class="new-menu-sep" role="separator" />
    <button type="button" class="new-menu-item" role="menuitem" @click="$emit('refresh')">
      <IconRefresh class="new-menu-ic" />
      <span>{{ t('list.action.refresh') }}</span>
    </button>
  </template>
  <template v-if="showSplit">
    <div class="new-menu-sep" role="separator" />
    <button type="button" class="new-menu-item" role="menuitem" @click="$emit('splitH')">
      <IconSplitH class="new-menu-ic" />
      <span>{{ t('pane.splitH') }}</span>
    </button>
    <button type="button" class="new-menu-item" role="menuitem" @click="$emit('splitV')">
      <IconSplitV class="new-menu-ic" />
      <span>{{ t('pane.splitV') }}</span>
    </button>
  </template>
</template>
