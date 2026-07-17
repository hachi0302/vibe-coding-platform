import { describe, expect, it } from 'vitest'
import {
  highlightJsonInPlace,
  looksLikeJson,
  prettifyAndHighlightJson,
} from '../src/jsonHighlight'

describe('looksLikeJson', () => {
  it('detects a plain JSON object', () => {
    expect(looksLikeJson('{ "a": 1 }')).toBe(true)
  })

  it('detects a JSON array', () => {
    expect(looksLikeJson('[1, 2, 3]')).toBe(true)
  })

  it('detects line-numbered JSON (cat -n style Read output)', () => {
    expect(looksLikeJson('0\t{\n1\t  "name": "x"\n2\t}')).toBe(true)
  })

  it('rejects plain prose', () => {
    expect(looksLikeJson('hello world')).toBe(false)
  })

  it('rejects malformed JSON', () => {
    expect(looksLikeJson('{ broken json :')).toBe(false)
  })

  it('rejects empty', () => {
    expect(looksLikeJson('')).toBe(false)
  })

  // Read tool with `limit` 截断的 JSON 不能 parse，但仍要识别为 JSON 上色。
  // 用 `"key":` 模式作宽松门控避免假阴性。
  it('detects truncated JSON (Read with limit cuts off the closing brace)', () => {
    const truncated =
      '0\t{\n1\t  "name": "x",\n2\t  "scripts": {\n3\t    "dev": "vite"'
    expect(looksLikeJson(truncated)).toBe(true)
  })
})

describe('prettifyAndHighlightJson', () => {
  it('reformats and tokenizes tool_use args', () => {
    const html = prettifyAndHighlightJson('{"file_path":"/x","limit":25,"flag":true}')
    expect(html).toContain('<span class="json-key">"file_path"</span>')
    expect(html).toContain('<span class="json-string">"/x"</span>')
    expect(html).toContain('<span class="json-num">25</span>')
    expect(html).toContain('<span class="json-bool">true</span>')
  })

  it('falls back to token colorize on parse failure', () => {
    const html = prettifyAndHighlightJson('{ broken: "yet has a string"')
    // 仍能给字符串字面量上色（哪怕全段 parse 失败）
    expect(html).toContain('<span class="json-string">"yet has a string"</span>')
  })

  it('handles null tokens distinctly from booleans', () => {
    const html = prettifyAndHighlightJson('{"v":null}')
    expect(html).toContain('<span class="json-null">null</span>')
  })

  it('escapes HTML in string values', () => {
    const html = prettifyAndHighlightJson('{"x":"<script>"}')
    expect(html).toContain('&lt;script&gt;')
    expect(html).not.toContain('<script>')
  })
})

describe('highlightJsonInPlace', () => {
  // tool_result 等场景：保留 cat -n 行号 / 用户原始缩进，只着色 JSON token。
  it('preserves the original layout (line numbers etc.)', () => {
    const html = highlightJsonInPlace('0\t{\n1\t  "name": "x"\n2\t}')
    expect(html).toContain('<span class="json-key">"name"</span>')
    expect(html).toContain('<span class="json-string">"x"</span>')
    // 行号本身是数字（也会被染色），但分行结构保留
    expect(html).toContain('\n')
  })
})
