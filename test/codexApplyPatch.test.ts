import { describe, expect, it } from 'vitest'
import { parseCodexApplyPatch, renderCodexApplyPatchHtml } from '../src/codexApplyPatch'

describe('parseCodexApplyPatch', () => {
  it('splits apply_patch content into per-file sections', () => {
    const input = [
      '*** Begin Patch',
      '*** Update File: /repo/src/a.ts',
      '@@',
      ' line 1',
      '+line 2',
      '*** Delete File: /repo/src/b.ts',
      '*** End Patch',
    ].join('\n')

    const sections = parseCodexApplyPatch(input)
    expect(sections).toHaveLength(2)
    expect(sections[0]).toMatchObject({
      op: 'update',
      path: '/repo/src/a.ts',
      addCount: 1,
      delCount: 0,
    })
    expect(sections[1]).toMatchObject({
      op: 'delete',
      path: '/repo/src/b.ts',
    })
  })
})

describe('renderCodexApplyPatchHtml', () => {
  it('renders file header plus diff body without patch wrapper lines', () => {
    const input = [
      '*** Begin Patch',
      '*** Update File: /Users/example-user/apps/vibe-coding-platform/test/format.test.ts',
      '@@',
      ' it("a", () => {})',
      '+it("b", () => {})',
      '*** End Patch',
    ].join('\n')

    const html = renderCodexApplyPatchHtml(
      input,
      '/Users/example-user/apps/vibe-coding-platform',
    )

    expect(html).toContain('test/format.test.ts')
    expect(html).toContain('codex-patch-file')
    expect(html).toContain('codex-patch-line add')
    expect(html).not.toContain('*** Begin Patch')
    expect(html).not.toContain('*** Update File:')
  })

  it('renders line numbers from unified-diff hunk headers', () => {
    const input = [
      '*** Begin Patch',
      '*** Update File: /repo/src/a.ts',
      '@@ -10,2 +10,3 @@',
      ' context',
      '-before',
      '+after',
      '+another',
      '*** End Patch',
    ].join('\n')

    expect(parseCodexApplyPatch(input)[0].lines).toEqual([
      { kind: 'hunk', text: '@@ -10,2 +10,3 @@' },
      { kind: 'ctx', text: 'context', oldNo: 10, newNo: 10 },
      { kind: 'del', text: 'before', oldNo: 11 },
      { kind: 'add', text: 'after', newNo: 11 },
      { kind: 'add', text: 'another', newNo: 12 },
    ])
    expect(renderCodexApplyPatchHtml(input)).toContain('class="codex-patch-no">11</span>')
  })

  it('numbers raw added files and omits an empty replace-delete section', () => {
    const input = [
      '*** Begin Patch',
      '*** Delete File: /repo/src/a.ts',
      '*** Add File: /repo/src/a.ts',
      '+first',
      '+second',
      '*** End Patch',
    ].join('\n')

    const sections = parseCodexApplyPatch(input)
    expect(sections).toHaveLength(1)
    expect(sections[0]).toMatchObject({ op: 'add', path: '/repo/src/a.ts' })
    expect(sections[0].lines).toEqual([
      { kind: 'add', text: 'first', newNo: 1 },
      { kind: 'add', text: 'second', newNo: 2 },
    ])
  })
})
