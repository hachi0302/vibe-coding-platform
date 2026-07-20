import { defineComponent, h, inject, nextTick } from 'vue'
import { flushPromises, shallowMount, type VueWrapper } from '@vue/test-utils'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const {
  existingProjectInitStatusMock,
  initializeExistingProjectMock,
  listProjectsMock,
  listSessionsMock,
  listenInitializationProgressMock,
} = vi.hoisted(() => ({
  existingProjectInitStatusMock: vi.fn(),
  initializeExistingProjectMock: vi.fn(),
  listProjectsMock: vi.fn(),
  listSessionsMock: vi.fn(),
  listenInitializationProgressMock: vi.fn(),
}))

vi.mock('../src/api', async () => {
  const actual = await vi.importActual<typeof import('../src/api')>('../src/api')
  return {
    ...actual,
    listProjects: listProjectsMock,
    listSessions: listSessionsMock,
    listTrash: vi.fn().mockResolvedValue([]),
    detectTerminals: vi.fn().mockResolvedValue([]),
    setTitlebarTheme: vi.fn().mockResolvedValue(undefined),
    unwatchSession: vi.fn().mockResolvedValue(undefined),
  }
})

vi.mock('../src/projectFactory/api', async () => {
  const actual = await vi.importActual<typeof import('../src/projectFactory/api')>(
    '../src/projectFactory/api',
  )
  return {
    ...actual,
    existingProjectInitStatus: existingProjectInitStatusMock,
    initializeExistingProject: initializeExistingProjectMock,
    listenInitializationProgress: listenInitializationProgressMock,
  }
})

vi.mock('../src/updateCheck', () => ({
  runBackgroundCheck: vi.fn().mockResolvedValue(undefined),
}))

vi.mock('../src/menu', async () => {
  const actual = await vi.importActual<typeof import('../src/menu')>('../src/menu')
  return {
    ...actual,
    emitMenuSync: vi.fn(),
    installMenuRouter: vi.fn().mockResolvedValue(() => {}),
  }
})

vi.mock('@tauri-apps/api/event', () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}))

vi.mock('../src/chatSessions', async () => {
  const actual = await vi.importActual<typeof import('../src/chatSessions')>('../src/chatSessions')
  return {
    ...actual,
    reconnectChats: vi.fn().mockResolvedValue([]),
  }
})

import App from '../src/App.vue'
import { PaneActionsKey, type PaneActions } from '../src/paneActions'
import type { BackgroundTaskSummary } from '../src/projectFactory/backgroundTask'
import type { ExistingProjectInitResult } from '../src/projectFactory/types'
import type { ProjectInfo } from '../src/types'

const project: ProjectInfo = {
  dirName: 'demo-project',
  displayPath: '/work/demo-project',
  sessionCount: 0,
  lastModified: 0,
  exists: true,
}

const SidebarStub = defineComponent({
  name: 'Sidebar',
  props: {
    backgroundTasks: { type: Array, default: () => [] },
  },
  emits: ['select-project', 'restore-background-task', 'open-project-factory'],
  setup(props, { emit }) {
    return () => h('aside', { 'data-sidebar-stub': '' }, [
      h('button', {
        'data-select-project': '',
        onClick: () => emit('select-project', project.dirName),
      }, '选择项目'),
      h('button', {
        'data-open-project-factory': '',
        onClick: () => emit('open-project-factory'),
      }, '新项目工厂'),
      ...(props.backgroundTasks as BackgroundTaskSummary[]).map((task) => h('button', {
        'data-background-task': task.kind,
        'data-elapsed': String(task.elapsedSeconds),
        'data-detail': task.detail,
        'data-percent': String(task.percent),
        onClick: () => emit('restore-background-task', task.kind),
      }, task.title)),
    ])
  },
})

const PaneGridStub = defineComponent({
  name: 'PaneGrid',
  setup() {
    const actions = inject<PaneActions>(PaneActionsKey)
    return () => h('button', {
      'data-start-initialization': '',
      onClick: () => actions?.initializeProject(),
    }, '初始化项目')
  },
})

const ProjectFactoryViewStub = defineComponent({
  name: 'ProjectFactoryView',
  emits: ['task-progress', 'task-finished', 'minimize-analysis'],
  setup(_, { emit }) {
    const task: BackgroundTaskSummary = {
      kind: 'analysis',
      title: '技术方案分析中',
      detail: '正在分析方案',
      percent: 32,
      elapsedSeconds: 4,
    }
    return () => h('section', { 'data-project-factory': '' }, [
      h('button', { 'data-publish-analysis': '', onClick: () => emit('task-progress', task) }, '发布任务'),
      h('button', { 'data-minimize-analysis': '', onClick: () => emit('minimize-analysis') }, '缩小'),
      h('button', { 'data-finish-analysis': '', onClick: () => emit('task-finished') }, '完成'),
    ])
  },
})

interface Deferred<T> {
  promise: Promise<T>
  resolve: (value: T) => void
  reject: (reason?: unknown) => void
}

function deferred<T>(): Deferred<T> {
  let resolve!: (value: T) => void
  let reject!: (reason?: unknown) => void
  const promise = new Promise<T>((res, rej) => {
    resolve = res
    reject = rej
  })
  return { promise, resolve, reject }
}

async function settle() {
  await flushPromises()
  await nextTick()
}

async function mountAndStartInitialization(): Promise<VueWrapper> {
  const wrapper = shallowMount(App, {
    global: {
      directives: { tooltip: () => {} },
      stubs: {
        Sidebar: SidebarStub,
        PaneGrid: PaneGridStub,
        ProjectFactoryView: ProjectFactoryViewStub,
      },
    },
  })
  await settle()
  await wrapper.get('[data-select-project]').trigger('click')
  await settle()
  await wrapper.get('[data-start-initialization]').trigger('click')
  await settle()
  return wrapper
}

describe('App existing-project initialization progress', () => {
  beforeEach(() => {
    vi.useFakeTimers()
    localStorage.clear()
    listProjectsMock.mockReset().mockResolvedValue([project])
    listSessionsMock.mockReset().mockResolvedValue({ sessions: [], total: 0 })
    existingProjectInitStatusMock.mockReset().mockResolvedValue({
      initialized: false,
      status: 'not-initialized',
      recoverable: false,
    })
    initializeExistingProjectMock.mockReset()
    listenInitializationProgressMock.mockReset().mockResolvedValue(() => {})
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('shows live progress first, then minimizes to an elapsed task card and clears on completion', async () => {
    const initialization = deferred<ExistingProjectInitResult>()
    initializeExistingProjectMock.mockReturnValue(initialization.promise)
    const wrapper = await mountAndStartInitialization()

    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(true)
    expect(wrapper.find('[data-background-task="initialization"]').exists()).toBe(false)

    const progressHandler = listenInitializationProgressMock.mock.calls[0]?.[0]
    expect(progressHandler).toBeTypeOf('function')
    progressHandler({
      projectPath: project.displayPath,
      runId: 'run-live',
      phase: 'documents',
      percent: 44,
      detail: '正在根据真实代码生成中文项目文档',
      attempt: 1,
      sequence: 4,
      recoverable: true,
      issues: [],
      conflicts: [],
      warnings: [],
    })
    await nextTick()
    expect(wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).props('progress')).toMatchObject({
      phase: 'documents',
      percent: 44,
      detail: '正在根据真实代码生成中文项目文档',
    })

    await vi.advanceTimersByTimeAsync(3_000)
    await nextTick()

    wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).vm.$emit('minimize')
    await nextTick()
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(false)
    expect(wrapper.get('[data-background-task="initialization"]').attributes('data-elapsed')).toBe('3')

    await wrapper.get('[data-background-task="initialization"]').trigger('click')
    await nextTick()
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(true)

    initialization.resolve({
      projectPath: project.displayPath,
      runId: 'run-live',
      status: 'current-v4',
      phase: 'complete',
      attempt: 1,
      sequence: 9,
      recoverable: false,
      issues: [],
      conflicts: [],
      warnings: [],
      artifactTotals: { documents: 3, rules: 2, skills: 1, total: 6 },
      generated: ['docs/ai/project-map.md'],
    })
    await settle()
    expect(wrapper.find('[data-background-task="initialization"]').exists()).toBe(false)
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(true)

    progressHandler({
      projectPath: project.displayPath,
      runId: 'run-live',
      phase: 'rules',
      percent: 52,
      detail: '迟到的旧进度事件',
      attempt: 1,
      sequence: 5,
      recoverable: true,
      issues: [],
      conflicts: [],
      warnings: [],
    })
    await nextTick()
    expect(wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).props('progress')).toMatchObject({
      phase: 'complete',
      detail: '初始化完成：3 份文档、2 条规则、1 个 skill 已通过校验。',
    })

    await vi.advanceTimersByTimeAsync(2_200)
    await nextTick()
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(false)
    wrapper.unmount()
  })

  it('removes the minimized new-project task as soon as analysis finishes', async () => {
    const wrapper = shallowMount(App, {
      global: {
        directives: { tooltip: () => {} },
        stubs: {
          Sidebar: SidebarStub,
          PaneGrid: PaneGridStub,
          ProjectFactoryView: ProjectFactoryViewStub,
        },
      },
    })
    await settle()

    await wrapper.get('[data-open-project-factory]').trigger('click')
    await nextTick()
    await wrapper.get('[data-publish-analysis]').trigger('click')
    await wrapper.get('[data-minimize-analysis]').trigger('click')
    await nextTick()
    expect(wrapper.find('[data-background-task="analysis"]').exists()).toBe(true)

    wrapper.getComponent({ name: 'ProjectFactoryView' }).vm.$emit('task-finished')
    await nextTick()
    expect(wrapper.find('[data-background-task="analysis"]').exists()).toBe(false)
    wrapper.unmount()
  })

  it('keeps the exact failed initialization detail visible until explicit dismissal', async () => {
    const initialization = deferred<ExistingProjectInitResult>()
    initializeExistingProjectMock.mockReturnValue(initialization.promise)
    const wrapper = await mountAndStartInitialization()

    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(true)

    initialization.reject(new Error('artifact rules/backend.md 缺少验证命令'))
    await settle()
    expect(wrapper.find('[data-background-task="initialization"]').exists()).toBe(false)
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(true)
    expect(wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).props('progress')).toMatchObject({
      phase: 'failed',
      detail: 'artifact rules/backend.md 缺少验证命令',
    })

    await vi.advanceTimersByTimeAsync(2_200)
    await nextTick()
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(true)

    wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).vm.$emit('minimize')
    await nextTick()
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(false)
    wrapper.unmount()
  })

  it('resumes an incomplete v4 run from its reported stage', async () => {
    existingProjectInitStatusMock.mockResolvedValue({
      initialized: false,
      status: 'incomplete',
      runId: 'run-resume',
      phase: 'rules',
      percent: 58,
      detail: '规则已完成 2/4，继续当前运行',
      attempt: 2,
      sequence: 11,
      recoverable: true,
      issues: [],
      conflicts: [],
      warnings: ['上次运行已中断'],
    })
    const initialization = deferred<ExistingProjectInitResult>()
    initializeExistingProjectMock.mockReturnValue(initialization.promise)

    const wrapper = await mountAndStartInitialization()

    expect(initializeExistingProjectMock).toHaveBeenCalledTimes(1)
    expect(wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).props('progress')).toMatchObject({
      phase: 'rules',
      percent: 58,
      detail: '规则已完成 2/4，继续当前运行',
      runId: 'run-resume',
      attempt: 2,
      sequence: 11,
    })

    initialization.resolve({
      projectPath: project.displayPath,
      runId: 'run-resume',
      status: 'current-v4',
      phase: 'complete',
      attempt: 2,
      sequence: 20,
      recoverable: false,
      issues: [],
      conflicts: [],
      warnings: [],
      artifactTotals: { documents: 1, rules: 4, skills: 2, total: 7 },
      generated: [],
    })
    await settle()
    expect(wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).props('progress')).toMatchObject({
      phase: 'complete',
      detail: '初始化完成：1 份文档、4 条规则、2 个 skills 已通过校验。',
    })
    wrapper.unmount()
  })

  it('does not restart a completed current-v4 project', async () => {
    existingProjectInitStatusMock.mockResolvedValue({
      initialized: true,
      status: 'current-v4',
      markerVersion: 'v4',
      recoverable: false,
    })

    const wrapper = await mountAndStartInitialization()

    expect(initializeExistingProjectMock).not.toHaveBeenCalled()
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(false)
    wrapper.unmount()
  })
})
