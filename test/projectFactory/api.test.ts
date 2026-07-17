import { beforeEach, describe, expect, it, vi } from 'vitest'

const { invoke, listen } = vi.hoisted(() => ({
  invoke: vi.fn(),
  listen: vi.fn(),
}))

vi.mock('@tauri-apps/api/core', () => ({ invoke }))
vi.mock('@tauri-apps/api/event', () => ({ listen }))

import {
  initializeExistingProject,
  listenInitializationProgress,
} from '../../src/projectFactory/api'

beforeEach(() => {
  invoke.mockReset()
  listen.mockReset()
  invoke.mockResolvedValue(undefined)
  listen.mockResolvedValue(() => {})
})

describe('existing project initialization api', () => {
  it('runs the headless backend command with the selected agent', () => {
    initializeExistingProject('/tmp/demo', 'codex', '真实项目初始化约束')

    expect(invoke).toHaveBeenCalledWith('project_factory_initialize_existing_project', {
      projectPath: '/tmp/demo',
      agent: 'codex',
      prompt: '真实项目初始化约束',
    })
  })

  it('forwards backend progress events without creating a chat session', async () => {
    const handler = vi.fn()
    listen.mockImplementation((_eventName, callback) => {
      callback({ payload: { projectPath: '/tmp/demo', phase: 'documents', percent: 42, detail: '正在生成中文文档' } })
      return Promise.resolve(() => {})
    })

    await listenInitializationProgress(handler)

    expect(listen).toHaveBeenCalledWith('project-factory://initialization-progress', expect.any(Function))
    expect(handler).toHaveBeenCalledWith(expect.objectContaining({ phase: 'documents', percent: 42 }))
  })
})
