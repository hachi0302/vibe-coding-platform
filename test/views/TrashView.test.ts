import { beforeEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import TrashView from '../../src/views/TrashView.vue'
import { vTooltip } from '../../src/tooltip'
import { setLang } from '../../src/settings'
import {
  resetTrashToolbar,
  selectMode,
  selectedTrash,
  trashSearch,
  trashSort,
} from '../../src/trashToolbar'
import type { TrashItem } from '../../src/types'

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

const factory = (trash: TrashItem[], loading = false) =>
  mount(TrashView, {
    props: { trash, loading },
    global: { directives: { tooltip: vTooltip } },
  })

describe('TrashView', () => {
  it('renders one card per trash item', () => {
    const wrapper = factory([item({ trashFile: 'a' }), item({ trashFile: 'b' })])
    expect(wrapper.findAll('.session-card')).toHaveLength(2)
  })

  it('shows the empty state when the trash is empty', () => {
    expect(factory([]).text()).toContain('Trash is empty')
  })

  it('shows the no-match state when filters exclude every item', () => {
    trashSearch.value = 'definitely-not-present'
    const wrapper = factory([item({ trashFile: 'a' })])
    expect(wrapper.findAll('.session-card')).toHaveLength(0)
    expect(wrapper.text()).toContain('No sessions match')
  })

  it('emits restore / permanent-delete from the row actions', async () => {
    const wrapper = factory([item({ trashFile: 'a' })])
    const [restore, del] = wrapper.findAll('.session-actions .icon-btn')
    await restore.trigger('click')
    await del.trigger('click')
    expect(wrapper.emitted('restore')).toHaveLength(1)
    expect(wrapper.emitted('permanent-delete')).toHaveLength(1)
  })

  describe('select mode', () => {
    it('shows a checkbox on each card and hides the row actions', () => {
      selectMode.value = true
      const wrapper = factory([item({ trashFile: 'a' })])
      expect(wrapper.find('.list-check').exists()).toBe(true)
      expect(wrapper.find('.session-actions').exists()).toBe(false)
    })

    it('toggles selection — and does not open — when a card is clicked', async () => {
      selectMode.value = true
      const wrapper = factory([item({ trashFile: 'a' })])

      await wrapper.find('.session-card').trigger('click')
      expect(selectedTrash.value.has('a')).toBe(true)

      await wrapper.find('.session-card').trigger('click')
      expect(selectedTrash.value.has('a')).toBe(false)

      expect(wrapper.emitted('open')).toBeUndefined()
    })
  })

  describe('open detail', () => {
    it('emits "open" with the item — and selects nothing — on a card click', async () => {
      const it0 = item({ trashFile: 'a' })
      const wrapper = factory([it0])
      await wrapper.find('.session-card').trigger('click')
      expect(selectedTrash.value.size).toBe(0)
      expect(wrapper.emitted('open')).toHaveLength(1)
      expect(wrapper.emitted('open')![0][0]).toEqual(it0)
    })

    it('does not open when a row action button is clicked', async () => {
      const wrapper = factory([item({ trashFile: 'a' })])
      const [restore] = wrapper.findAll('.session-actions .icon-btn')
      await restore.trigger('click')
      expect(wrapper.emitted('open')).toBeUndefined()
      expect(wrapper.emitted('restore')).toHaveLength(1)
    })
  })

  describe('header actions', () => {
    // 用 aria-label 找按钮，避免依赖 .list-head-actions 里的位置。
    // 行内顺序：normal 模式 = [sort, select, Empty Trash]，select 模式 = [select-all, restore, cancel]
    const findByLabel = (wrapper: ReturnType<typeof factory>, label: string) =>
      wrapper.findAll('.list-head-actions .icon-btn').find((b) =>
        b.attributes('aria-label')?.startsWith(label),
      )!

    it('toggles the time sort from the sort button (moved from TrashTopbar)', async () => {
      const wrapper = factory([item({ trashFile: 'a' }), item({ trashFile: 'b' })])
      expect(trashSort.value).toBe('recent')
      await findByLabel(wrapper, 'Sorted by').trigger('click')
      expect(trashSort.value).toBe('oldest')
    })

    it('hides the sort and select buttons unless there are 2+ items', () => {
      expect(
        factory([]).find('.list-head-actions .icon-btn[aria-label^="Sorted"]').exists(),
      ).toBe(false)
      expect(
        factory([item({ trashFile: 'a' })]).find(
          '.list-head-actions .icon-btn[aria-label^="Select"]',
        ).exists(),
      ).toBe(false)
      expect(
        factory([item({ trashFile: 'a' }), item({ trashFile: 'b' })]).find(
          '.list-head-actions .icon-btn[aria-label^="Sorted"]',
        ).exists(),
      ).toBe(true)
    })

    it('enters select mode from the select button', async () => {
      const wrapper = factory([item({ trashFile: 'a' }), item({ trashFile: 'b' })])
      await findByLabel(wrapper, 'Select multiple').trigger('click')
      expect(selectMode.value).toBe(true)
    })

    it('select-all toggles the whole visible set', async () => {
      selectMode.value = true
      const wrapper = factory([item({ trashFile: 'a' }), item({ trashFile: 'b' })])
      await findByLabel(wrapper, 'Select all').trigger('click')
      expect(selectedTrash.value.size).toBe(2)
      await findByLabel(wrapper, 'Deselect all').trigger('click')
      expect(selectedTrash.value.size).toBe(0)
    })

    it('emits batch-restore when restore is clicked with a selection', async () => {
      selectMode.value = true
      selectedTrash.value = new Set(['a'])
      const wrapper = factory([item({ trashFile: 'a' })])
      await findByLabel(wrapper, 'Restore selected').trigger('click')
      expect(wrapper.emitted('batch-restore')).toHaveLength(1)
    })

    it('exits select mode from the cancel button', async () => {
      selectMode.value = true
      const wrapper = factory([item({ trashFile: 'a' })])
      await findByLabel(wrapper, 'Exit selection').trigger('click')
      expect(selectMode.value).toBe(false)
    })
  })

  describe('keyword highlight', () => {
    it('highlights the matched keyword in the trash title', () => {
      trashSearch.value = 'parser'
      const wrapper = factory([
        item({ trashFile: 'a', title: 'Refactor parser', projectLabel: 'web' }),
      ])
      const hits = wrapper.findAll('.session-title .kw-hit')
      expect(hits).toHaveLength(1)
      expect(hits[0].text()).toBe('parser')
    })

    it('highlights a match in the project label', () => {
      trashSearch.value = 'viewer'
      const wrapper = factory([
        item({ trashFile: 'a', title: 'no match', projectLabel: '/Users/me/viewer' }),
      ])
      const hits = wrapper.findAll('.session-meta .kw-hit')
      expect(hits).toHaveLength(1)
      expect(hits[0].text()).toBe('viewer')
    })

    it('renders no highlight when there is no active search', () => {
      const wrapper = factory([item({ trashFile: 'a', title: 'Refactor parser' })])
      expect(wrapper.find('.kw-hit').exists()).toBe(false)
    })
  })
})
