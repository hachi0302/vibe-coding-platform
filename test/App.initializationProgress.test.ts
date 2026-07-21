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

    const initializedEvent = vi.fn()
    window.addEventListener('vibe-project-initialized', initializedEvent)
    progressHandler({
      projectPath: project.displayPath,
      runId: 'run-live',
      phase: 'complete',
      percent: 100,
      detail: '后端完成事件',
      attempt: 1,
      sequence: 9,
      recoverable: false,
      issues: [],
      conflicts: [],
      warnings: [],
      artifactTotals: { documents: 3, rules: 2, skills: 1, total: 6 },
    })
    await nextTick()
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
    expect(initializedEvent).toHaveBeenCalledTimes(1)
    window.removeEventListener('vibe-project-initialized', initializedEvent)
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
      detail: '初始化完成：已安装 3 份文档、2 条规则、1 个 skill。',
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

  it('hides raw failed initialization diagnostics until explicit dismissal', async () => {
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
      detail: '项目初始化未能安全完成。平台已保留恢复诊断，请处理安全问题后重试。',
    })

    await vi.advanceTimersByTimeAsync(2_200)
    await nextTick()
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(true)

    wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).vm.$emit('minimize')
    await nextTick()
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(false)
    wrapper.unmount()
  })

  it('keeps a resolved needs-attention result instead of claiming completion', async () => {
    initializeExistingProjectMock.mockResolvedValue({
      projectPath: project.displayPath,
      runId: 'run-attention',
      status: 'needs-attention',
      phase: 'conflict',
      percent: 82,
      detail: 'CLAUDE.md 已由用户修改，未安装任何产物',
      attempt: 2,
      sequence: 14,
      recoverable: false,
      issues: [{ code: 'install.conflict', detail: '安装前冲突检查失败' }],
      conflicts: [{ path: 'CLAUDE.md', detail: '当前哈希与扫描时不同' }],
      warnings: ['用户文件保持不变'],
      artifactTotals: { documents: 3, rules: 2, skills: 1, total: 6 },
      generated: [],
    })

    const wrapper = await mountAndStartInitialization()

    expect(wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).props('progress')).toMatchObject({
      phase: 'conflict',
      percent: 82,
      detail: '检测到 1 处用户文件冲突，请处理后重试。',
      runId: 'run-attention',
      sequence: 14,
      issues: [{ code: 'install.conflict', detail: '安装前冲突检查失败' }],
      conflicts: [{ path: 'CLAUDE.md', detail: '当前哈希与扫描时不同' }],
      warnings: ['用户文件保持不变'],
      artifactTotals: { documents: 3, rules: 2, skills: 1, total: 6 },
    })
    await vi.advanceTimersByTimeAsync(2_200)
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(true)
    wrapper.unmount()
  })

  it('treats current-v4 complete without artifact totals as a contract failure', async () => {
    initializeExistingProjectMock.mockResolvedValue({
      projectPath: project.displayPath,
      runId: 'run-no-totals',
      status: 'current-v4',
      phase: 'complete',
      detail: '后端称已完成但未返回报告计数',
      attempt: 1,
      sequence: 7,
      recoverable: false,
      issues: [{ code: 'report.totals.missing', detail: '完成报告缺少产物计数' }],
      conflicts: [],
      warnings: ['保留 run 诊断以便重试'],
      generated: [],
    })

    const wrapper = await mountAndStartInitialization()

    expect(wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).props('progress')).toMatchObject({
      phase: 'failed',
      detail: '初始化完成结果缺少 artifactTotals，无法确认产物数量。',
      runId: 'run-no-totals',
      attempt: 1,
      sequence: 7,
      recoverable: true,
      issues: [{ code: 'report.totals.missing', detail: '完成报告缺少产物计数' }],
      conflicts: [],
      warnings: ['保留 run 诊断以便重试'],
    })
    await vi.advanceTimersByTimeAsync(2_200)
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(true)
    wrapper.unmount()
  })

  it('keeps an exact terminal event when the invoke rejects afterward', async () => {
    const initialization = deferred<ExistingProjectInitResult>()
    initializeExistingProjectMock.mockReturnValue(initialization.promise)
    const wrapper = await mountAndStartInitialization()
    const progressHandler = listenInitializationProgressMock.mock.calls[0]?.[0]

    progressHandler({
      projectPath: project.displayPath,
      runId: 'run-terminal',
      phase: 'failed',
      percent: 67,
      detail: 'artifact-plan.json 缺少 backend 模块覆盖',
      attempt: 3,
      sequence: 12,
      recoverable: true,
      issues: [{ code: 'plan.module.uncovered', detail: 'backend 模块未覆盖' }],
      conflicts: [],
      warnings: ['已保留诊断'],
    })
    await nextTick()
    initialization.reject(new Error('agent process exited with code 1'))
    await settle()

    expect(wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).props('progress')).toMatchObject({
      phase: 'failed',
      percent: 67,
      detail: 'artifact-plan.json 缺少 backend 模块覆盖',
      issues: [{ code: 'plan.module.uncovered', detail: 'backend 模块未覆盖' }],
      warnings: ['已保留诊断'],
    })
    wrapper.unmount()
  })

  it('isolates events by run id while allowing a new run to reset sequence', async () => {
    existingProjectInitStatusMock.mockReset()
      .mockResolvedValueOnce({ initialized: false, status: 'not-initialized', recoverable: false })
      .mockResolvedValueOnce({
        initialized: false,
        status: 'incomplete',
        runId: 'run-next',
        phase: 'scan',
        percent: 5,
        detail: '开始新的恢复运行',
        attempt: 1,
        sequence: 0,
        recoverable: true,
        issues: [],
        conflicts: [],
        warnings: [],
      })
    const first = deferred<ExistingProjectInitResult>()
    const second = deferred<ExistingProjectInitResult>()
    initializeExistingProjectMock
      .mockReturnValueOnce(first.promise)
      .mockReturnValueOnce(second.promise)
    const wrapper = await mountAndStartInitialization()
    const progressHandler = listenInitializationProgressMock.mock.calls[0]?.[0]

    progressHandler({
      projectPath: project.displayPath,
      runId: 'run-first',
      phase: 'documents',
      percent: 42,
      detail: '第一轮文档阶段',
      attempt: 1,
      sequence: 50,
      recoverable: true,
      issues: [],
      conflicts: [],
      warnings: [],
    })
    await nextTick()
    progressHandler({
      projectPath: project.displayPath,
      runId: 'run-old',
      phase: 'rules',
      percent: 55,
      detail: '其他运行的事件',
      attempt: 1,
      sequence: 99,
      recoverable: true,
      issues: [],
      conflicts: [],
      warnings: [],
    })
    await nextTick()
    expect(wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).props('progress')).toMatchObject({
      runId: 'run-first',
      phase: 'documents',
      sequence: 50,
    })

    first.reject(new Error('第一轮已退出'))
    await settle()
    await wrapper.get('[data-start-initialization]').trigger('click')
    await settle()
    progressHandler({
      projectPath: project.displayPath,
      runId: 'run-next',
      phase: 'plan',
      percent: 18,
      detail: '新运行从低 sequence 开始',
      attempt: 1,
      sequence: 1,
      recoverable: true,
      issues: [],
      conflicts: [],
      warnings: [],
    })
    await nextTick()
    progressHandler({
      projectPath: project.displayPath,
      runId: 'run-first',
      phase: 'skills',
      percent: 70,
      detail: '第一轮迟到事件',
      attempt: 1,
      sequence: 100,
      recoverable: true,
      issues: [],
      conflicts: [],
      warnings: [],
    })
    await nextTick()
    expect(wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).props('progress')).toMatchObject({
      runId: 'run-next',
      phase: 'plan',
      sequence: 1,
      detail: '新运行从低 sequence 开始',
    })
    wrapper.unmount()
  })

  it('ignores an old invoke rejection after dismissal and a new run has started', async () => {
    existingProjectInitStatusMock.mockReset()
      .mockResolvedValueOnce({ initialized: false, status: 'not-initialized', recoverable: false })
      .mockResolvedValueOnce({
        initialized: false,
        status: 'incomplete',
        runId: 'run-new',
        phase: 'scan',
        percent: 5,
        detail: '新运行开始',
        attempt: 1,
        sequence: 0,
        recoverable: true,
        issues: [],
        conflicts: [],
        warnings: [],
      })
    const oldInvocation = deferred<ExistingProjectInitResult>()
    const newInvocation = deferred<ExistingProjectInitResult>()
    initializeExistingProjectMock
      .mockReturnValueOnce(oldInvocation.promise)
      .mockReturnValueOnce(newInvocation.promise)
    const wrapper = await mountAndStartInitialization()
    const progressHandler = listenInitializationProgressMock.mock.calls[0]?.[0]

    progressHandler({
      projectPath: project.displayPath,
      runId: 'run-old',
      phase: 'failed',
      percent: 60,
      detail: '旧运行已失败，等待 invoke 退出',
      attempt: 2,
      sequence: 8,
      recoverable: true,
      issues: [{ code: 'old.failed', detail: '旧运行失败' }],
      conflicts: [],
      warnings: [],
    })
    await nextTick()
    wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).vm.$emit('minimize')
    await nextTick()

    await wrapper.get('[data-start-initialization]').trigger('click')
    await settle()
    progressHandler({
      projectPath: project.displayPath,
      runId: 'run-new',
      phase: 'plan',
      percent: 19,
      detail: '新运行正在规划产物',
      attempt: 1,
      sequence: 1,
      recoverable: true,
      issues: [],
      conflicts: [],
      warnings: [],
    })
    await nextTick()

    oldInvocation.reject(new Error('旧 invoke 最终退出'))
    await settle()
    expect(wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).props('progress')).toMatchObject({
      runId: 'run-new',
      phase: 'plan',
      percent: 19,
      sequence: 1,
      detail: '新运行正在规划产物',
    })
    wrapper.unmount()
  })

  it('restarts elapsed time when a terminal run returns to a running phase', async () => {
    const initialization = deferred<ExistingProjectInitResult>()
    initializeExistingProjectMock.mockReturnValue(initialization.promise)
    const wrapper = await mountAndStartInitialization()
    const progressHandler = listenInitializationProgressMock.mock.calls[0]?.[0]

    progressHandler({
      projectPath: project.displayPath,
      runId: 'run-recover',
      phase: 'interrupted',
      percent: 64,
      detail: '进程中断',
      attempt: 1,
      sequence: 6,
      recoverable: true,
      issues: [],
      conflicts: [],
      warnings: [],
    })
    await nextTick()
    await vi.advanceTimersByTimeAsync(1_000)
    expect(wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).props('elapsedSeconds')).toBe(0)

    progressHandler({
      projectPath: project.displayPath,
      runId: 'run-recover',
      phase: 'skills',
      percent: 66,
      detail: '已恢复 skills 阶段',
      attempt: 2,
      sequence: 7,
      recoverable: true,
      issues: [],
      conflicts: [],
      warnings: ['从有效节点恢复'],
    })
    await nextTick()
    await vi.advanceTimersByTimeAsync(2_000)
    expect(wrapper.getComponent({ name: 'AgentAnalysisProgressPanel' }).props('elapsedSeconds')).toBe(2)
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
      detail: '初始化完成：已安装 1 份文档、4 条规则、2 个 skills。',
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
