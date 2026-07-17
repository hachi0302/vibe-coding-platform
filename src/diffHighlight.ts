// 统一 diff（unified diff）文本高亮 —— Bash 跑 `git diff` / 工具吐 patch 等，
// 拿到的是文本形态 diff（不是带 hunks 的 structured patch）。结构 patch 走
// 已有的 DiffBlock；这里只做"明显是 diff 文本"的 case：行首前缀染色。
//
// 颜色复用 DiffBlock 的 `--diff-add` / `--diff-del`（已在 style.css 定义）。
// 行类型：
//   diff-file    `diff --git a/x b/x`
//   diff-meta    `index ...` / `--- a/...` / `+++ b/...`
//   diff-hunk    `@@ -1,2 +3,4 @@ ...`
//   diff-add     `+...`
//   diff-del     `-...`
//   diff-ctx     其它

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

const HUNK_RE = /^@@\s+-\d+(?:,\d+)?\s+\+\d+(?:,\d+)?\s+@@/

/** 文本是否长得像 unified diff —— 看是否含 `@@ -m,n +p,q @@` hunk 头（必要条件，
 *  普通日志 / 自然语言 / JSON 都不会有这个组合）。
 *  也接受 `diff --git ` 开头但缺 hunk 的情况（差异为空时 git 也会出 file header）。 */
export function looksLikeDiff(text: string): boolean {
  if (!text) return false
  const trimmed = text.trimStart()
  // 任一行匹 hunk 头 → 一定是 diff
  if (/^@@\s+-\d+(?:,\d+)?\s+\+\d+(?:,\d+)?\s+@@/m.test(trimmed)) return true
  // 没 hunk 但 `diff --git ` 开头也算（空 diff / rename-only 等）
  if (trimmed.startsWith('diff --git ')) return true
  return false
}

/** 把 unified diff 文本染色 —— 返回安全 HTML 字符串，可直接 v-html。
 *  不重排版（保留原换行、空白、可能的 cat-n 行号前缀）。 */
export function highlightDiff(raw: string): string {
  const text = raw ?? ''
  if (!text) return ''
  const lines = text.split('\n')
  const out: string[] = []
  for (const line of lines) {
    const cls = classifyDiffLine(line)
    out.push(`<span class="${cls}">${escapeHtml(line)}</span>`)
  }
  // 行间用 \n 拼回 —— <pre> 默认 white-space:pre，能正确换行。
  return out.join('\n')
}

function classifyDiffLine(line: string): string {
  if (HUNK_RE.test(line)) return 'diff-hunk'
  // file headers / metadata —— 注意：`---` 与 `-` 都是 minus 开头，要先判 `---`/`+++`
  if (line.startsWith('--- ') || line.startsWith('+++ ')) return 'diff-meta'
  if (line.startsWith('diff --git ')) return 'diff-file'
  if (line.startsWith('index ')) return 'diff-meta'
  if (line.startsWith('+')) return 'diff-add'
  if (line.startsWith('-')) return 'diff-del'
  return 'diff-ctx'
}
