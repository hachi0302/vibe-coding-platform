import { ref } from 'vue'
import type { Agent } from './types'

// 「已导出会话」历史 —— 记录被导出过的原始会话，按时间倒序存 localStorage。
//
// 关键点：这里存的是**原始会话**的引用（agent + 原始 JSONL 路径），不是导出产物。
// 点开一条历史 = 用平时查看会话的同一套逻辑（read_session）重新打开那个原始 transcript，
// 跟落盘的 md/html/json 文件没有任何关系。
// 局限：1) 只记录加这个功能之后导出过的会话。
//       2) 原始文件被移动/删除后条目会失效 —— 打开时后端报错，列表项提供"移除"。

const KEY = 'exportHistory:v1'
const CAP = 50

export interface ExportRecord {
  /** 原始会话 JSONL 的绝对路径 —— 既是打开入口，也是去重键。 */
  path: string
  title: string
  agent: Agent
  sessionId: string
  cwd?: string
  /** 导出时刻（Date.now()），列表按它倒序。 */
  exportedAt: number
}

function load(): ExportRecord[] {
  try {
    const arr = JSON.parse(localStorage.getItem(KEY) ?? '[]')
    if (!Array.isArray(arr)) return []
    // 丢掉不符合当前形状的条目 —— 历史版本曾按导出文件路径（filePath）存，
    // 现在按原始会话（path）存；旧条目缺 path，留着会在渲染时炸。
    return arr.filter(
      (r): r is ExportRecord =>
        r && typeof r.path === 'string' && typeof r.agent === 'string',
    )
  } catch {
    return []
  }
}

function persist() {
  localStorage.setItem(KEY, JSON.stringify(history.value))
}

/** 响应式快照：历史页读它；写操作整体替换以触发刷新。 */
export const history = ref<ExportRecord[]>(load())

/** 记录一次导出：同一原始会话去重提到队首，截断到 CAP。 */
export function recordExport(rec: ExportRecord) {
  const next = [rec, ...history.value.filter((r) => r.path !== rec.path)].slice(
    0,
    CAP,
  )
  history.value = next
  persist()
}

/** 从历史里移除一条（原始文件已失效 / 用户手动删）。 */
export function removeExport(path: string) {
  if (!history.value.some((r) => r.path === path)) return
  history.value = history.value.filter((r) => r.path !== path)
  persist()
}

/** 清空整个导出历史。 */
export function clearExportHistory() {
  if (!history.value.length) return
  history.value = []
  persist()
}
