import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import ClarifyingQuestionsPanel from '../../../src/components/projectFactory/ClarifyingQuestionsPanel.vue'

describe('ClarifyingQuestionsPanel', () => {
  it('keeps the answers visible and renders analysis progress in place', () => {
    const wrapper = mount(ClarifyingQuestionsPanel, {
      props: {
        questions: [{
          id: 'audience',
          label: '这个项目主要面向谁？',
          options: [{ value: 'internal-staff', label: '内部员工' }],
        }],
        analyzing: true,
        progress: { phase: 'codex', percent: 42, detail: '正在比较候选技术方案' },
        elapsedSeconds: 23,
      },
    })

    expect(wrapper.text()).toContain('正在比较候选技术方案')
    expect(wrapper.text()).not.toContain('Codex')
    expect(wrapper.text()).not.toContain('Claude')
    expect(wrapper.text()).toContain('已用时 23 秒')
    expect(wrapper.text()).not.toContain('生成方案')
  })

  it('shows inferred facts as locked context instead of repeating them as choices', () => {
    const wrapper = mount(ClarifyingQuestionsPanel, {
      props: {
        questions: [{
          id: 'scale',
          label: '项目规模大概是？',
          options: [{ value: 'small-production', label: '正式小项目' }],
        }],
        recognizedConstraints: [
          { id: 'systemType', label: '产品形态', value: '微信小程序' },
          { id: 'audience', label: '主要用户', value: '外部用户' },
        ],
      },
    })

    expect(wrapper.text()).toContain('已从需求识别')
    expect(wrapper.text()).toContain('产品形态：微信小程序')
    expect(wrapper.text()).toContain('主要用户：外部用户')
    expect(wrapper.text()).not.toContain('这个项目主要面向谁？')
  })

  it('defaults to AI-recommended answers and supports multi-select questions', async () => {
    const wrapper = mount(ClarifyingQuestionsPanel, {
      props: {
        questions: [
          {
            id: 'merchant-operations',
            label: '是否需要商家运营后台？',
            description: '预约和商品配置是否由商家自行管理会影响项目边界。',
            selectionMode: 'single',
            options: [
              { value: 'required', label: '需要独立商家后台', recommended: true },
              { value: 'not-required', label: '第一期不需要' },
            ],
          },
          {
            id: 'payment-scope',
            label: '第一期需要哪些支付方式？',
            selectionMode: 'multiple',
            options: [
              { value: 'wechat', label: '微信支付', recommended: true },
              { value: 'offline', label: '到店支付' },
            ],
          },
        ],
      },
    })

    expect(wrapper.get('[data-option="merchant-operations:required"]').classes()).toContain('active')
    expect(wrapper.get('[data-option="payment-scope:wechat"]').classes()).toContain('active')
    expect(wrapper.text()).toContain('推荐')

    await wrapper.get('[data-option="payment-scope:offline"]').trigger('click')
    await wrapper.get('button.pf-button.primary').trigger('click')

    expect(wrapper.emitted('submit')?.[0]).toEqual([{
      'merchant-operations': ['required'],
      'payment-scope': ['wechat', 'offline'],
    }])
  })
})
