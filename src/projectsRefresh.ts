// 「项目列表会话计数已过期」信号。
//
// 侧栏每个项目的会话计数来自后端 list_projects，只在显式 loadProjects() 时刷新，没有文件
// 监听。于是「新建会话」后计数会停在旧值 —— 尤其空 worktree（合成条目计数恒为 0），在里面
// 跑出第一个会话后侧栏仍显示 0，直到用户手动刷新。
//
// 这里提供一个极简的事件位：会话落盘的那一刻（GUI chat 首次拿到 sessionId / TUI 起新会话）
// 调 markProjectsDirty()，App 订阅 projectsDirty 做去抖 loadProjects()。独立成模块是为了
// 避免 chatSessions / terminals → App 的循环依赖（对齐 usage.ts 的 bumpUsage 模式）。
import { ref } from 'vue'

/** 单调自增计数；每次自增 = 一次「请重载项目列表」请求。App watch 它触发去抖刷新。 */
export const projectsDirty = ref(0)

export function markProjectsDirty(): void {
  projectsDirty.value++
}
