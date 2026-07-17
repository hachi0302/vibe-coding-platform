<script setup lang="ts">
// 递归渲染一棵分屏树。
//   leaf  → 一个 <PaneContent>（按该叶子的 pane 渲染 strip + 会话/列表/欢迎 + TUI 层）。
//   split → 按 dir 铺成 flex 行/列，孩子之间插一根可拖拽的 divider（.pane-resizer）；
//           每个孩子按 sizes[i] 占比（flex-grow）。孩子本身可能又是 split，于是无限递归。
//
// 叶子要的那一坨「当前项目视图数据」（activeProject / sessions / loading…）对所有 pane 都相同
// （同一 (agent, project)），这里原样透传给 PaneContent 和更深一层的 PaneGrid。
//
// 组件在自己的模板里按文件名递归引用自身（Vue SFC 支持），无需显式 import 自己。

import { computed } from 'vue'
import type { Agent, ProjectInfo, SessionMeta, TrashItem } from '../types'
import { type PaneNode, paneOf, setSplitSizes, leafPaneIds } from '../panes'
import PaneContent from './PaneContent.vue'

const props = defineProps<{
  node: PaneNode
  activeProject: ProjectInfo | undefined
  agent: Agent
  projects: ProjectInfo[]
  sessions: SessionMeta[]
  sessionTotal: number
  loadingList: boolean
  loadingMore: boolean
  openTrashItem: TrashItem | null
  hasGit: boolean
}>()

// 透传给叶子 / 更深 PaneGrid 的数据包（除 node 外的所有 prop）。
const leafProps = computed(() => ({
  activeProject: props.activeProject,
  agent: props.agent,
  projects: props.projects,
  sessions: props.sessions,
  sessionTotal: props.sessionTotal,
  loadingList: props.loadingList,
  loadingMore: props.loadingMore,
  openTrashItem: props.openTrashItem,
  hasGit: props.hasGit,
}))

// 子树稳定 key：用该子树最左/最上的叶子 paneId（唯一，且只有该叶子被移走才变）。
function childKey(child: PaneNode): number {
  return leafPaneIds(child)[0]
}

// —— divider 拖拽：调整某 split 内相邻两格的占比 ——
const MIN_FRAC = 0.1
function startResize(e: PointerEvent, split: Extract<PaneNode, { kind: 'split' }>, i: number) {
  e.preventDefault()
  const container = (e.currentTarget as HTMLElement).closest('.pane-split') as HTMLElement | null
  if (!container) return
  const rect = container.getBoundingClientRect()
  const horiz = split.dir === 'row'
  const total = horiz ? rect.width : rect.height
  if (total <= 0) return
  const startPos = horiz ? e.clientX : e.clientY
  const a0 = split.sizes[i]
  const b0 = split.sizes[i + 1]
  const move = (ev: PointerEvent) => {
    const delta = ((horiz ? ev.clientX : ev.clientY) - startPos) / total
    let a = a0 + delta
    let b = b0 - delta
    if (a < MIN_FRAC) { b -= MIN_FRAC - a; a = MIN_FRAC }
    if (b < MIN_FRAC) { a -= MIN_FRAC - b; b = MIN_FRAC }
    const next = split.sizes.slice()
    next[i] = a
    next[i + 1] = b
    setSplitSizes(split, next)
  }
  const up = () => {
    window.removeEventListener('pointermove', move)
    window.removeEventListener('pointerup', up)
    window.removeEventListener('pointercancel', up)
  }
  window.addEventListener('pointermove', move)
  window.addEventListener('pointerup', up)
  window.addEventListener('pointercancel', up)
}
</script>

<template>
  <!-- 叶子：一个格子 -->
  <PaneContent
    v-if="node.kind === 'leaf' && paneOf(node.paneId)"
    :pane="paneOf(node.paneId)!"
    v-bind="leafProps"
  />

  <!-- 分屏：行/列 flex 容器 + 孩子之间的 divider -->
  <div v-else-if="node.kind === 'split'" class="pane-split" :class="node.dir">
    <template v-for="(child, i) in node.children" :key="childKey(child)">
      <div class="pane-cell" :style="{ flexGrow: node.sizes[i], flexBasis: '0%' }">
        <PaneGrid :node="child" v-bind="leafProps" />
      </div>
      <div
        v-if="i < node.children.length - 1"
        class="pane-resizer"
        :class="node.dir"
        @pointerdown="startResize($event, node, i)"
      />
    </template>
  </div>
</template>
