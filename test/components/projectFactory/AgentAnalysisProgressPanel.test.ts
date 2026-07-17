import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import AgentAnalysisProgressPanel from '../../../src/components/projectFactory/AgentAnalysisProgressPanel.vue'

describe('AgentAnalysisProgressPanel', () => {
  it('shows product-facing analysis stages without exposing internal model providers', () => {
    const wrapper = mount(AgentAnalysisProgressPanel, {
      props: {
        progress: { phase: 'codex', percent: 42, detail: '正在比较候选技术方案' },
        elapsedSeconds: 23,
      },
    })

    expect(wrapper.text()).toContain('正在分析技术方案')
    expect(wrapper.text()).toContain('正在比较候选技术方案')
    expect(wrapper.text()).toContain('分析方案')
    expect(wrapper.text()).not.toContain('Codex')
    expect(wrapper.text()).not.toContain('Claude')
    expect(wrapper.text()).toContain('已用时 23 秒')
    expect(wrapper.get('[data-testid="analysis-progress-fill"]').attributes('style')).toContain('width: 42%')
  })

  it('can minimize a running task into the background', async () => {
    const wrapper = mount(AgentAnalysisProgressPanel, {
      props: {
        progress: { phase: 'codex', percent: 42, detail: '正在比较候选技术方案' },
        elapsedSeconds: 23,
        minimizable: true,
      },
    })

    await wrapper.get('[data-testid="minimize-progress"]').trigger('click')

    expect(wrapper.emitted('minimize')).toHaveLength(1)
  })
})
