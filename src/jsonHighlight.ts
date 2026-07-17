// 工具调用参数 / 工具结果的 JSON 语法高亮。轻量手写 tokenizer，没拉 highlight.js。
//
// 两种 mode：
//   - prettifyJson(text)：tool_use 的 args 永远是 JSON。能 parse 就 pretty-print（2 空格）
//     再上色；parse 不了就 colorize 原文（用户写错的 args 也能看个 token 大概）。
//   - highlightJsonInPlace(text)：tool_result 这种"已带 cat -n 行号"的 JSON 文件输出。
//     不重排版，逐字符 escape + 正则 tokenize，行号 / 缩进保留原样。

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}

// JSON token 正则。alternation 顺序很重要：先匹 string（可能带紧跟 `:` 判定为 key），
// 再 keyword（true/false/null），最后 number。字符串内 `\\.` 处理转义引号。
const TOKEN_RE =
  /("(?:\\.|[^"\\])*")(\s*:)?|\b(true|false|null)\b|(-?\d+(?:\.\d+)?(?:[eE][+-]?\d+)?)/g

function colorize(escaped: string): string {
  return escaped.replace(TOKEN_RE, (_m, str, colon, kw, num) => {
    if (str !== undefined) {
      // key 跟普通 string 同色但不同 class —— UI 可以再分两色（key 偏 brand，string 偏绿）。
      const cls = colon ? 'json-key' : 'json-string'
      return `<span class="${cls}">${str}</span>${colon ?? ''}`
    }
    if (kw !== undefined) {
      const cls = kw === 'null' ? 'json-null' : 'json-bool'
      return `<span class="${cls}">${kw}</span>`
    }
    return `<span class="json-num">${num}</span>`
  })
}

/** 文本是否长得像 JSON —— 先剥掉每行可能的 `<n>\s+` cat-n 行号前缀再 trim，
 *  首字符必须是 `{` / `[`，再走"宽松检测"。给 tool_result / 任意文本块用做"该不该上色"
 *  的门控。tool_use 的 args 不走这里 —— args 永远当 JSON 试一次（parse 不了原样上色）。
 *
 *  注意不能强 `JSON.parse`：Claude 的 Read 工具带 `limit` 参数时返回的是 *截断* 的
 *  JSON（只前 N 行），parse 一定失败 —— 但用户照样希望看到上色。所以两条触发线：
 *    1. 至少出现一对 `"key":` 模式（Bash 输出 / 自然语言里极少见，假阳性低）
 *    2. 否则才退到严格 `JSON.parse` 兜底（短对象 / 短数组也能上色） */
export function looksLikeJson(text: string): boolean {
  if (!text) return false
  const stripped = text.replace(/^\s*\d+\s+/gm, '')
  const t = stripped.trim()
  if (!t || (t[0] !== '{' && t[0] !== '[')) return false
  // `"…":` 这个组合普通日志 / 自然语言 / shell 输出几乎不会出现 —— 用作宽松门控，
  // 截断的 JSON 也能被识别。
  if (/"[^"\n\\]+"\s*:/.test(t)) return true
  try {
    JSON.parse(t)
    return true
  } catch {
    return false
  }
}

/** Tool args 专用：能 parse 就重新 pretty-print（2-space），再上色。
 *  parse 失败 → 原文 escapeHtml + tokenize（用户可能写错也想看个大概）。
 *  返回值始终是安全 HTML，可直接 v-html。 */
export function prettifyAndHighlightJson(raw: string): string {
  const text = raw ?? ''
  try {
    const parsed = JSON.parse(text)
    const pretty = JSON.stringify(parsed, null, 2)
    return colorize(escapeHtml(pretty))
  } catch {
    return colorize(escapeHtml(text))
  }
}

/** Tool result 等"原文保留"场景：不重排版（避免破坏 cat -n 行号 / 用户在意的缩进），
 *  整段 escapeHtml + 正则 tokenize 即可。行号本身是 number 也会被染色 —— 跟正文颜色
 *  区分度低，可接受。 */
export function highlightJsonInPlace(raw: string): string {
  return colorize(escapeHtml(raw ?? ''))
}
