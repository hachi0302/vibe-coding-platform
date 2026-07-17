import { describe, expect, it } from 'vitest'
import { toStackRecommendationResult } from '../../src/projectFactory/agentAnalysis'

describe('agent analysis result mapper', () => {
  it('uses the agent selected stack throughout the recommendation result', () => {
    const result = toStackRecommendationResult({
      provider: 'codex',
      recommended: {
        id: 'nextjs',
        title: 'Next.js + TypeScript',
        frontend: ['Next.js', 'TypeScript'],
        backend: [],
        database: [],
        cache: [],
        messaging: [],
        decisions: [],
        structure: 'single-app',
        packageManager: 'npm',
        reasons: ['需要 SEO'],
        tradeoffs: ['团队需要维护 React 生态'],
        preferenceMatched: false,
      },
      alternatives: [],
      notRecommended: [],
      assumptions: ['假设主要面向外部用户'],
      projectName: 'brand-site',
      projectNameReason: '品牌官网的业务语义。',
    }, {
      text: '做一个需要 SEO 的品牌官网',
      projectName: 'brand-site',
      preference: 'java',
    })

    expect(result.recommended.id).toBe('nextjs')
    expect(result.recommended.status).toBe('recommended')
    expect(result.recommended.reasons).toEqual(['需要 SEO'])
    expect(result.profile.systemType).toBe('web-h5')
    expect(result.assumptions).toEqual(['假设主要面向外部用户'])
    expect(result.provider).toBe('codex')
    expect(result.projectName).toBe('brand-site')
  })
})
