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
      '*** Update File: /Users/wuchao/apps/claude-session-viewer/test/format.test.ts',
      '@@',
      ' it("a", () => {})',
      '+it("b", () => {})',
      '*** End Patch',
    ].join('\n')

    const html = renderCodexApplyPatchHtml(
      input,
      '/Users/wuchao/apps/claude-session-viewer',
    )

    expect(html).toContain('test/format.test.ts')
    expect(html).toContain('codex-patch-file')
    expect(html).toContain('codex-patch-line add')
    expect(html).not.toContain('*** Begin Patch')
    expect(html).not.toContain('*** Update File:')
  })
})
