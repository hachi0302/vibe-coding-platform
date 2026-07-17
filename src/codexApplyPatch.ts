type PatchOp = 'update' | 'add' | 'delete'
type PatchLineKind = 'ctx' | 'add' | 'del' | 'hunk'

export interface CodexPatchLine {
  kind: PatchLineKind
  text: string
}

export interface CodexPatchSection {
  op: PatchOp
  path: string
  movedTo?: string
  lines: CodexPatchLine[]
  addCount: number
  delCount: number
}

const FILE_HEADER_RE = /^\*\*\* (Update|Add|Delete) File: (.+)$/
const MOVE_TO_RE = /^\*\*\* Move to: (.+)$/

function escapeHtml(s: string): string {
  return s
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
    .replace(/"/g, '&quot;')
}

function opFromHeader(op: string): PatchOp {
  if (op === 'Add') return 'add'
  if (op === 'Delete') return 'delete'
  return 'update'
}

function displayPath(path: string, cwd?: string): string {
  const normalizedPath = path.replace(/\\/g, '/')
  const normalizedCwd = cwd?.replace(/\\/g, '/').replace(/\/$/, '')
  if (normalizedCwd && normalizedPath.startsWith(`${normalizedCwd}/`)) {
    return normalizedPath.slice(normalizedCwd.length + 1)
  }
  return normalizedPath
}

function opLabel(op: PatchOp): string {
  if (op === 'add') return 'Added'
  if (op === 'delete') return 'Deleted'
  return 'Updated'
}

function renderPatchLine(line: CodexPatchLine): string {
  if (line.kind === 'hunk') {
    if (line.text.trim() === '@@') return ''
    return `<div class="codex-patch-line hunk"><span class="codex-patch-text">${escapeHtml(line.text)}</span></div>`
  }
  const sign = line.kind === 'add' ? '+' : line.kind === 'del' ? '-' : ''
  const text = line.text.length ? escapeHtml(line.text) : '&nbsp;'
  return `<div class="codex-patch-line ${line.kind}"><span class="codex-patch-sign">${sign}</span><span class="codex-patch-text">${text}</span></div>`
}

export function parseCodexApplyPatch(input: string): CodexPatchSection[] {
  const lines = (input ?? '').split('\n')
  const sections: CodexPatchSection[] = []
  let current: CodexPatchSection | null = null

  const flush = () => {
    if (!current) return
    sections.push(current)
    current = null
  }

  for (const line of lines) {
    if (!line) continue
    if (line === '*** Begin Patch') continue
    if (line === '*** End Patch') break
    if (line === '*** End of File') continue

    const fileHeader = FILE_HEADER_RE.exec(line)
    if (fileHeader) {
      flush()
      current = {
        op: opFromHeader(fileHeader[1]),
        path: fileHeader[2],
        lines: [],
        addCount: 0,
        delCount: 0,
      }
      continue
    }

    if (!current) continue

    const moveTo = MOVE_TO_RE.exec(line)
    if (moveTo) {
      current.movedTo = moveTo[1]
      continue
    }

    if (line.startsWith('@@')) {
      current.lines.push({ kind: 'hunk', text: line })
      continue
    }
    if (line.startsWith('+')) {
      current.lines.push({ kind: 'add', text: line.slice(1) })
      current.addCount += 1
      continue
    }
    if (line.startsWith('-')) {
      current.lines.push({ kind: 'del', text: line.slice(1) })
      current.delCount += 1
      continue
    }
    if (line.startsWith(' ')) {
      current.lines.push({ kind: 'ctx', text: line.slice(1) })
      continue
    }
  }

  flush()
  return sections
}

export function renderCodexApplyPatchHtml(input: string, cwd?: string): string | null {
  const sections = parseCodexApplyPatch(input)
  if (!sections.length) return null

  return sections
    .map((section) => {
      const target = section.movedTo ?? section.path
      const visiblePath = displayPath(target, cwd)
      const stat = `+${section.addCount} -${section.delCount}`
      const body = section.lines.map(renderPatchLine).filter(Boolean).join('')
      return [
        '<div class="codex-patch-file">',
        '<div class="codex-patch-head">',
        `<a href="${escapeHtml(target)}" class="local-file-link codex-patch-path" data-local-file-link="1" data-local-target="${escapeHtml(target)}" title="${escapeHtml(target)}">${escapeHtml(visiblePath)}</a>`,
        `<span class="codex-patch-op">${opLabel(section.op)}</span>`,
        `<span class="codex-patch-stat"><span class="add">+${section.addCount}</span><span class="del">-${section.delCount}</span></span>`,
        '</div>',
        body
          ? `<div class="codex-patch-diff">${body}</div>`
          : `<div class="codex-patch-empty">${escapeHtml(stat)}</div>`,
        '</div>',
      ].join('')
    })
    .join('')
}
