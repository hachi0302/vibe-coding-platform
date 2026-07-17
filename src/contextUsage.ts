// `/context` 的解析器：把 headless `claude` 在 stream-json 里吐出的 `## Context Usage`
// markdown 报告（model 行 + token 行 + 「按类目估算」表 + 若干明细表）解析成结构化数据，
// 供 ContextWindowCard 渲染成可折叠的可视化面板（参考 Claude 客户端的 Context window 卡片）。
//
// 纯函数、无副作用，便于单测。只认严格匹配的报告结构，匹配不上就返回 null（回落到普通 markdown）。

/** 类目在进度条/圆点里的配色族：蓝色=真实占用，灰色=缓冲/延迟，最浅=空闲。 */
export type ContextCategoryKind = 'used' | 'buffer' | 'free' | 'deferred'

export interface ContextCategory {
  name: string
  tokensLabel: string
  /** 数值百分比，用于条宽；解析不出（如「—」）记 0。 */
  percent: number
  percentLabel: string
  kind: ContextCategoryKind
}

export interface ContextDetailSection {
  /** 明细分区标题，如 "Memory Files" / "Skills" / "MCP tools"。 */
  name: string
  columns: string[]
  rows: string[][]
  /** 该分区在「按类目估算」表里对应类目的 token 标签（名字大小写不敏感匹配），用于头部摘要。 */
  tokensLabel: string | null
  /** 明细条目数（= rows.length），用于头部摘要里的计数。 */
  count: number
}

export interface ContextUsage {
  model: string
  usedLabel: string
  totalLabel: string
  percent: number
  categories: ContextCategory[]
  details: ContextDetailSection[]
}

interface RawTable {
  section: string
  columns: string[]
  rows: string[][]
}

/** `| a | b | c |` → `['a','b','c']`；非表行返回空数组。 */
function parseCells(line: string): string[] {
  const t = line.trim()
  if (!t.startsWith('|')) return []
  return t
    .replace(/^\|/, '')
    .replace(/\|$/, '')
    .split('|')
    .map((c) => c.trim())
}

/** markdown 表的分隔行：每格都是 `---` / `:--:` 之类。 */
function isSeparatorRow(cells: string[]): boolean {
  return cells.length > 0 && cells.every((c) => /^:?-{2,}:?$/.test(c.replace(/\s/g, '')))
}

/** 按 `### 标题` 归组，扫出文中所有 markdown 表。 */
function parseTables(lines: string[]): RawTable[] {
  const tables: RawTable[] = []
  let section = ''
  let cur: RawTable | null = null
  for (const line of lines) {
    const h = /^#{2,3}\s+(.+?)\s*$/.exec(line.trim())
    if (h) {
      section = h[1].trim()
      cur = null
      continue
    }
    const cells = parseCells(line)
    if (cells.length === 0) {
      cur = null // 空行 / 非表行 → 当前表结束
      continue
    }
    if (cur === null) {
      cur = { section, columns: cells, rows: [] }
      tables.push(cur)
    } else if (!isSeparatorRow(cells)) {
      cur.rows.push(cells)
    }
  }
  return tables
}

function classifyCategory(name: string): ContextCategoryKind {
  const n = name.toLowerCase()
  if (n.includes('free space')) return 'free'
  if (n.includes('autocompact') || n.includes('buffer')) return 'buffer'
  if (n.includes('(deferred)')) return 'deferred'
  return 'used'
}

const CATEGORY_HEADINGS = /^(estimated usage by category|usage by category|context usage by category)$/i

export function parseContextUsage(text: string | undefined | null): ContextUsage | null {
  if (!text) return null
  const t = text.trim()
  if (!/^#{1,3}\s+Context Usage/m.test(t)) return null

  const model = /\*\*Model:\*\*\s*(.+?)\s*$/m.exec(t)?.[1]?.trim() ?? ''
  const tok = /\*\*Tokens:\*\*\s*([\d.,]+\s*[kKmMgG]?)\s*\/\s*([\d.,]+\s*[kKmMgG]?)\s*\((\d+(?:\.\d+)?)\s*%\)/.exec(t)
  if (!tok) return null
  const usedLabel = tok[1].trim()
  const totalLabel = tok[2].trim()
  const percent = parseFloat(tok[3])

  const tables = parseTables(t.split('\n'))
  const categoryTable = tables.find(
    (tb) => CATEGORY_HEADINGS.test(tb.section) || /^category$/i.test(tb.columns[0] ?? ''),
  )
  if (!categoryTable) return null

  const categories: ContextCategory[] = categoryTable.rows
    .filter((r) => r.length >= 2 && r[0])
    .map((r) => {
      const name = r[0]
      const tokensLabel = r[1] ?? ''
      const percentLabel = (r[2] ?? '').trim() || '—'
      const pctNum = parseFloat(percentLabel.replace('%', ''))
      return {
        name,
        tokensLabel,
        percent: Number.isFinite(pctNum) ? pctNum : 0,
        percentLabel,
        kind: classifyCategory(name),
      }
    })

  // 明细分区 ↔ 类目的回链：先按完整名（小写）精确匹配，再退回「去掉 (deferred) 后缀」的归一名，
  // 这样 "MCP tools" 明细能对上 "MCP tools (deferred)" 类目的聚合 token 数。精确优先以避免归一后撞车。
  const norm = (s: string) => s.toLowerCase().replace(/\(deferred\)/g, '').replace(/\s+/g, ' ').trim()
  const tokensByCategory = new Map(categories.map((c) => [c.name.toLowerCase(), c.tokensLabel]))
  const tokensByNorm = new Map<string, string>()
  for (const c of categories) {
    const k = norm(c.name)
    if (!tokensByNorm.has(k)) tokensByNorm.set(k, c.tokensLabel)
  }
  const details: ContextDetailSection[] = tables
    .filter((tb) => tb !== categoryTable && tb.section && tb.rows.length > 0)
    .map((tb) => ({
      name: tb.section,
      columns: tb.columns,
      rows: tb.rows,
      tokensLabel: tokensByCategory.get(tb.section.toLowerCase()) ?? tokensByNorm.get(norm(tb.section)) ?? null,
      count: tb.rows.length,
    }))

  return { model, usedLabel, totalLabel, percent, categories, details }
}
