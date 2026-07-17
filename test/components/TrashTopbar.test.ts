import { beforeEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import TrashTopbar from '../../src/components/topbar/TrashTopbar.vue'
import { vTooltip } from '../../src/tooltip'
import { setLang } from '../../src/settings'
import {
  resetTrashToolbar,
  trashProject,
  trashSearch,
} from '../../src/trashToolbar'
import type { TrashItem } from '../../src/types'

// Only the project filter + search box live in the topbar; sort, select-mode
// entry, and batch-restore moved into TrashView's body header (see
// `list-head-actions` in TrashView.test.ts) so the topbar no longer competes
// with the body header's "Empty Trash" row.

beforeEach(() => {
  setLang('en')
  resetTrashToolbar()
})

const item = (over: Partial<TrashItem> & { trashFile: string }): TrashItem => ({
  agent: 'claude',
  projectLabel: 'proj',
  originalPath: '/orig',
  trashPath: `/trash/${over.trashFile}`,
  deletedAt: 0,
  title: 'A session',
  size: 100,
  ...over,
})

const factory = (
  items: TrashItem[] = [item({ trashFile: 'a' }), item({ trashFile: 'b' })],
) =>
  mount(TrashTopbar, {
    props: { items },
    global: { directives: { tooltip: vTooltip } },
  })

describe('TrashTopbar', () => {
  it('binds the search box to the shared search ref (debounced)', async () => {
    const wrapper = factory()
    await wrapper.find('.ct-search-input').setValue('hello')
    expect(trashSearch.value).toBe('') // 还没过防抖
    await new Promise((r) => setTimeout(r, 280))
    expect(trashSearch.value).toBe('hello')
  })

  it('lists distinct projects in the filter dropdown and applies a pick', async () => {
    const wrapper = factory([
      item({ trashFile: 'a', projectLabel: 'web' }),
      item({ trashFile: 'b', projectLabel: 'api' }),
    ])
    await wrapper.find('.ct-scope-btn').trigger('click')
    const items = wrapper.findAll('.ct-scope-item')
    // 'All projects' + 2 distinct labels
    expect(items).toHaveLength(3)

    await items[1].trigger('click') // 'api' (sorted first)
    expect(trashProject.value).toBe('api')
  })

  it('shows only the project basename in the dropdown, not the full path', async () => {
    const wrapper = factory([
      item({ trashFile: 'a', projectLabel: '/Users/me/apps/my-project' }),
    ])
    await wrapper.find('.ct-scope-btn').trigger('click')
    const labels = wrapper.findAll('.ct-scope-item').map((b) => b.text())
    expect(labels).toContain('my-project')
    expect(labels.some((l) => l.includes('/Users'))).toBe(false)
  })

  it('focuses the search box on the ⌘F / Ctrl+F shortcut', () => {
    const wrapper = mount(TrashTopbar, {
      props: { items: [item({ trashFile: 'a' })] },
      global: { directives: { tooltip: vTooltip } },
      attachTo: document.body,
    })
    const isMac = /Mac/i.test(navigator.platform)
    window.dispatchEvent(
      new KeyboardEvent('keydown', { key: 'f', metaKey: isMac, ctrlKey: !isMac }),
    )
    expect(document.activeElement).toBe(wrapper.find('.ct-search-input').element)
    wrapper.unmount()
  })

  it('does not render the action bar — those controls live in TrashView now', () => {
    const wrapper = factory()
    expect(wrapper.find('.ct-actions').exists()).toBe(false)
  })
})
