import { describe, expect, it } from 'vitest'
import { buildPreview } from '../../src/projectFactory/previewBuilder'
import type { ProjectProfile, StackRecommendation } from '../../src/projectFactory/types'

const recommendation: StackRecommendation = {
  id: 'vue-spring-boot', title: 'Vue 3 + Spring Boot 3', status: 'recommended',
  frontend: ['Vue 3'], backend: ['Spring Boot 3', 'Java 21'], database: ['MySQL 8'],
  cache: [], messaging: [], decisions: [], structure: 'frontend-backend', packageManager: 'maven',
  reasons: [], tradeoffs: [], preferenceMatched: true,
}

const profile: ProjectProfile = {
  summary: '订单后台', systemType: 'admin', features: {
    seo: false, mobileFirst: false, auth: true, fileUpload: false, realtime: false,
    paymentOrOrder: true, adminConsole: true, crossPlatform: false, offline: false,
  },
}

describe('project factory preview builder', () => {
  it('shows both agent entry files and symlink strategy when both agents are selected', () => {
    const preview = buildPreview({
      projectName: 'order-admin',
      parentPath: '/tmp/projects',
      frontendProjectName: 'order-console',
      backendProjectName: 'order-service',
      recommendation,
      profile,
      agentChoice: 'both',
    })

    expect(preview.targetPaths).toEqual([
      { label: '前端项目', path: '/tmp/projects/order-console' },
      { label: '后端项目', path: '/tmp/projects/order-service' },
    ])
    expect(preview.directories).toContain('src/')
    expect(preview.agentFiles).toEqual(expect.arrayContaining([
      'CLAUDE.md', 'AGENTS.md', '.claude/rules/', '.agents/rules/ → .claude/rules/',
    ]))
    expect(preview.agentMode).toBe('symlink')
  })
})
