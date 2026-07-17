// 客户端 slash 指令分类器 —— 把 GUI 输入框里**被拦截、不发给 agent** 的系统命令归一成动作。
//
// 背景：Claude / Codex 的不少内置斜杠命令在 headless stream-json 模式下并不可用，但在桌面
// 客户端里它们是「客户端动作」：展开导出菜单（/export）、打开重命名框（/rename）、清屏 +
// 重置上下文（/clear）、fork 会话（/fork）、展开底部模型面板（/model）、转交侧聊（/btw）。
// 这里只做**分类**，具体动作由 ChatComposer 分派。返回 null = 不是客户端命令，照常发给 agent
// （如 /compact、/context、/reload-skills 等真·CLI 命令，以及普通 prose）。
//
// 纯函数、无副作用，便于单测。

export type ChatSlashAction =
  | { kind: 'btw'; prompt?: string }
  | { kind: 'export' }
  | { kind: 'rename' }
  | { kind: 'clear' }
  | { kind: 'fork' }
  | { kind: 'model' }
  | { kind: 'archive' }

/**
 * 把一条 composer 输入归类成客户端 slash 动作；不是则返回 null（照常发送）。
 * 仅当整行就是该命令时才拦截：`/btw` 可带提示词，其余几个不收参数 —— 带尾随文本即视为
 * 普通发送（避免误吞 `/clear all`、`/export now` 之类）。命令名不分大小写，允许前后空白。
 */
export function parseChatSlashAction(input: string): ChatSlashAction | null {
  const body = input.trim()
  const btw = /^\/btw(?:\s+([\s\S]*))?$/i.exec(body)
  if (btw) return { kind: 'btw', prompt: (btw[1] ?? '').trim() || undefined }
  if (/^\/export(\s|$)/i.test(body)) return { kind: 'export' }
  if (/^\/rename(\s|$)/i.test(body)) return { kind: 'rename' }
  if (/^\/clear$/i.test(body)) return { kind: 'clear' }
  if (/^\/fork$/i.test(body)) return { kind: 'fork' }
  if (/^\/model(\s|$)/i.test(body)) return { kind: 'model' }
  if (/^\/archive(\s|$)/i.test(body)) return { kind: 'archive' }
  return null
}
