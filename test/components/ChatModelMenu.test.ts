import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import ChatModelMenu from '../../src/components/ChatModelMenu.vue'
import { vTooltip } from '../../src/tooltip'

describe('ChatModelMenu', () => {
  it('uses displayValue when selected is empty, avoiding the generic Model placeholder after send', () => {
    const wrapper = mount(ChatModelMenu, {
      props: {
        agent: 'claude',
        selected: undefined,
        displayValue: 'sonnet',
        menuOptions: { claudeAliasMode: true },
      },
      global: { directives: { tooltip: vTooltip } },
    })
    expect(wrapper.text()).toContain('Sonnet')
    expect(wrapper.text()).not.toContain('Model')
  })

  it('checkmarks the displayValue model when selected is empty (resumed session, model not yet picked)', async () => {
    const wrapper = mount(ChatModelMenu, {
      props: { agent: 'claude', selected: undefined, displayValue: 'claude-sonnet-5' },
      global: { directives: { tooltip: vTooltip } },
    })
    await wrapper.find('button.mm-trigger').trigger('click')
    const checked = wrapper.findAll('.mm-item.active')
    expect(checked).toHaveLength(1)
    expect(checked[0].text()).toContain('Sonnet 5')
  })

  it('shows the real mapped model name for Claude alias menu items', async () => {
    const wrapper = mount(ChatModelMenu, {
      props: {
        agent: 'claude',
        selected: 'fable',
        displayValue: 'fable',
        menuOptions: {
          claudeAliasMode: true,
          claudeAliasTargets: {
            opus: 'mimo-v2.5-pro',
            sonnet: 'mimo-v2.5-pro',
            haiku: 'mimo-v2.5-pro',
            fable: 'mimo-v2.5-pro',
          },
        },
      },
      global: { directives: { tooltip: vTooltip } },
    })
    await wrapper.find('button.mm-trigger').trigger('click')
    expect(wrapper.text()).toContain('Opus (mimo-v2.5-pro)')
    expect(wrapper.text()).toContain('Fable (mimo-v2.5-pro)')
  })
})
