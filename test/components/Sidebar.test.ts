import { beforeEach, describe, expect, it } from 'vitest'
import { mount } from '@vue/test-utils'
import Sidebar from '../../src/components/Sidebar.vue'
import { vTooltip } from '../../src/tooltip'
import { setLang } from '../../src/settings'
import type { ProjectInfo } from '../../src/types'

beforeEach(() => setLang('en'))

const project = (over: Partial<ProjectInfo> & { dirName: string }): ProjectInfo => ({
  displayPath: `/projects/${over.dirName}`,
  sessionCount: 1,
  lastModified: 0,
  exists: true,
  ...over,
})

type Props = InstanceType<typeof Sidebar>['$props']
const factory = (props: Partial<Props> = {}) =>
  mount(Sidebar, {
    props: {
      agent: 'claude',
      projects: [],
      activeDir: null,
      showTrash: false,
      projPrefs: {},
      ...props,
    } as Props,
    global: { directives: { tooltip: vTooltip } },
  })

describe('Sidebar', () => {
  it('shows the agent name in the sub-header', () => {
    expect(factory({ agent: 'claude' }).find('.sidebar-sub').text()).toContain('Claude')
    expect(factory({ agent: 'codex' }).find('.sidebar-sub').text()).toContain('Codex')
  })

  it('renders one row per project', () => {
    const wrapper = factory({
      projects: [project({ dirName: 'a' }), project({ dirName: 'b' })],
    })
    expect(wrapper.findAll('.proj-item')).toHaveLength(2)
  })

  it('shows the project factory action outside the original project rows', async () => {
    const wrapper = factory({
      projects: [project({ dirName: 'current-project' })],
      activeDir: 'current-project',
    })

    expect(wrapper.find('[data-project-factory-entry]').text()).toContain('New project factory')
    expect(wrapper.findAll('.proj-item')).toHaveLength(1)

    await wrapper.find('[data-project-factory-entry]').trigger('click')
    expect(wrapper.emitted('open-project-factory')).toHaveLength(1)
  })

  it('shows concurrent background tasks below the project factory and restores the selected task', async () => {
    const wrapper = factory({
      backgroundTasks: [
        {
          kind: 'initialization',
          title: '正在初始化 iam',
          detail: '正在生成项目文档',
          percent: 32,
          elapsedSeconds: 18,
        },
        {
          kind: 'analysis',
          title: '技术方案分析中',
          detail: '正在比较候选技术方案',
          percent: 42,
          elapsedSeconds: 23,
        },
      ],
    } as any)

    const tasks = wrapper.findAll('[data-background-task]')
    expect(tasks).toHaveLength(2)
    expect(tasks[0].text()).toContain('正在初始化 iam')
    expect(tasks[0].attributes('style')).toContain('--task-progress: 32%')
    expect(tasks[1].text()).toContain('技术方案分析中')
    expect(tasks[1].text()).toContain('23 秒')
    expect(tasks[1].attributes('style')).toContain('--task-progress: 42%')

    await tasks[0].trigger('click')
    await tasks[1].trigger('click')
    expect(wrapper.emitted('restore-background-task')).toEqual([['initialization'], ['analysis']])
  })

  it('shows the empty-state message when there are no projects', () => {
    const wrapper = factory({ projects: [] })
    expect(wrapper.findAll('.proj-item')).toHaveLength(0)
    expect(wrapper.text()).toContain('No Claude sessions')
  })

  it('emits switch-agent when an agent tab is clicked', async () => {
    const wrapper = factory({ agent: 'claude' })
    await wrapper.findAll('.agent-switch button')[1].trigger('click')
    expect(wrapper.emitted('switch-agent')![0]).toEqual(['codex'])
  })

  it('emits select-project with the project dirName', async () => {
    const wrapper = factory({ projects: [project({ dirName: 'proj-x' })] })
    await wrapper.find('.proj-item').trigger('click')
    expect(wrapper.emitted('select-project')![0]).toEqual(['proj-x'])
  })

  it('emits context-menu on right-click', async () => {
    const wrapper = factory({ projects: [project({ dirName: 'p' })] })
    await wrapper.find('.proj-item').trigger('contextmenu')
    expect(wrapper.emitted('context-menu')).toHaveLength(1)
  })

  it('emits open-settings from the footer button', async () => {
    const wrapper = factory()
    await wrapper.find('.trash-tab').trigger('click')
    expect(wrapper.emitted('open-settings')).toHaveLength(1)
  })

  it('orders pinned projects first and sunk projects last', () => {
    const wrapper = factory({
      projects: [
        project({ dirName: 'normal' }),
        project({ dirName: 'pinned' }),
        project({ dirName: 'sunk' }),
      ],
      projPrefs: { 'claude::pinned': 'pinned', 'claude::sunk': 'sunk' },
    })
    const names = wrapper.findAll('.proj-name').map((n) => n.text())
    expect(names).toEqual(['pinned', 'normal', 'sunk'])
  })

  it('renders a pin dot only for pinned projects', () => {
    const wrapper = factory({
      projects: [project({ dirName: 'p' }), project({ dirName: 'q' })],
      projPrefs: { 'claude::p': 'pinned' },
    })
    const items = wrapper.findAll('.proj-item')
    expect(items[0].find('.pin-dot').exists()).toBe(true)
    expect(items[1].find('.pin-dot').exists()).toBe(false)
  })

  it('marks the active project, but not while the trash view is open', () => {
    const projects = [project({ dirName: 'here' })]
    expect(
      factory({ projects, activeDir: 'here', showTrash: false }).find('.proj-item').classes(),
    ).toContain('active')
    expect(
      factory({ projects, activeDir: 'here', showTrash: true }).find('.proj-item').classes(),
    ).not.toContain('active')
  })

  it('flags a project whose directory no longer exists', () => {
    const wrapper = factory({ projects: [project({ dirName: 'gone', exists: false })] })
    expect(wrapper.find('.proj-item').classes()).toContain('missing')
  })

  it('shows the session count and the short project name', () => {
    const wrapper = factory({
      projects: [project({ dirName: 'x', displayPath: '/a/b/my-proj', sessionCount: 12 })],
    })
    expect(wrapper.find('.proj-name').text()).toBe('my-proj')
    expect(wrapper.find('.proj-count').text()).toBe('12')
  })
})
