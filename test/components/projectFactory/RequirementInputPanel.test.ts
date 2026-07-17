import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import RequirementInputPanel from '../../../src/components/projectFactory/RequirementInputPanel.vue'

describe('RequirementInputPanel', () => {
  const baseProps = {
    kind: 'text' as const,
    text: '做一个支持订单管理和权限控制的内部后台',
    sourceValue: '',
    structurePreference: 'auto' as const,
    followUp: '',
  }

  it('only presents direct description and local materials as requirement sources', () => {
    const wrapper = mount(RequirementInputPanel, {
      props: baseProps,
    })

    const sourceButtons = wrapper.findAll('.pf-source-option')
    expect(sourceButtons).toHaveLength(2)
    expect(wrapper.text()).toContain('直接描述')
    expect(wrapper.text()).toContain('选择本机资料')
    expect(wrapper.text()).not.toContain('Word')
    expect(wrapper.text()).not.toContain('PDF')
    expect(wrapper.text()).not.toContain('飞书文档')
    expect(wrapper.text()).not.toContain('网页链接')
    expect(wrapper.text()).not.toContain('项目名称')
    expect(wrapper.find('input[placeholder="例如：order-admin"]').exists()).toBe(false)
    expect(wrapper.get('button.pf-button.primary').attributes('disabled')).toBeUndefined()
  })

  it('offers separate file and folder pickers inside the single local material source', async () => {
    const wrapper = mount(RequirementInputPanel, {
      props: { ...baseProps, kind: 'local' },
    })

    await wrapper.get('[data-testid="choose-local-file"]').trigger('click')
    await wrapper.get('[data-testid="choose-local-folder"]').trigger('click')

    expect(wrapper.emitted('choose-file')).toHaveLength(1)
    expect(wrapper.emitted('choose-folder')).toHaveLength(1)
  })

  it('keeps analysis conclusion, one follow-up box and the confirm action on the same screen', async () => {
    const wrapper = mount(RequirementInputPanel, {
      props: {
        ...baseProps,
        analysis: {
          provider: 'codex',
          recommended: {
            id: 'recommended', title: 'Vue + Spring Boot 订单后台', frontend: ['Vue 3'], backend: ['Spring Boot'],
            database: ['MySQL'], cache: [], messaging: [], decisions: [], structure: 'frontend-backend',
            packageManager: 'maven', reasons: ['适合长期维护'], tradeoffs: [], preferenceMatched: true,
          },
          alternatives: [], notRecommended: [], assumptions: ['沿用现有 MySQL'], projectName: 'order-admin',
          projectNameReason: '根据订单管理场景命名',
          recognizedConstraints: [{ id: 'auth', label: '权限', value: '需要登录权限' }],
          clarifyingQuestions: [{ id: 'role', label: '需要哪些角色？', selectionMode: 'single', options: [] }],
        },
      },
    })

    expect(wrapper.text()).toContain('需求分析结论')
    expect(wrapper.text()).toContain('Vue + Spring Boot 订单后台')
    expect(wrapper.text()).toContain('需要哪些角色？')
    expect(wrapper.findAll('[data-testid="analysis-follow-up"]')).toHaveLength(1)

    await wrapper.get('[data-testid="analysis-follow-up"]').setValue('只需要管理员和运营两个角色')
    await wrapper.setProps({ followUp: '只需要管理员和运营两个角色' })
    await wrapper.get('[data-testid="reanalyze-requirement"]').trigger('click')
    await wrapper.get('[data-testid="confirm-requirement-analysis"]').trigger('click')

    expect(wrapper.emitted('reanalyze')).toHaveLength(1)
    expect(wrapper.emitted('confirm-analysis')).toHaveLength(1)
  })
})
