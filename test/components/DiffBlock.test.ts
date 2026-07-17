import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import DiffBlock from '../../src/components/DiffBlock.vue'
import type { DiffHunk } from '../../src/types'

const hunk = (lines: DiffHunk['lines'], oldStart = 1, newStart = 1): DiffHunk => ({
  oldStart,
  newStart,
  lines,
})

describe('DiffBlock', () => {
  it('renders one row per diff line', () => {
    const wrapper = mount(DiffBlock, {
      props: {
        hunks: [
          hunk([
            { kind: 'ctx', oldNo: 1, newNo: 1, text: 'context' },
            { kind: 'add', oldNo: null, newNo: 2, text: 'inserted' },
            { kind: 'del', oldNo: 2, newNo: null, text: 'deleted' },
          ]),
        ],
      },
    })
    expect(wrapper.findAll('.diff-line')).toHaveLength(3)
  })

  it('applies the line-kind class to each row', () => {
    const wrapper = mount(DiffBlock, {
      props: {
        hunks: [
          hunk([
            { kind: 'add', oldNo: null, newNo: 1, text: 'a' },
            { kind: 'del', oldNo: 1, newNo: null, text: 'b' },
          ]),
        ],
      },
    })
    const rows = wrapper.findAll('.diff-line')
    expect(rows[0].classes()).toContain('add')
    expect(rows[1].classes()).toContain('del')
  })

  it('uses +/-/space signs by line kind', () => {
    const wrapper = mount(DiffBlock, {
      props: {
        hunks: [
          hunk([
            { kind: 'ctx', oldNo: 1, newNo: 1, text: 'c' },
            { kind: 'add', oldNo: null, newNo: 2, text: 'a' },
            { kind: 'del', oldNo: 2, newNo: null, text: 'd' },
          ]),
        ],
      },
    })
    const signs = wrapper.findAll('.diff-sign').map((s) => s.text())
    expect(signs).toEqual(['', '+', '-'])
  })

  it('shows the new line number for additions and the old number otherwise', () => {
    const wrapper = mount(DiffBlock, {
      props: {
        hunks: [
          hunk([
            { kind: 'add', oldNo: null, newNo: 9, text: 'a' },
            { kind: 'ctx', oldNo: 4, newNo: 9, text: 'c' },
          ]),
        ],
      },
    })
    const nums = wrapper.findAll('.diff-no').map((n) => n.text())
    expect(nums).toEqual(['9', '4'])
  })

  it('inserts a separator between hunks but not before the first', () => {
    const line: DiffHunk['lines'] = [{ kind: 'ctx', oldNo: 1, newNo: 1, text: 'x' }]
    const one = mount(DiffBlock, { props: { hunks: [hunk(line)] } })
    expect(one.findAll('.diff-sep')).toHaveLength(0)

    const two = mount(DiffBlock, { props: { hunks: [hunk(line), hunk(line)] } })
    expect(two.findAll('.diff-sep')).toHaveLength(1)
  })

  it('renders an empty container for no hunks', () => {
    const wrapper = mount(DiffBlock, { props: { hunks: [] } })
    expect(wrapper.find('.diff').exists()).toBe(true)
    expect(wrapper.findAll('.diff-line')).toHaveLength(0)
  })
})
