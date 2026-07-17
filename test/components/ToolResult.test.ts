import { beforeEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import ToolResult from '../../src/components/ToolResult.vue'
import type { Block } from '../../src/types'
import { setLang } from '../../src/settings'

beforeEach(() => setLang('en'))

function blk(over: Partial<Block> & { kind: Block['kind'] }): Block {
  return { isError: false, ...over }
}

describe('ToolResult', () => {
  it('labels a plain result and stays collapsed', () => {
    const wrapper = mount(ToolResult, {
      props: { block: blk({ kind: 'tool_result', text: 'output' }) },
    })
    expect(wrapper.find('.label').text()).toBe('Tool result')
    expect(wrapper.find('details').attributes('open')).toBeUndefined()
    expect(wrapper.find('pre').text()).toBe('output')
  })

  it('marks an error result', () => {
    const wrapper = mount(ToolResult, {
      props: { block: blk({ kind: 'tool_result', text: 'bad', isError: true }) },
    })
    expect(wrapper.find('.label').text()).toBe('Tool result · error')
    expect(wrapper.find('.label').classes()).toContain('error')
  })

  it('labels a diff result with the file basename and opens it', () => {
    const wrapper = mount(ToolResult, {
      props: {
        block: blk({
          kind: 'tool_result',
          filePath: '/deep/nested/file.ts',
          diff: [
            {
              oldStart: 1,
              newStart: 1,
              lines: [
                { kind: 'add', oldNo: null, newNo: 1, text: 'x' },
                { kind: 'add', oldNo: null, newNo: 2, text: 'y' },
                { kind: 'del', oldNo: 1, newNo: null, text: 'z' },
              ],
            },
          ],
        }),
      },
    })
    expect(wrapper.find('.label').text()).toBe('File change · file.ts')
    expect(wrapper.find('details').attributes('open')).toBeDefined()
    expect(wrapper.find('.diff-stat').text()).toBe('+2 −1')
    expect(wrapper.findComponent({ name: 'DiffBlock' }).exists()).toBe(true)
  })

  it('omits the diff-stat element when there is no diff', () => {
    const wrapper = mount(ToolResult, {
      props: { block: blk({ kind: 'tool_result', text: 'plain' }) },
    })
    expect(wrapper.find('.diff-stat').exists()).toBe(false)
  })

  it('adds the in-user modifier class when inUser is set', () => {
    const wrapper = mount(ToolResult, {
      props: { block: blk({ kind: 'tool_result', text: 'o' }), inUser: true },
    })
    expect(wrapper.find('details').classes()).toContain('in-user')
  })
})
