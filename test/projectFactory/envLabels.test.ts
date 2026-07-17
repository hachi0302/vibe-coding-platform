import { describe, expect, it } from 'vitest'
import { toolsForRecommendation } from '../../src/projectFactory/envLabels'
import type { StackRecommendation } from '../../src/projectFactory/types'

const javaAdmin: StackRecommendation = {
  id: 'vue-spring-boot', title: 'Vue 3 + Spring Boot 3', status: 'recommended',
  frontend: ['Vue 3'], backend: ['Spring Boot 3', 'Java 21', 'MyBatis-Plus'],
  database: ['MySQL 8'], cache: ['Redis'], messaging: [], decisions: [], structure: 'frontend-backend',
  packageManager: 'maven', reasons: [], tradeoffs: [], preferenceMatched: true,
}

describe('project factory environment labels', () => {
  it('checks only local tools required by a Java admin stack', () => {
    expect(toolsForRecommendation(javaAdmin).map(tool => tool.toolId)).toEqual([
      'node', 'jdk', 'maven',
    ])
  })

  it('does not require database and cache servers to be installed locally', () => {
    const ids = toolsForRecommendation(javaAdmin).map(tool => tool.toolId)
    expect(ids).not.toContain('mysql')
    expect(ids).not.toContain('redis')
  })

  it('includes Rust and Tauri only for desktop recommendations', () => {
    expect(toolsForRecommendation({
      ...javaAdmin,
      id: 'tauri-vue',
      backend: ['Rust', 'Tauri 2'],
      database: [], cache: [], messaging: [], decisions: [], packageManager: 'pnpm',
    }).map(tool => tool.toolId)).toEqual(['node', 'pnpm', 'rust', 'tauri'])
  })

  it('maps Python, Go and .NET API recommendations to their local runtimes', () => {
    const backendOnly = (id: string, backend: string[]): StackRecommendation => ({
      ...javaAdmin,
      id,
      frontend: [],
      backend,
      database: [],
      cache: [],
      messaging: [],
      decisions: [],
      structure: 'single-app',
      packageManager: undefined,
    })

    expect(toolsForRecommendation(backendOnly('fastapi-api', ['FastAPI', 'Python'])).map(tool => tool.toolId)).toEqual(['python'])
    expect(toolsForRecommendation(backendOnly('go-api', ['Go'])).map(tool => tool.toolId)).toEqual(['go'])
    expect(toolsForRecommendation(backendOnly('aspnet-api', ['ASP.NET Core', '.NET'])).map(tool => tool.toolId)).toEqual(['dotnet'])
  })
})
