import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import CreateResultPanel from '../../../src/components/projectFactory/CreateResultPanel.vue'

describe('CreateResultPanel', () => {
  it('does not offer another-project restart after creation has completed', () => {
    const wrapper = mount(CreateResultPanel, {
      props: {
        result: {
          projectPaths: ['/tmp/retail-ops'],
          agentMode: 'symlink',
          message: '项目骨架已创建',
          verification: { status: 'passed', checks: ['构建通过'], detail: '已验证可启动' },
        },
      },
    })

    expect(wrapper.text()).toContain('项目骨架已创建')
    expect(wrapper.text()).not.toContain('创建另一个项目')
  })
})
