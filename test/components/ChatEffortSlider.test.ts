import { describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import ChatEffortSlider from '../../src/components/ChatEffortSlider.vue'
import { vTooltip } from '../../src/tooltip'

const mountSlider = (props: {
  selected: string | undefined
  defaultLevel?: string
  model?: string
}) =>
  mount(ChatEffortSlider, {
    props: { agent: 'claude', model: props.model, selected: props.selected, defaultLevel: props.defaultLevel },
    global: { directives: { tooltip: vTooltip } },
  })

describe('ChatEffortSlider', () => {
  it('shows the runtime defaultLevel (not a fabricated Low) when effort is unset', () => {
    // 续聊未改档：session.effort=undefined。CLI 不带 --effort 实际用 settings.effortLevel=xhigh，
    // 故触发器应显示 Extra High，而不是滑杆假定的最低档 Low。
    const wrapper = mountSlider({ selected: undefined, defaultLevel: 'xhigh' })
    expect(wrapper.find('.es-trigger').text()).toContain('Extra High')
    expect(wrapper.find('.es-trigger').text()).not.toContain('Low')
  })

  it('prefers the user-picked effort over the runtime default', () => {
    const wrapper = mountSlider({ selected: 'medium', defaultLevel: 'xhigh' })
    expect(wrapper.find('.es-trigger').text()).toContain('Medium')
  })

  it('falls back to the lowest level when neither selected nor a valid default is given', () => {
    const wrapper = mountSlider({ selected: undefined, defaultLevel: undefined })
    expect(wrapper.find('.es-trigger').text()).toContain('Low')
  })

  it('ignores a default that is not a valid level for the current model', () => {
    // ultracode 只在 Opus 4.7/4.8 存在；Sonnet 下不是合法档 → 应回落最低档，而非显示 Ultracode。
    const wrapper = mountSlider({ selected: undefined, defaultLevel: 'ultracode', model: 'claude-sonnet-5' })
    expect(wrapper.find('.es-trigger').text()).toContain('Low')
    expect(wrapper.find('.es-trigger').text()).not.toContain('Ultracode')
  })
})
