import { describe, expect, it } from 'vitest'
import { parseContextUsage } from '../src/contextUsage'

// 真实样本：headless claude 在 stream-json 里吐出的 `/context` 报告（截断了 Skills 明细）。
const REAL = `## Context Usage

**Model:** claude-opus-4-8[1m]
**Tokens:** 26.8k / 400k (7%)

### Estimated usage by category

| Category | Tokens | Percentage |
|----------|--------|------------|
| System prompt | 2.4k | 0.6% |
| System tools | 9.7k | 2.4% |
| System tools (deferred) | 14.3k | 3.6% |
| Memory files | 741 | 0.2% |
| Skills | 4.4k | 1.1% |
| Messages | 10k | 2.5% |
| Free space | 339.9k | 85.0% |
| Autocompact buffer | 33k | 8.3% |

### Memory Files

| Type | Path | Tokens |
|------|------|--------|
| User | /Users/wuchao/.claude/CLAUDE.md | 372 |
| User | /Users/wuchao/.claude/RTK.md | 369 |

### Skills

| Skill | Source | Tokens |
|-------|--------|--------|
| animejs | User | ~100 |
| gsap | User | ~100 |`

describe('parseContextUsage', () => {
  it('returns null for non-context text', () => {
    expect(parseContextUsage('## Hello\n\nsome text')).toBeNull()
    expect(parseContextUsage('')).toBeNull()
    expect(parseContextUsage(undefined)).toBeNull()
  })

  it('parses the model and token header', () => {
    const u = parseContextUsage(REAL)!
    expect(u).not.toBeNull()
    expect(u.model).toBe('claude-opus-4-8[1m]')
    expect(u.usedLabel).toBe('26.8k')
    expect(u.totalLabel).toBe('400k')
    expect(u.percent).toBe(7)
  })

  it('parses every category row with tokens + percentage', () => {
    const u = parseContextUsage(REAL)!
    expect(u.categories).toHaveLength(8)
    const sysPrompt = u.categories.find((c) => c.name === 'System prompt')!
    expect(sysPrompt.tokensLabel).toBe('2.4k')
    expect(sysPrompt.percent).toBe(0.6)
    expect(sysPrompt.percentLabel).toBe('0.6%')
  })

  it('classifies category kinds (used / buffer / free / deferred)', () => {
    const u = parseContextUsage(REAL)!
    const kind = (n: string) => u.categories.find((c) => c.name === n)!.kind
    expect(kind('System prompt')).toBe('used')
    expect(kind('Messages')).toBe('used')
    expect(kind('System tools (deferred)')).toBe('deferred')
    expect(kind('Autocompact buffer')).toBe('buffer')
    expect(kind('Free space')).toBe('free')
  })

  it('parses detail sections with columns, rows and count', () => {
    const u = parseContextUsage(REAL)!
    expect(u.details.map((d) => d.name)).toEqual(['Memory Files', 'Skills'])
    const mem = u.details[0]
    expect(mem.columns).toEqual(['Type', 'Path', 'Tokens'])
    expect(mem.rows).toHaveLength(2)
    expect(mem.rows[0]).toEqual(['User', '/Users/wuchao/.claude/CLAUDE.md', '372'])
    expect(mem.count).toBe(2)
  })

  it('back-links each detail section to its category token total (case-insensitive)', () => {
    const u = parseContextUsage(REAL)!
    // "Memory Files" 分区 ↔ "Memory files" 类目 (741)；"Skills" ↔ "Skills" (4.4k)
    expect(u.details.find((d) => d.name === 'Memory Files')!.tokensLabel).toBe('741')
    expect(u.details.find((d) => d.name === 'Skills')!.tokensLabel).toBe('4.4k')
  })

  it('excludes the category table from the detail sections', () => {
    const u = parseContextUsage(REAL)!
    expect(u.details.some((d) => d.name === 'Estimated usage by category')).toBe(false)
  })

  it('handles a dash ("—") percentage as 0 without crashing', () => {
    const txt = `## Context Usage

**Model:** m
**Tokens:** 1k / 10k (10%)

### Estimated usage by category

| Category | Tokens | Percentage |
|----------|--------|------------|
| MCP tools (deferred) | 45.8k | — |
| Free space | 9k | 90.0% |`
    const u = parseContextUsage(txt)!
    const def = u.categories.find((c) => c.name === 'MCP tools (deferred)')!
    expect(def.percent).toBe(0)
    expect(def.percentLabel).toBe('—')
    expect(def.kind).toBe('deferred')
  })

  it('back-links a detail section to a "(deferred)" category (suffix-tolerant)', () => {
    const txt = `## Context Usage

**Model:** m
**Tokens:** 80k / 400k (20%)

### Estimated usage by category

| Category | Tokens | Percentage |
|----------|--------|------------|
| MCP tools (deferred) | 50k | 12.5% |
| Free space | 320k | 80.0% |

### MCP tools

| Tool | Source | Tokens |
|------|--------|--------|
| read_file | server-a | ~600 |
| write_file | server-a | ~600 |`
    const u = parseContextUsage(txt)!
    const mcp = u.details.find((d) => d.name === 'MCP tools')!
    expect(mcp.tokensLabel).toBe('50k')
    expect(mcp.count).toBe(2)
  })

  it('returns null when the token header is missing', () => {
    const txt = `## Context Usage

**Model:** m

### Estimated usage by category

| Category | Tokens | Percentage |
|----------|--------|------------|
| Messages | 10k | 2.5% |`
    expect(parseContextUsage(txt)).toBeNull()
  })
})
