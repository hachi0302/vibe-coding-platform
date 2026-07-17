// 流式统计数据消费侧。
//
// 把 Tauri 后端 `start_agent_stats` 的事件流（stats://progress / stats://done /
// stats://error）封装成一组响应式 ref，让 StatsView / SessionStatsView 只关心
// 渲染。
//
// 设计点：
//   1. requestId 单调递增；任何到来的事件先比 id，过时的 partial / done 直接丢。
//   2. 进入 / 离开新 scope+range 时调 `start()` —— 内部先 bump id 再 invoke。
//   3. 一个组件实例配一个 `useStatsStream()` 实例：listener 在 onMounted 注册、
//      onUnmounted 卸载；不会跨实例共享。
//   4. 失败：error 事件写入 `error` ref，UI 切到 error 视图；前端不再尝试重试。
//   5. 取消：`stop()` 调 `api.cancelStats()` + 抹平本地 ref；切换 scope 也会
//      自动经过这条路径。

import { onUnmounted, ref, shallowRef, type Ref } from 'vue'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'

import * as api from './api'
import type {
  AgentStats,
  StatsDone,
  StatsError,
  StatsProgress,
  StatsRange,
} from './types'

export interface UseStatsStream {
  /** 累积快照。partial / done 事件都写它；finalized = true 表示已经收到 done。 */
  stats: Ref<AgentStats | null>
  /** 后端报告的已处理文件数 / 总文件数。stage = 'idle' 时为 0/0。 */
  progress: Ref<{ processed: number; total: number }>
  /** 当前状态机：idle = 还没启动；computing = 收到 progress 中；done = 收到 done；
   *  error = 收到 error。computing 期间 stats 可能已经有 partial 值。 */
  stage: Ref<'idle' | 'computing' | 'done' | 'error'>
  /** error 事件的描述串；非 error 阶段为空。 */
  error: Ref<string>
  /** 启动 / 重启一次扫描。会先 bump requestId + cancelStats，确保旧的 partial 不会再写进来。 */
  start: (scope: string, range: StatsRange) => Promise<void>
  /** 立刻取消并清空。组件 unmount 时会自动调一次。 */
  stop: () => Promise<void>
}

export function useStatsStream(): UseStatsStream {
  const stats = shallowRef<AgentStats | null>(null)
  const progress = ref<{ processed: number; total: number }>({ processed: 0, total: 0 })
  const stage = ref<'idle' | 'computing' | 'done' | 'error'>('idle')
  const error = ref<string>('')

  // 本实例已 listen 的句柄；start 时初始化、unmount 时一次性 unlisten。
  const unlisteners: UnlistenFn[] = []
  let listenerReady: Promise<void> | null = null
  // 本实例最近一次 start 的 requestId；事件 handler 比对，丢弃过时事件。
  let activeRequestId = 0

  async function ensureListeners(): Promise<void> {
    if (listenerReady) return listenerReady
    listenerReady = (async () => {
      const onProgress = await listen<StatsProgress>('stats://progress', (e) => {
        if (e.payload.requestId !== activeRequestId) return
        stats.value = e.payload.partial
        progress.value = { processed: e.payload.processed, total: e.payload.total }
        if (stage.value !== 'done' && stage.value !== 'error') {
          stage.value = 'computing'
        }
      })
      const onDone = await listen<StatsDone>('stats://done', (e) => {
        if (e.payload.requestId !== activeRequestId) return
        stats.value = e.payload.stats
        progress.value = {
          processed: progress.value.total || 1,
          total: progress.value.total || 1,
        }
        stage.value = 'done'
      })
      const onError = await listen<StatsError>('stats://error', (e) => {
        if (e.payload.requestId !== activeRequestId) return
        error.value = e.payload.error
        stage.value = 'error'
      })
      unlisteners.push(onProgress, onDone, onError)
    })()
    return listenerReady
  }

  async function start(scope: string, range: StatsRange): Promise<void> {
    await ensureListeners()
    // bump id，让任何已经在路上的事件被 handler 当过期丢弃
    activeRequestId = api.nextStatsRequestId()
    // 重置 UI 态：进入 computing 骨架；保留 stats（partial）会闪烁，所以清掉
    stats.value = null
    progress.value = { processed: 0, total: 0 }
    error.value = ''
    stage.value = 'computing'
    // 取消后端在跑的旧 worker —— 如果还有的话
    try {
      await api.cancelStats()
    } catch {
      // 后端没监听 cancel 也无所谓
    }
    try {
      await api.startAgentStats(scope, range, activeRequestId)
    } catch (e) {
      error.value = String(e)
      stage.value = 'error'
    }
  }

  async function stop(): Promise<void> {
    activeRequestId = 0
    stats.value = null
    progress.value = { processed: 0, total: 0 }
    stage.value = 'idle'
    error.value = ''
    try {
      await api.cancelStats()
    } catch {}
  }

  onUnmounted(() => {
    activeRequestId = 0
    unlisteners.forEach((u) => u())
    unlisteners.length = 0
    listenerReady = null
    // best-effort 通知后端：当前用户已经离开统计页
    api.cancelStats().catch(() => {})
  })

  return { stats, progress, stage, error, start, stop }
}
