import { describe, expect, it } from 'vitest'
import { highlightDiff, looksLikeDiff } from '../src/diffHighlight'

describe('looksLikeDiff', () => {
  it('detects a hunk header', () => {
    expect(looksLikeDiff('@@ -1,3 +1,4 @@\n context\n+added\n')).toBe(true)
  })

  it('detects a multi-line patch with file header', () => {
    const patch = [
      'diff --git a/foo.ts b/foo.ts',
      'index abc..def 100644',
      '--- a/foo.ts',
      '+++ b/foo.ts',
      '@@ -10,2 +10,3 @@',
      ' const x = 1',
      '+const y = 2',
      ' const z = 3',
    ].join('\n')
    expect(looksLikeDiff(patch)).toBe(true)
  })

  it('detects rename-only diff without hunks', () => {
    expect(looksLikeDiff('diff --git a/a b/b\nsimilarity index 100%')).toBe(true)
  })

  it('rejects plain prose with a leading minus', () => {
    expect(looksLikeDiff('- item one\n- item two\n- item three')).toBe(false)
  })

  it('rejects empty', () => {
    expect(looksLikeDiff('')).toBe(false)
  })

  it('rejects JSON', () => {
    expect(looksLikeDiff('{ "a": 1, "b": 2 }')).toBe(false)
  })
})

describe('highlightDiff', () => {
  it('classifies hunk header / add / del / context', () => {
    const html = highlightDiff('@@ -1,2 +1,3 @@\n+added\n-removed\n unchanged')
    expect(html).toContain('<span class="diff-hunk">@@ -1,2 +1,3 @@</span>')
    expect(html).toContain('<span class="diff-add">+added</span>')
    expect(html).toContain('<span class="diff-del">-removed</span>')
    expect(html).toContain('<span class="diff-ctx"> unchanged</span>')
  })

  it('treats --- and +++ as metadata, not add/del', () => {
    const html = highlightDiff('--- a/x\n+++ b/x')
    expect(html).toContain('<span class="diff-meta">--- a/x</span>')
    expect(html).toContain('<span class="diff-meta">+++ b/x</span>')
    expect(html).not.toContain('diff-del">--- ')
    expect(html).not.toContain('diff-add">+++ ')
  })

  it('classifies diff --git as file header', () => {
    const html = highlightDiff('diff --git a/foo b/foo')
    expect(html).toContain('<span class="diff-file">diff --git a/foo b/foo</span>')
  })

  it('escapes HTML in diff lines', () => {
    const html = highlightDiff('+<script>alert(1)</script>')
    expect(html).toContain('&lt;script&gt;')
    expect(html).not.toContain('<script>alert')
  })
})
