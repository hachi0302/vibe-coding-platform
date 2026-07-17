import { mount } from '@vue/test-utils'
import { describe, expect, it } from 'vitest'
import RecommendationPanel from '../../../src/components/projectFactory/RecommendationPanel.vue'

const result = {
  profile: { summary: '智能客服', systemType: 'fullstack', features: {} },
  recommended: {
    id: 'vue-spring-boot', title: 'Vue 3 + Spring Boot', status: 'recommended',
    frontend: ['Vue 3', 'TypeScript', 'Vite'], backend: ['Java', 'Spring Boot'],
    database: ['MySQL'], cache: ['Redis'], messaging: [], structure: 'frontend-backend', packageManager: 'maven',
    reasons: ['适合已有 Java 与 MySQL 的团队'], tradeoffs: ['第一期不引入 RAG'], preferenceMatched: true,
    decisions: [
      { category: 'frontend', title: '前端应用', status: 'adopt', choices: ['Vue 3', 'TypeScript', 'Vite'], reason: '管理工作台需要高效表单开发', provision: 'project' },
      { category: 'business-backend', title: '业务后端', status: 'adopt', choices: ['Java', 'Spring Boot'], reason: '承载业务 API 与事务', provision: 'project' },
      { category: 'agent', title: 'Agent 服务', status: 'defer', choices: ['Python', 'FastAPI', 'LangGraph'], reason: '当前需求尚未要求模型工作流', provision: 'project', trigger: '确认需要 LLM 工具调用或工作流后' },
      { category: 'persistence', title: '业务数据', status: 'adopt', choices: ['MySQL'], reason: '复用已有团队数据库', provision: 'existing-platform' },
      { category: 'engineering', title: '部署与工程化', status: 'adopt', choices: ['Docker Compose', 'Nginx', '结构化日志'], reason: '满足首版交付与排障', provision: 'project' },
    ],
  },
  alternatives: [], notRecommended: [], assumptions: ['复用已有 MySQL'], projectName: 'support-agent', projectNameReason: '客服 Agent',
}

describe('RecommendationPanel', () => {
  it('groups decisions into frontend, business backend, agent, data, and engineering sections', () => {
    const wrapper = mount(RecommendationPanel, { props: { result } })

    expect(wrapper.text()).toContain('前端应用')
    expect(wrapper.text()).toContain('业务后端')
    expect(wrapper.text()).toContain('Agent 服务')
    expect(wrapper.text()).toContain('数据与基础设施')
    expect(wrapper.text()).toContain('部署与工程化')
    expect(wrapper.text()).toContain('后续引入')
    expect(wrapper.text()).not.toContain('Codex')
    expect(wrapper.text()).not.toContain('Claude')
    expect(wrapper.findAll('.pf-decision-card')).toHaveLength(5)
    expect(wrapper.findAll('.pf-decision-card .pf-decision-status')).toHaveLength(5)
    expect(wrapper.text()).toContain('为什么这样选')
  })
})
