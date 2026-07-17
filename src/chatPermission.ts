// 交互式工具权限（Claude `--permission-prompt-tool stdio` 的 `can_use_tool`）的纯逻辑：
// 把请求里的工具参数提炼成可读预览，以及把用户的三选一构造成 CLI 控制协议的 decision。
// 不依赖 Vue / Tauri，便于单测；ChatView / ChatPermissionPrompt 与 chatSessions 共用。

import type { ChatPermissionRequest } from './types'

export type PermissionChoice = 'allow-once' | 'always-allow' | 'deny'

/** 把 request.input 当对象安全读取（CLI 总会带工具参数对象，防御性兜底）。 */
function inputObj(req: ChatPermissionRequest): Record<string, unknown> {
  return req.input && typeof req.input === 'object' && !Array.isArray(req.input)
    ? (req.input as Record<string, unknown>)
    : {}
}

/** 该工具最值得展示的「这要干什么」一行 —— Bash 显示命令、文件工具显示路径，否则 undefined
 *  （只靠 CLI 的 description）。返回的字符串原样进 `<pre>`，不做截断（CSS 控制溢出）。 */
export function permissionCommandPreview(req: ChatPermissionRequest): string | undefined {
  const input = inputObj(req)
  const str = (k: string): string | undefined =>
    typeof input[k] === 'string' && (input[k] as string).length ? (input[k] as string) : undefined
  if (req.toolName === 'Bash' || req.toolName === 'shell') return str('command')
  // 文件类工具（Write / Edit / Read / NotebookEdit …）统一展示目标路径。
  return str('file_path') ?? str('path') ?? str('notebook_path') ?? str('pattern') ?? str('url')
}

/** 「始终允许」是否可用 —— CLI 给了非空规则建议（addRules）才有意义。 */
export function permissionHasSuggestions(req: ChatPermissionRequest): boolean {
  return Array.isArray(req.permissionSuggestions) && req.permissionSuggestions.length > 0
}

/**
 * 把用户的三选一构造成 CLI 控制协议的 `decision` 对象：
 *   allow-once   → `{behavior:'allow', updatedInput:<原参数>}`
 *   always-allow → 同上 + `updatedPermissions:<CLI 规则建议>`（无建议时退化为 allow-once）
 *   deny         → `{behavior:'deny', message, interrupt:false}`（把拒绝反馈给模型、不打断本轮）
 */
export function buildPermissionDecision(
  req: ChatPermissionRequest,
  choice: PermissionChoice,
): Record<string, unknown> {
  if (choice === 'deny') {
    return { behavior: 'deny', message: 'The user declined this tool use.', interrupt: false }
  }
  const decision: Record<string, unknown> = {
    behavior: 'allow',
    updatedInput: req.input ?? {},
  }
  if (choice === 'always-allow' && permissionHasSuggestions(req)) {
    decision.updatedPermissions = req.permissionSuggestions
  }
  return decision
}
