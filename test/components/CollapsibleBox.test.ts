import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import CollapsibleBox from '../../src/components/CollapsibleBox.vue'
import { setLang } from '../../src/settings'

beforeEach(() => setLang('en'))

const slot = '<div class="payload">content</div>'

describe('CollapsibleBox', () => {
  it('renders the slot directly when disabled', () => {
    const wrapper = mount(CollapsibleBox, {
      props: { enabled: false },
      slots: { default: slot },
    })
    expect(wrapper.find('.payload').exists()).toBe(true)
    expect(wrapper.find('.collapsible-box').exists()).toBe(false)
  })

  it('wraps the slot in a collapsible box when enabled', () => {
    const wrapper = mount(CollapsibleBox, { slots: { default: slot } })
    expect(wrapper.find('.collapsible-box').exists()).toBe(true)
    expect(wrapper.find('.collapsible-inner .payload').exists()).toBe(true)
  })

  it('shows no toggle when the content fits within maxHeight', async () => {
    const wrapper = mount(CollapsibleBox, { slots: { default: slot } })
    await flushPromises()
    expect(wrapper.find('.collapsible-toggle').exists()).toBe(false)
  })

  it('falls back to rendering the slot directly when disabled at runtime', async () => {
    const wrapper = mount(CollapsibleBox, {
      props: { enabled: true },
      slots: { default: slot },
    })
    expect(wrapper.find('.collapsible-box').exists()).toBe(true)
    await wrapper.setProps({ enabled: false })
    expect(wrapper.find('.collapsible-box').exists()).toBe(false)
    expect(wrapper.find('.payload').exists()).toBe(true)
  })

  it('unmounts cleanly (disconnecting its ResizeObserver)', () => {
    const wrapper = mount(CollapsibleBox, { slots: { default: slot } })
    expect(() => wrapper.unmount()).not.toThrow()
  })

  describe('when the content overflows', () => {
    let spy: ReturnType<typeof vi.spyOn>

    beforeEach(() => {
      // jsdom reports scrollHeight as 0; force an overflow so the measure()
      // pass in onMounted flips `overflowing` to true.
      spy = vi.spyOn(HTMLElement.prototype, 'scrollHeight', 'get').mockReturnValue(9999)
    })
    afterEach(() => spy.mockRestore())

    it('reveals a "Show more" toggle and expands/collapses on click', async () => {
      const wrapper = mount(CollapsibleBox, {
        props: { maxHeight: 100 },
        slots: { default: slot },
      })
      await flushPromises()

      const toggle = wrapper.find('.collapsible-toggle')
      expect(toggle.exists()).toBe(true)
      expect(wrapper.find('.collapsible-box').classes()).toContain('collapsed')
      expect(toggle.text()).toContain('Show more')

      await toggle.trigger('click')
      expect(wrapper.find('.collapsible-box').classes()).not.toContain('collapsed')
      expect(wrapper.find('.collapsible-toggle').text()).toContain('Show less')

      await wrapper.find('.collapsible-toggle').trigger('click')
      expect(wrapper.find('.collapsible-box').classes()).toContain('collapsed')
    })
  })
})
