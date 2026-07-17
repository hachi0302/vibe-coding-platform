import { describe, expect, it } from 'vitest'
import { recommendStacks } from '../../src/projectFactory/stackSelector'

describe('project factory stack selector', () => {
  it('prioritizes Spring Boot when a maintainable admin system matches Java preference', () => {
    const result = recommendStacks({
      text: '做一个订单管理后台，有登录权限、订单列表、支付记录和长期维护',
      audience: 'internal-staff',
      scale: 'large-maintained',
      preference: 'java',
    })

    expect(result.profile.systemType).toBe('admin')
    expect(result.recommended.backend).toContain('Spring Boot')
    expect(result.recommended.preferenceMatched).toBe(true)
    expect(result.recommended.reasons.join(' ')).toMatch(/长期维护|订单|权限/)
    expect(result.recommended).not.toHaveProperty('score')
    expect(result.alternatives.length).toBeGreaterThan(0)
  })

  it('keeps a SEO focused H5 site as a lightweight frontend project', () => {
    const result = recommendStacks({
      text: '做一个品牌 H5 官网，需要 SEO 和移动端访问',
      audience: 'external-users',
      scale: 'small-production',
      preference: 'none',
    })

    expect(result.profile.systemType).toBe('web-h5')
    expect(result.recommended.frontend.length).toBeGreaterThan(0)
    expect(result.recommended.backend).toEqual([])
    expect(result.recommended.reasons.join(' ')).toMatch(/SEO|移动端/)
  })

  it('recommends a cross-platform desktop stack for local AI session tools', () => {
    const result = recommendStacks({
      text: '做一个桌面客户端，用来查看本机 AI 会话和项目文件',
      preference: 'none',
    })

    expect(result.profile.systemType).toBe('desktop')
    expect(result.recommended.title).toMatch(/Tauri/)
    expect(result.recommended.frontend).toContain('Vue 3')
    expect(result.recommended.backend).toContain('Rust')
  })
})
