<script setup lang="ts">
import type { ProjectInfo } from '../types'
import { t } from '../i18n'
import {
  IconPinUp,
  IconPinDown,
  IconRefresh,
  IconTrashOpen,
  IconFolder,
  IconGitBranch,
  IconZap,
} from '../components/icons'

type ProjState = 'pinned' | 'sunk'

const props = defineProps<{
  x: number
  y: number
  project: ProjectInfo
  projState: ProjState | undefined
  /** 该项目所在目录是否是 git 仓库（开菜单时异步探测，决定是否显示「创建 Worktree」）。 */
  isGitRepo: boolean
  /** 仅由平台写入的初始化标识判定；已完成后不允许重复启动。 */
  initialized?: boolean
}>()

const emit = defineEmits<{
  (e: 'toggle-state', state: ProjState): void
  (e: 'refresh'): void
  (e: 'open-folder'): void
  (e: 'delete'): void
  (e: 'remove-bookmark'): void
  (e: 'create-worktree'): void
  (e: 'delete-worktree'): void
  (e: 'initialize-project'): void
}>()

// worktree 条目自带 worktreeName；据此把底部删除项切成「删除 Worktree」，
// 并隐藏「创建 Worktree」（不支持 worktree 套 worktree）。
const isWorktree = () => !!props.project.worktreeName
</script>

<template>
  <Teleport to="body">
    <div class="ctx-menu" :style="{ left: x + 'px', top: y + 'px' }">
      <button class="ctx-item" @click="emit('toggle-state', 'pinned')">
        <IconPinUp />
        {{ projState === 'pinned' ? t('proj.unpin') : t('proj.pin') }}
      </button>
      <button class="ctx-item" @click="emit('toggle-state', 'sunk')">
        <IconPinDown />
        {{ projState === 'sunk' ? t('proj.unsink') : t('proj.sink') }}
      </button>
      <div class="ctx-sep" />
      <!-- 目录已不存在 → 刷新无意义，连同分隔线一起隐藏 -->
      <template v-if="project.exists">
        <button class="ctx-item" @click="emit('open-folder')">
          <IconFolder />
          {{ t('proj.openFolder') }}
        </button>
        <button
          class="ctx-item"
          data-menu-action="initialize-project"
          :disabled="initialized"
          @click="emit('initialize-project')"
        >
          <IconZap />
          {{ initialized ? t('proj.initialized') : t('proj.initialize') }}
        </button>
        <button class="ctx-item" @click="emit('refresh')">
          <IconRefresh />
          {{ t('proj.refresh') }}
        </button>
        <button
          v-if="isGitRepo && !isWorktree()"
          class="ctx-item"
          @click="emit('create-worktree')"
        >
          <IconGitBranch />
          {{ t('proj.createWorktree') }}
        </button>
        <div class="ctx-sep" />
      </template>
      <button
        v-if="isWorktree()"
        class="ctx-item danger"
        @click="emit('delete-worktree')"
      >
        <IconTrashOpen />
        {{ t('proj.deleteWorktree') }}
      </button>
      <button v-else class="ctx-item danger" @click="emit('delete')">
        <IconTrashOpen />
        {{ t('proj.delete') }}
      </button>
    </div>
  </Teleport>
</template>
