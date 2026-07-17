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
  emits: ['select-project', 'restore-background-task'],
  setup(props, { emit }) {
    return () => h('aside', { 'data-sidebar-stub': '' }, [
      h('button', {
        'data-select-project': '',
        onClick: () => emit('select-project', project.dirName),
      }, '选择项目'),
      ...(props.backgroundTasks as BackgroundTaskSummary[]).map((task) => h('button', {
        'data-background-task': task.kind,
        'data-elapsed': String(task.elapsedSeconds),
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
    existingProjectInitStatusMock.mockReset().mockResolvedValue({ initialized: false })
    initializeExistingProjectMock.mockReset()
    listenInitializationProgressMock.mockReset().mockResolvedValue(() => {})
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('increments elapsed time, restores the minimized card, and clears completed progress', async () => {
    const initialization = deferred<{ generated: string[] }>()
    initializeExistingProjectMock.mockReturnValue(initialization.promise)
    const wrapper = await mountAndStartInitialization()

    expect(wrapper.get('[data-background-task="initialization"]').attributes('data-elapsed')).toBe('0')
    await vi.advanceTimersByTimeAsync(3_000)
    await nextTick()
    expect(wrapper.get('[data-background-task="initialization"]').attributes('data-elapsed')).toBe('3')

    await wrapper.get('[data-background-task="initialization"]').trigger('click')
    await nextTick()
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(true)

    initialization.resolve({ generated: ['docs/项目总览.md'] })
    await settle()
    expect(wrapper.find('[data-background-task="initialization"]').exists()).toBe(false)
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(true)

    await vi.advanceTimersByTimeAsync(2_200)
    await nextTick()
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(false)
    wrapper.unmount()
  })

  it('automatically clears failed initialization progress', async () => {
    const initialization = deferred<{ generated: string[] }>()
    initializeExistingProjectMock.mockReturnValue(initialization.promise)
    const wrapper = await mountAndStartInitialization()

    await wrapper.get('[data-background-task="initialization"]').trigger('click')
    await nextTick()
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(true)

    initialization.reject(new Error('初始化失败'))
    await settle()
    expect(wrapper.find('[data-background-task="initialization"]').exists()).toBe(false)
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(true)

    await vi.advanceTimersByTimeAsync(2_200)
    await nextTick()
    expect(wrapper.find('.initialization-progress-overlay').exists()).toBe(false)
    wrapper.unmount()
  })
})
