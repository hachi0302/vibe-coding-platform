import { flushPromises, mount } from '@vue/test-utils'
import { afterEach, describe, expect, it, vi } from 'vitest'

const { analyzeWithAgentMock, checkEnvironmentMock, createProjectMock, readRequirementMaterialsMock, openMock } = vi.hoisted(() => ({
  analyzeWithAgentMock: vi.fn(() => new Promise(() => {})),
  checkEnvironmentMock: vi.fn().mockResolvedValue([]),
  createProjectMock: vi.fn().mockResolvedValue({
    projectPaths: ['/tmp/order-admin'], agentMode: 'symlink', message: 'ok',
    verification: { status: 'passed', checks: [], detail: 'ok' },
  }),
  readRequirementMaterialsMock: vi.fn(),
  openMock: vi.fn(),
}))

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: openMock }))

vi.mock('../../src/projectFactory/api', () => ({
  analyzeWithAgent: analyzeWithAgentMock,
  checkEnvironment: checkEnvironmentMock,
  createProject: createProjectMock,
  installTool: vi.fn(),
  listenAnalysisProgress: vi.fn().mockResolvedValue(() => {}),
  readRequirementMaterials: readRequirementMaterialsMock,
}))

import ProjectFactoryView from '../../src/views/ProjectFactoryView.vue'

describe('ProjectFactoryView', () => {
  afterEach(() => {
    vi.useRealTimers()
    analyzeWithAgentMock.mockReset()
    analyzeWithAgentMock.mockImplementation(() => new Promise(() => {}))
    checkEnvironmentMock.mockReset()
    checkEnvironmentMock.mockResolvedValue([])
    createProjectMock.mockClear()
    readRequirementMaterialsMock.mockReset()
    openMock.mockReset()
  })

  it('keeps the elapsed timer moving while agent analysis is still running', async () => {
    vi.useFakeTimers()
    const wrapper = mount(ProjectFactoryView)
    await wrapper.find('textarea').setValue('做一个订单管理后台')
    await wrapper.get('button.pf-button.primary').trigger('click')
    await vi.advanceTimersByTimeAsync(50)
    await flushPromises()

    expect(wrapper.text()).toContain('已用时 0 秒')
    await vi.advanceTimersByTimeAsync(3000)
    expect(wrapper.text()).toContain('已用时 3 秒')
  })

  it('keeps the first analysis on the input screen and only advances after user confirmation', async () => {
    analyzeWithAgentMock.mockResolvedValue({
      provider: 'codex',
      recommended: {
        id: 'recommended', title: 'Vue 订单后台', frontend: ['Vue 3'], backend: ['Spring Boot'], database: ['MySQL'],
        cache: [], messaging: [], decisions: [], structure: 'frontend-backend', packageManager: 'maven', reasons: ['长期维护'],
        tradeoffs: [], preferenceMatched: true,
      },
      alternatives: [], notRecommended: [], assumptions: [], projectName: 'order-admin', projectNameReason: '按业务命名',
      recognizedConstraints: [],
      clarifyingQuestions: [{ id: 'role', label: '需要哪些角色？', selectionMode: 'single', options: [] }],
    })
    const wrapper = mount(ProjectFactoryView)
    await wrapper.find('textarea').setValue('做一个订单管理后台')
    await wrapper.get('button.pf-button.primary').trigger('click')
    await new Promise(resolve => setTimeout(resolve, 60))
    await flushPromises()

    expect(wrapper.text()).toContain('需求分析结论')
    expect(wrapper.text()).toContain('需要哪些角色？')
    expect(wrapper.text()).not.toContain('推荐技术方案')

    await wrapper.get('[data-testid="confirm-requirement-analysis"]').trigger('click')
    await flushPromises()
    expect(wrapper.text()).toContain('推荐技术方案')
  })

  it('creates from the confirmed concise facts without leaking local material text', async () => {
    const originalMaterial = '本机需求原文-'.repeat(300)
    openMock.mockResolvedValue('/tmp/order-materials')
    readRequirementMaterialsMock.mockResolvedValue({
      rootPath: '/tmp/order-materials', sourceLabel: '需求资料文件夹', text: originalMaterial,
      files: [{ relativePath: '需求.md', absolutePath: '/tmp/order-materials/需求.md', kind: 'markdown', included: true, detail: '已读取' }],
      warnings: [],
    })
    analyzeWithAgentMock.mockResolvedValue({
      provider: 'codex',
      recommended: {
        id: 'recommended', title: 'Vue 订单后台', frontend: ['Vue 3'], backend: ['Spring Boot'], database: ['MySQL'],
        cache: [], messaging: [], decisions: [], structure: 'frontend-backend', packageManager: 'maven',
        reasons: ['适合订单与权限业务'], tradeoffs: ['维护两个工程'], preferenceMatched: true,
      },
      alternatives: [], notRecommended: [], assumptions: ['首期仅支持内部员工'],
      projectName: 'order-admin', projectNameReason: '按业务命名',
      recognizedConstraints: [
        { id: 'product', label: '产品形态', value: '订单管理后台' },
        { id: 'users', label: '主要用户', value: '运营人员' },
      ],
      clarifyingQuestions: [],
    })

    const wrapper = mount(ProjectFactoryView)
    await wrapper.findAll('.pf-source-option')[1].trigger('click')
    await wrapper.get('[data-testid="choose-local-folder"]').trigger('click')
    await flushPromises()
    await wrapper.get('button.pf-button.primary').trigger('click')
    await new Promise(resolve => setTimeout(resolve, 60))
    await flushPromises()
    await wrapper.get('[data-testid="confirm-requirement-analysis"]').trigger('click')
    await wrapper.findAll('button').find(button => button.text() === '采用当前方案')!.trigger('click')
    await flushPromises()
    await wrapper.findAll('button').find(button => button.text() === '查看项目预览')!.trigger('click')
    await wrapper.find('.pf-preview-panel .pf-file-row input').setValue('/tmp')
    await wrapper.findAll('button').find(button => button.text() === '创建并自检项目')!.trigger('click')
    await flushPromises()

    const request = createProjectMock.mock.calls[0][0]
    expect(request.conciseRequirement).toBe('产品形态：订单管理后台；主要用户：运营人员')
    expect(request.conciseRequirement).not.toContain('本机需求原文')
    expect(request.profile.summary).toBe(request.conciseRequirement)
    expect(request.recognizedConstraints).toEqual([
      { id: 'product', label: '产品形态', value: '订单管理后台' },
      { id: 'users', label: '主要用户', value: '运营人员' },
    ])
    expect(request.assumptions).toEqual(['首期仅支持内部员工'])
    expect(request.recommendation).toMatchObject({
      reasons: ['适合订单与权限业务'],
      tradeoffs: ['维护两个工程'],
      preferenceMatched: true,
      packageManager: 'maven',
    })
  })
})
