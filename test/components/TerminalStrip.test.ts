import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import TerminalStrip from '../../src/components/TerminalStrip.vue'
import { vTooltip } from '../../src/tooltip'
import { setLang } from '../../src/settings'
import {
  activeUiId,
  markTabSessionActivity,
  markTabTurnStarted,
  markTabTurnCompleted,
  reconcileNewTabs,
  syncTabTitlesFromSessions,
  tabs,
  type TerminalTab,
} from '../../src/terminals'
import { PaneActionsKey, type PaneActions } from '../../src/paneActions'

vi.mock('../../src/api', () => ({
  watchSessionTurn: vi.fn().mockResolvedValue(undefined),
  unwatchSessionTurn: vi.fn().mockResolvedValue(undefined),
}))

beforeEach(() => {
  setLang('en')
  activeUiId.value = null
  tabs.value = []
})

const PANE_ID = 99

function tab(over: Partial<TerminalTab> = {}): TerminalTab {
  return {
    uiId: 1,
    ptyId: 1,
    agent: 'codex',
    projectKey: 'proj',
    paneId: PANE_ID,
    sessionId: '',
    sessionPath: '',
    title: 'New session',
    cwd: '/repo',
    createdAt: 1_000,
    term: {} as TerminalTab['term'],
    fitAddon: {} as TerminalTab['fitAddon'],
    container: document.createElement('div'),
    unlistenData: null,
    unlistenExit: null,
    onDataDisp: null,
    lastSyncedCols: 80,
    lastSyncedRows: 24,
    currentInputLine: '',
    pendingAnsiBytes: null,
    processState: 'alive',
    turnState: 'unknown',
    turnStateSource: null,
    turnStateUpdatedAt: 1_000,
    lastOutputAt: 0,
    lastSessionActivityAt: 0,
    turnWatchPath: null,
    status: 'running',
    ...over,
  }
}

const stubPaneActions: PaneActions = {
  onTuiListClick: () => {},
  onTuiViewTabClick: () => {},
  onTuiViewClose: () => {},
  onViewRename: () => {},
  onViewCloseOthers: () => {},
  onViewCloseProject: () => {},
  onCloseOthersAll: () => {},
  onCloseAll: () => {},
  onTuiTabClosed: () => {},
  openRenameFromTuiTab: () => {},
  openRenameFromSavedTab: () => {},
  saveTabState: () => {},
  newSession: () => {},
  newDefaultAction: () => {},
  newGuiSession: () => {},
  newShellSession: () => {},
  openGitChanges: () => {},
  refreshSessions: () => {},
  hydrateSavedTab: () => {},
  splitH: () => {},
  splitV: () => {},
  exitPane: () => {},
} as unknown as PaneActions

function factory() {
  return mount(TerminalStrip, {
    props: {
      pane: {
        id: PANE_ID,
        agent: 'codex' as const,
        projectKey: 'proj',
        activeUiId: null,
        activeViewTabId: null,
      },
      agent: 'codex',
      projectKey: 'proj',
      inProjectBrowse: true,
      hasGit: false,
      viewTabs: [],
      activeViewTabId: null,
    },
    global: {
      directives: { tooltip: vTooltip },
      provide: { [PaneActionsKey as symbol]: stubPaneActions },
    },
  })
}

describe('TerminalStrip', () => {
  it('opens the tab context menu from right-click and keeps existing actions', async () => {
    const t = tab()
    tabs.value = [t]
    const wrapper = factory()

    await wrapper.findAll('.term-tab').slice(-1)[0].trigger('contextmenu', {
      clientX: 80,
      clientY: 40,
    })

    const actions = wrapper
      .findAll('.term-tab-ctx-menu .ctx-item')
      .map((item) => item.attributes('data-menu-action'))
    expect(actions).toEqual([
      'tab-rename',
      'tab-close',
      'tab-close-others',
      'tab-close-project',
      undefined, // close others all
      undefined, // close all
    ])
    expect(wrapper.emitted('tabRename')).toBeUndefined()
  })

  it('does not open the tab context menu from left-click', async () => {
    const t = tab()
    tabs.value = [t]
    const wrapper = factory()

    await wrapper.findAll('.term-tab').slice(-1)[0].trigger('click')

    expect(wrapper.find('.term-tab-more').exists()).toBe(false)
    expect(wrapper.find('.term-tab-ctx-menu').exists()).toBe(false)
    expect(wrapper.emitted('tabRename')).toBeUndefined()
  })

  it('emits newDefault when double-clicking blank space in the strip', async () => {
    const t = tab()
    tabs.value = [t]
    const wrapper = factory()

    await wrapper.find('.terminal-strip').trigger('dblclick')

    expect(wrapper.emitted('newDefault')).toHaveLength(1)
  })

  it('does not emit newDefault when double-clicking an existing tab', async () => {
    const t = tab()
    tabs.value = [t]
    const wrapper = factory()

    await wrapper.findAll('.term-tab').slice(-1)[0].trigger('dblclick')

    expect(wrapper.emitted('newDefault')).toBeUndefined()
  })

  it('emits newSession from the blank-strip context menu', async () => {
    const t = tab()
    tabs.value = [t]
    const wrapper = factory()

    await wrapper.find('.terminal-strip').trigger('contextmenu', {
      clientX: 120,
      clientY: 24,
    })

    expect(wrapper.find('.new-menu-floating').exists()).toBe(true)
    await wrapper.find('.new-menu-floating .new-menu-item').trigger('click')

    expect(wrapper.emitted('newSession')).toHaveLength(1)
  })

  it('does not open the blank-strip context menu from an existing tab', async () => {
    const t = tab()
    tabs.value = [t]
    const wrapper = factory()

    await wrapper.findAll('.term-tab').slice(-1)[0].trigger('contextmenu', {
      clientX: 80,
      clientY: 40,
    })

    expect(wrapper.find('.term-strip-ctx-menu').exists()).toBe(false)
    expect(wrapper.find('.term-tab-ctx-menu').exists()).toBe(true)
    expect(wrapper.emitted('newSession')).toBeUndefined()
  })

  it('emits tabRename when choosing rename from the context menu', async () => {
    const t = tab()
    tabs.value = [t]
    const wrapper = factory()

    await wrapper.findAll('.term-tab').slice(-1)[0].trigger('contextmenu', {
      clientX: 80,
      clientY: 40,
    })
    await wrapper.find('[data-menu-action="tab-rename"]').trigger('click')

    expect(wrapper.emitted('tabRename')![0]).toEqual([t])
  })

  it('renders the pulse dots for a working tab', () => {
    const t = tab({ turnState: 'working' })
    tabs.value = [t]
    const wrapper = factory()

    const item = wrapper.findAll('.term-tab').slice(-1)[0]
    expect(item.classes()).toContain('state-working')
    expect(item.findAll('.term-tab-status-working i')).toHaveLength(3)
  })

  it('renders done only after an explicit turn completion signal', () => {
    const t = tab({
      sessionPath: '/repo/session.jsonl',
      sessionId: 'session-1',
      turnState: 'working',
    })
    tabs.value = [t]

    markTabSessionActivity('codex', '/repo/session.jsonl')
    expect(t.turnState).toBe('working')

    markTabTurnCompleted('codex', '/repo/session.jsonl')
    expect(t.turnState).toBe('review')

    const wrapper = factory()
    const item = wrapper.findAll('.term-tab').slice(-1)[0]
    expect(item.classes()).toContain('state-done')
    expect(item.find('.term-tab-status-done').exists()).toBe(true)
  })

  it('does not downgrade a session-jsonl-completed turn when session append arrives later', () => {
    const t = tab({
      sessionPath: '/repo/session.jsonl',
      sessionId: 'session-1',
      turnState: 'working',
    })
    tabs.value = [t]

    markTabTurnCompleted('codex', '/repo/session.jsonl')
    expect(t.turnState).toBe('review')

    markTabSessionActivity('codex', '/repo/session.jsonl')
    expect(t.turnState).toBe('review')
    expect(t.turnStateSource).toBe('session-jsonl')
  })

  it('does not infer working state from session append alone', () => {
    const t = tab({
      sessionPath: '/repo/session.jsonl',
      sessionId: 'session-1',
      turnState: 'unknown',
    })
    tabs.value = [t]

    markTabSessionActivity('codex', '/repo/session.jsonl')
    expect(t.turnState).toBe('unknown')

    markTabTurnStarted('codex', '/repo/session.jsonl')
    expect(t.turnState).toBe('working')
    expect(t.turnStateSource).toBe('session-jsonl')
  })

  it('keeps process exit separate from turn completion', () => {
    const t = tab({
      sessionPath: '/repo/session.jsonl',
      sessionId: 'session-1',
      processState: 'exited',
      turnState: 'unknown',
      status: 'exited',
    })
    tabs.value = [t]

    markTabTurnStarted('codex', '/repo/session.jsonl')
    expect(t.turnState).toBe('unknown')

    const wrapper = factory()
    const item = wrapper.findAll('.term-tab').slice(-1)[0]
    expect(item.classes()).toContain('state-exited')
    expect(item.classes()).not.toContain('state-done')
  })

  it('syncs a newly created tab to the matched session title', () => {
    const t = tab({ createdAt: 10_000 })
    tabs.value = [t]

    reconcileNewTabs('proj', [
      {
        path: '/repo/session.jsonl',
        id: 'session-1',
        modified: 12_000,
        title: 'Investigate auth bug',
      },
    ], 'codex')

    expect(t.sessionPath).toBe('/repo/session.jsonl')
    expect(t.sessionId).toBe('session-1')
    expect(t.title).toBe('Investigate auth bug')
  })

  it('applies pending turn completion after a new tab is reconciled to its session', () => {
    const t = tab({ createdAt: 10_000 })
    tabs.value = [t]

    markTabTurnCompleted('codex', '/repo/session.jsonl')
    expect(t.turnState).toBe('unknown')

    reconcileNewTabs('proj', [
      {
        path: '/repo/session.jsonl',
        id: 'session-1',
        modified: 12_000,
        title: 'Generated title',
      },
    ], 'codex')

    expect(t.sessionPath).toBe('/repo/session.jsonl')
    expect(t.turnState).toBe('review')
  })

  it('syncs existing tab titles from refreshed sessions', () => {
    const t = tab({
      sessionId: 'session-1',
      sessionPath: '/repo/session.jsonl',
      title: 'New session',
    })
    tabs.value = [t]

    syncTabTitlesFromSessions('codex', 'proj', [
      {
        path: '/repo/session.jsonl',
        id: 'session-1',
        modified: 12_000,
        title: 'Generated title',
      },
    ])

    expect(t.title).toBe('Generated title')
  })
})
