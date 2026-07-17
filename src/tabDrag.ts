// 跨 pane / pane 内 tab 拖拽的统一模型。
//
// strip 里的活 tab（tui 终端/会话 + view 只读/chat）在同一条时间线（createdAt 升序）上并排，
// 所以：
//   · 「排序」= 改 createdAt（落到目标时间线相邻两 tab 之间取中点）；
//   · 「跨屏移动」= 改 paneId，再落到目标 pane 时间线的合适位置。
// saved（重启后的懒 pill）不参与拖拽 —— Phase 5 给它 paneId 后再说。
//
// dragState 是一小份全局共享拖拽态：源 strip 注册 window pointermove 后写它，各 strip /
// PaneContent 读它来画「落点线 / 拖到我这」高亮。跨屏时光标停在别的 strip 实例上，源 strip
// 的落点目标 tab 在另一个组件里，只有共享态能让目标那格自己亮起来。

import { reactive } from 'vue'
import type { Agent } from './types'
import { tabs, type TerminalTab } from './terminals'
import { viewTabs, type ViewTab } from './viewTabs'
import { panes, focusPane } from './panes'

export type DragKind = 'tui' | 'view'
export interface DragRef {
  kind: DragKind
  uiId: number
}

export const dragState = reactive<{
  active: boolean
  source: DragRef | null
  sourcePaneId: number | null
  /** 光标当前悬停在哪个 pane 上（跨屏高亮用）。 */
  overPaneId: number | null
  /** 落点线锚定的可见 tab 序号（over-pane 的 unifiedTabs.orderIndex）；null → 不画线。 */
  dropIndex: number | null
  dropPosition: 'before' | 'after'
  /** 落点两侧可见邻居的 createdAt 边界（含 saved 懒 pill）；松手时取中点。null = 该侧到头。 */
  dropBefore: number | null
  dropAfter: number | null
  /** 松手是否应执行移动（同 pane 落回原槽位 = false）。 */
  dropReady: boolean
}>({
  active: false,
  source: null,
  sourcePaneId: null,
  overPaneId: null,
  dropIndex: null,
  dropPosition: 'after',
  dropBefore: null,
  dropAfter: null,
  dropReady: false,
})

export function resetDragState() {
  dragState.active = false
  dragState.source = null
  dragState.sourcePaneId = null
  dragState.overPaneId = null
  dragState.dropIndex = null
  dragState.dropPosition = 'after'
  dragState.dropBefore = null
  dragState.dropAfter = null
  dragState.dropReady = false
}

export function sameRef(a: DragRef | null, b: DragRef | null): boolean {
  return !!a && !!b && a.kind === b.kind && a.uiId === b.uiId
}

function findTab(ref: DragRef): TerminalTab | ViewTab | undefined {
  return ref.kind === 'tui'
    ? tabs.value.find((t) => t.uiId === ref.uiId)
    : viewTabs.value.find((t) => t.uiId === ref.uiId)
}

export interface PaneTabItem {
  ref: DragRef
  order: number
}

/** 某 pane 里全部可拖 tab（tui + view），按 createdAt 升序 —— 即 strip 里的可见顺序。 */
export function paneTabItems(agent: Agent, projectKey: string, paneId: number): PaneTabItem[] {
  const items: PaneTabItem[] = []
  for (const t of tabs.value) {
    if (t.agent === agent && t.projectKey === projectKey && t.paneId === paneId) {
      items.push({ ref: { kind: 'tui', uiId: t.uiId }, order: t.createdAt })
    }
  }
  for (const v of viewTabs.value) {
    if (v.agent === agent && v.projectKey === projectKey && v.paneId === paneId) {
      items.push({ ref: { kind: 'view', uiId: v.uiId }, order: v.createdAt })
    }
  }
  items.sort((a, b) => a.order - b.order)
  return items
}

/** 在 pane 里激活某 tab（露出对应层）。 */
function activateInPane(paneId: number, ref: DragRef) {
  const pane = panes.get(paneId)
  if (!pane) return
  if (ref.kind === 'tui') {
    pane.activeUiId = ref.uiId
    pane.activeViewTabId = null
  } else {
    pane.activeViewTabId = ref.uiId
    pane.activeUiId = null
  }
}

/**
 * 把 source 移到 targetPaneId，落到可见邻居的 createdAt 边界 (before, after) 之间（某侧为
 * null = 该侧到头）。边界由调用方按「整条可见 strip（含 saved 懒 pill）」算好传入，这样活 tab
 * 之间夹着 saved pill 时也能精确落位、不会跳到 pill 后面。重排靠改 createdAt（取两邻居中点）；
 * 跨屏再改 paneId 并把该 tab 在目标格子激活、聚焦目标格子；源格子若正显示这个 tab 则回退到它的
 * 上一个 view tab / 列表。返回是否发生变化。
 */
export function moveTabTo(
  source: DragRef,
  targetPaneId: number,
  before: number | null,
  after: number | null,
): boolean {
  const src = findTab(source)
  if (!src) return false
  const oldPaneId = src.paneId

  let newOrder: number
  if (before == null && after == null) newOrder = src.createdAt
  else if (before == null) newOrder = after! - 1000
  else if (after == null) newOrder = before + 1000
  else newOrder = (before + after) / 2

  if (oldPaneId === targetPaneId && newOrder === src.createdAt) return false

  src.createdAt = newOrder
  src.paneId = targetPaneId

  if (oldPaneId !== targetPaneId) {
    const oldPane = panes.get(oldPaneId)
    if (oldPane) {
      if (source.kind === 'tui' && oldPane.activeUiId === source.uiId) {
        oldPane.activeUiId = null
      }
      if (source.kind === 'view' && oldPane.activeViewTabId === source.uiId) {
        const restViews = paneTabItems(src.agent, src.projectKey, oldPaneId).filter(
          (it) => it.ref.kind === 'view',
        )
        const last = restViews[restViews.length - 1]
        oldPane.activeViewTabId = last ? last.ref.uiId : null
      }
    }
    activateInPane(targetPaneId, source)
    focusPane(targetPaneId)
  }
  return true
}
