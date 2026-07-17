import { beforeEach, describe, expect, it, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import ChatComposer from '../../src/components/ChatComposer.vue'
import { vTooltip } from '../../src/tooltip'
import { setLang } from '../../src/settings'
import type { ChatSession } from '../../src/chatSessions'

const { claudeRuntimeInfoMock, listProjectFilesMock, openSideChatMock } = vi.hoisted(() => ({
  claudeRuntimeInfoMock: vi.fn().mockResolvedValue({ hasCustomBaseUrl: false }),
  listProjectFilesMock: vi.fn().mockResolvedValue([]),
  openSideChatMock: vi.fn().mockResolvedValue(null),
}))

vi.mock('../../src/api', () => ({
  agentChatSlashCommands: vi.fn().mockResolvedValue([]),
  claudeRuntimeInfo: claudeRuntimeInfoMock,
  codexRuntimeInfo: vi.fn().mockResolvedValue({ usesApiKey: false }),
  listProjectFiles: listProjectFilesMock,
}))

vi.mock('../../src/sideChat', () => ({
  openSideChat: openSideChatMock,
}))

// composer onMounted 注册 Tauri webview 级 drag-drop 监听；jsdom 里没有 Tauri internals，mock 掉。
vi.mock('@tauri-apps/api/webview', () => ({
  getCurrentWebview: () => ({ onDragDropEvent: vi.fn().mockResolvedValue(() => {}) }),
}))

vi.mock('../../src/usage', () => ({
  usage: { value: null },
  usageWindows: vi.fn(() => [
    { key: 'five_hour', usedPct: 0, resetsAt: new Date(Date.now() + 60_000).toISOString() },
    { key: 'seven_day', usedPct: 0, resetsAt: new Date(Date.now() + 120_000).toISOString() },
  ]),
  usageLevel: vi.fn(() => 'ok'),
  formatRemaining: vi.fn(() => ''),
  nowMs: { value: Date.now() },
  startUsagePolling: vi.fn(),
  stopUsagePolling: vi.fn(),
  bumpUsage: vi.fn(),
}))

const baseSession = (over: Partial<ChatSession> = {}): ChatSession =>
  ({
    uiId: 1,
    chatId: 1,
    agent: 'claude',
    projectKey: 'proj',
    cwd: '/work/proj',
    sessionId: 's1',
    title: 'Chat',
    createdAt: new Date().toISOString(),
    msgs: [],
    turnState: 'idle',
    turnStartedAt: 0,
    lastTurnMs: 0,
    status: 'running',
    queue: [],
    usage: undefined,
    lastModel: undefined,
    apiKeySource: 'none',
    errorMessage: undefined,
    stderrTail: [],
    live: null,
    pendingPermissions: [],
    pendingQuestions: [],
    permissionMode: 'acceptEdits',
    model: 'claude-opus-4-8',
    effort: 'high',
    processModel: 'longLivedStdin',
    applied: { permissionMode: 'acceptEdits', model: 'claude-opus-4-8', effort: 'high' },
    ...over,
  }) as ChatSession

describe('ChatComposer', () => {
  it('hides the effort slider for Claude API-key sessions', () => {
    setLang('en')
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession({ apiKeySource: 'ANTHROPIC_API_KEY' }) },
      global: { directives: { tooltip: vTooltip } },
    })
    expect(wrapper.findComponent({ name: 'ChatEffortSlider' }).exists()).toBe(false)
  })

  it('hides effort and rate limits for Claude custom endpoints even when apiKeySource reports none', async () => {
    claudeRuntimeInfoMock.mockResolvedValueOnce({ hasCustomBaseUrl: true })
    setLang('en')
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession({ apiKeySource: 'none' }) },
      global: { directives: { tooltip: vTooltip } },
    })
    await Promise.resolve()
    await wrapper.vm.$nextTick()
    expect(wrapper.findComponent({ name: 'ChatEffortSlider' }).exists()).toBe(false)
    expect(wrapper.text()).not.toContain('5h')
    expect(wrapper.text()).not.toContain('week')
  })

  it('keeps the model picker for Claude API-key sessions so settings.json model mapping can apply', () => {
    setLang('en')
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession({ apiKeySource: 'ANTHROPIC_API_KEY' }) },
      global: { directives: { tooltip: vTooltip } },
    })
    expect(wrapper.findComponent({ name: 'ChatModelMenu' }).exists()).toBe(true)
    expect(wrapper.text()).toContain('Opus')
  })

  it('keeps the effort slider for Claude subscription sessions', () => {
    setLang('en')
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession({ apiKeySource: 'none' }) },
      global: { directives: { tooltip: vTooltip } },
    })
    expect(wrapper.findComponent({ name: 'ChatEffortSlider' }).exists()).toBe(true)
  })

  it('hides the effort slider while Claude apiKeySource is still unknown', () => {
    setLang('en')
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession({ apiKeySource: undefined }) },
      global: { directives: { tooltip: vTooltip } },
    })
    expect(wrapper.findComponent({ name: 'ChatEffortSlider' }).exists()).toBe(false)
  })

  it('hides subscription rate-limit badges until Claude apiKeySource is confirmed as none', () => {
    setLang('en')
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession({ apiKeySource: undefined }) },
      global: { directives: { tooltip: vTooltip } },
    })
    expect(wrapper.text()).not.toContain('5h')
    expect(wrapper.text()).not.toContain('week')
  })

  it('seeds effort slider + rate limits from the runtime guess before init arrives', async () => {
    // runtime_info 预判官方订阅（钥匙串有凭证）→ session.apiKeySource 还没回来也该立刻显示。
    claudeRuntimeInfoMock.mockResolvedValueOnce({ hasCustomBaseUrl: false, apiKeySource: 'none' })
    setLang('en')
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession({ apiKeySource: undefined }) },
      global: { directives: { tooltip: vTooltip } },
    })
    // 预判前：仍是保守态（未知 → 不显示）。
    expect(wrapper.findComponent({ name: 'ChatEffortSlider' }).exists()).toBe(false)
    await Promise.resolve()
    await wrapper.vm.$nextTick()
    // 预判落地后：官方专属元素出现，无需等首轮 init。
    expect(wrapper.findComponent({ name: 'ChatEffortSlider' }).exists()).toBe(true)
    expect(wrapper.text()).toContain('5h')
  })

  it('lets a real init apiKeySource override the runtime guess', async () => {
    // runtime 误判成订阅，但 init 权威地说是 API key → 以 init 为准，隐藏 effort。
    claudeRuntimeInfoMock.mockResolvedValueOnce({ hasCustomBaseUrl: false, apiKeySource: 'none' })
    setLang('en')
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession({ apiKeySource: 'ANTHROPIC_API_KEY' }) },
      global: { directives: { tooltip: vTooltip } },
    })
    await Promise.resolve()
    await wrapper.vm.$nextTick()
    expect(wrapper.findComponent({ name: 'ChatEffortSlider' }).exists()).toBe(false)
  })

  it('auto-selects a standard-context model for a brand-new subscription chat', async () => {
    // 空 model → 后端不带 --model → CLI 回落到 settings 默认（常被映射成 1M 上下文，需额度）
    // → 首条消息直接 1M API Error。进会话该自动选一个标准上下文模型避免它。
    claudeRuntimeInfoMock.mockResolvedValueOnce({ hasCustomBaseUrl: false, apiKeySource: 'none' })
    const session = baseSession({ model: undefined, lastModel: undefined, msgs: [] })
    mount(ChatComposer, {
      props: { session },
      global: { directives: { tooltip: vTooltip } },
    })
    expect(session.model).toBeUndefined() // runtime info 就位前先不乱选
    await flushPromises()
    expect(session.model).toBe('claude-opus-4-8') // 标准上下文，不是会触发 1M 报错的默认别名
  })

  it('auto-selects the alias model for a brand-new API-key chat so settings.json mapping applies', async () => {
    claudeRuntimeInfoMock.mockResolvedValueOnce({ hasCustomBaseUrl: false })
    const session = baseSession({
      model: undefined,
      lastModel: undefined,
      msgs: [],
      apiKeySource: 'ANTHROPIC_API_KEY',
    })
    mount(ChatComposer, {
      props: { session },
      global: { directives: { tooltip: vTooltip } },
    })
    await flushPromises()
    expect(session.model).toBe('opus') // 别名模式选 opus，让 settings.json 的模型映射接管
  })

  it('does not force a model on a chat that already has history', async () => {
    // 续聊（有历史）模型应随历史/lastModel，不该被默认值覆盖。
    claudeRuntimeInfoMock.mockResolvedValueOnce({ hasCustomBaseUrl: false, apiKeySource: 'none' })
    const session = baseSession({
      model: undefined,
      lastModel: undefined,
      msgs: [{ role: 'user', blocks: [{ kind: 'text', text: 'hi' }], ts: 1 } as never],
    })
    mount(ChatComposer, {
      props: { session },
      global: { directives: { tooltip: vTooltip } },
    })
    await flushPromises()
    expect(session.model).toBeUndefined()
  })
})

describe('ChatComposer @ file mention', () => {
  const MENTIONS = [
    { relPath: '.codex', name: '.codex', isDir: true, hasChildren: true },
    { relPath: '.codex/skills/git-push/SKILL.md', name: 'SKILL.md', isDir: false, hasChildren: false },
    { relPath: 'README.md', name: 'README.md', isDir: false, hasChildren: false },
  ]

  // 默认所有列举都返回 MENTIONS —— 避免上一测试遗留的防抖定时器在下一测试触发时
  // 抢消费 mockResolvedValueOnce 队列导致的串扰（曾让本组最后一例偶发失败）。
  beforeEach(() => {
    listProjectFilesMock.mockReset()
    listProjectFilesMock.mockResolvedValue(MENTIONS)
  })

  // 触发 `@` 浮层：手动设值 + 光标（jsdom 下 selectionStart 不可靠），再发 input，
  // 等过去内部 70ms 防抖 + 异步拉取。
  async function typeAt(wrapper: ReturnType<typeof mount>, value: string) {
    const ta = wrapper.find('textarea')
    const el = ta.element as HTMLTextAreaElement
    el.value = value
    el.selectionStart = el.selectionEnd = value.length
    await ta.trigger('input')
    await new Promise((r) => setTimeout(r, 90))
    await flushPromises()
    await wrapper.vm.$nextTick()
  }

  function mountComposer() {
    setLang('en')
    return mount(ChatComposer, {
      props: { session: baseSession() },
      global: { directives: { tooltip: vTooltip } },
    })
  }

  it('opens the mention popup listing project entries when typing @', async () => {
    const wrapper = mountComposer()
    await typeAt(wrapper, '@')
    expect(listProjectFilesMock).toHaveBeenCalledWith('/work/proj', '', expect.any(Number))
    const popup = wrapper.find('.cc-mention')
    expect(popup.exists()).toBe(true)
    // 目录带尾斜杠展示，文件原名。
    expect(popup.text()).toContain('.codex/')
    expect(popup.text()).toContain('README.md')
  })

  it('shows a breadcrumb header with the live directory path', async () => {
    const wrapper = mountComposer()
    await typeAt(wrapper, '@')
    // 顶层：只有项目名（cwd 末段 proj）。
    expect(wrapper.find('.cc-mention-path').text()).toBe('proj/')
    // 钻入 .codex 后实时更新到 proj/.codex/。
    await wrapper.find('.cc-mention-item .cc-mention-open').trigger('click')
    await wrapper.vm.$nextTick()
    expect(wrapper.find('.cc-mention-path').text()).toBe('proj/.codex/')
    wrapper.unmount()
  })

  it('attaches a file as a chip and strips the @token on click', async () => {
    const wrapper = mountComposer()
    await typeAt(wrapper, 'see @')
    const rows = wrapper.findAll('.cc-mention-item')
    // 第 3 项是 README.md（文件）。
    await rows[2].trigger('click')
    await wrapper.vm.$nextTick()
    const chip = wrapper.find('.cc-file-chip')
    expect(chip.exists()).toBe(true)
    expect(chip.text()).toContain('README.md')
    // 浮层关闭、`@token` 被抹掉（正文只剩用户写的话）。
    expect(wrapper.find('.cc-mention').exists()).toBe(false)
    expect((wrapper.find('textarea').element as HTMLTextAreaElement).value).toBe('see ')
  })

  it('hides the drill chevron for an empty directory (no children)', async () => {
    listProjectFilesMock.mockReset()
    listProjectFilesMock.mockResolvedValue([
      { relPath: '.tmp', name: '.tmp', isDir: true, hasChildren: false },
    ])
    const wrapper = mountComposer()
    await typeAt(wrapper, '@')
    expect(wrapper.find('.cc-mention-item').exists()).toBe(true)
    // 空目录没有下级 → 不渲染 chevron，提示里也不出现「open」。
    expect(wrapper.find('.cc-mention-item .cc-mention-open').exists()).toBe(false)
    expect(wrapper.find('.cc-mention-hint').text()).not.toContain('open')
    wrapper.unmount()
  })

  it('drills into a directory via the chevron (token becomes @dir/)', async () => {
    const wrapper = mountComposer()
    await typeAt(wrapper, '@')
    const chevron = wrapper.find('.cc-mention-item .cc-mention-open')
    expect(chevron.exists()).toBe(true)
    await chevron.trigger('click')
    await wrapper.vm.$nextTick()
    expect((wrapper.find('textarea').element as HTMLTextAreaElement).value).toBe('@.codex/')
    wrapper.unmount()
  })

  it('returns to the parent level with ArrowLeft after drilling in', async () => {
    const wrapper = mountComposer()
    await typeAt(wrapper, '@')
    await wrapper.find('.cc-mention-item .cc-mention-open').trigger('click') // 钻入 .codex
    await wrapper.vm.$nextTick()
    const ta = wrapper.find('textarea')
    const el = ta.element as HTMLTextAreaElement
    expect(el.value).toBe('@.codex/')
    // 光标置于 token 末尾后按 ← → 逐级返回到顶层。
    el.selectionStart = el.selectionEnd = el.value.length
    await ta.trigger('keydown', { key: 'ArrowLeft' })
    await wrapper.vm.$nextTick()
    expect((ta.element as HTMLTextAreaElement).value).toBe('@')
    wrapper.unmount()
  })

  it('attaches a folder chip when Enter is pressed on a directory row', async () => {
    const wrapper = mountComposer()
    await typeAt(wrapper, '@')
    // 高亮默认第 0 项（.codex 目录）；Enter 引用为目录 chip，而非提交消息。
    await wrapper.find('textarea').trigger('keydown', { key: 'Enter' })
    await wrapper.vm.$nextTick()
    const chip = wrapper.find('.cc-file-chip')
    expect(chip.exists()).toBe(true)
    expect(chip.text()).toContain('.codex')
    expect(wrapper.find('.cc-mention').exists()).toBe(false)
  })
})

describe('ChatComposer /btw side chat', () => {
  beforeEach(() => {
    openSideChatMock.mockClear()
    setLang('en')
  })

  async function submitText(wrapper: ReturnType<typeof mount>, value: string) {
    const ta = wrapper.find('textarea')
    const el = ta.element as HTMLTextAreaElement
    el.value = value
    el.selectionStart = el.selectionEnd = value.length
    await ta.trigger('input')
    await ta.trigger('keydown', { key: 'Enter' })
    await flushPromises()
  }

  it('routes "/btw <prompt>" to the side chat (forking the main session) instead of the main chat', async () => {
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession() },
      global: { directives: { tooltip: vTooltip } },
    })
    await submitText(wrapper, '/btw what does foo do?')
    expect(openSideChatMock).toHaveBeenCalledTimes(1)
    expect(openSideChatMock).toHaveBeenCalledWith(
      expect.objectContaining({
        cwd: '/work/proj',
        forkSessionId: 's1',
        prompt: 'what does foo do?',
      }),
    )
    // 转给侧聊后清空主输入框，且不在主聊里回显该消息。
    expect((wrapper.find('textarea').element as HTMLTextAreaElement).value).toBe('')
    expect(wrapper.props('session').msgs.length).toBe(0)
  })

  it('opens an empty side chat for a bare "/btw"', async () => {
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession() },
      global: { directives: { tooltip: vTooltip } },
    })
    await submitText(wrapper, '/btw')
    expect(openSideChatMock).toHaveBeenCalledWith(
      expect.objectContaining({ prompt: undefined }),
    )
  })

  it('shows a btw button for Claude sessions that opens the side chat', async () => {
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession() },
      global: { directives: { tooltip: vTooltip } },
    })
    const btn = wrapper.find('.cc-btw-btn')
    expect(btn.exists()).toBe(true)
    await btn.trigger('click')
    expect(openSideChatMock).toHaveBeenCalledTimes(1)
  })

  it('recalls prior user messages with ↑/↓ and shows the History hint', async () => {
    setLang('en')
    const userMsg = (s: string) => ({
      role: 'user' as const,
      sidechain: false,
      blocks: [{ kind: 'text' as const, text: s, isError: false }],
    })
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession({ msgs: [userMsg('first'), userMsg('second')] }) },
      global: { directives: { tooltip: vTooltip } },
    })
    const ta = wrapper.find('textarea')
    const el = ta.element as HTMLTextAreaElement

    // ↑ 从最新一条开始回填
    await ta.trigger('keydown', { key: 'ArrowUp' })
    await wrapper.vm.$nextTick()
    expect(el.value).toBe('second')
    expect(wrapper.text()).toContain('History 2/2')

    // 再 ↑ 翻到更旧一条
    await ta.trigger('keydown', { key: 'ArrowUp' })
    await wrapper.vm.$nextTick()
    expect(el.value).toBe('first')
    expect(wrapper.text()).toContain('History 1/2')

    // ↓ 翻回更新一条
    await ta.trigger('keydown', { key: 'ArrowDown' })
    await wrapper.vm.$nextTick()
    expect(el.value).toBe('second')

    // ↓ 越过最新 → 还原空草稿，提示消失
    await ta.trigger('keydown', { key: 'ArrowDown' })
    await wrapper.vm.$nextTick()
    expect(el.value).toBe('')
    expect(wrapper.text()).not.toContain('History')
  })

  it('keeps cycling history when a recalled entry is a slash command (no popup hijack)', async () => {
    setLang('en')
    const userMsg = (s: string) => ({
      role: 'user' as const,
      sidechain: false,
      blocks: [{ kind: 'text' as const, text: s, isError: false }],
    })
    // 最新一条是 `/context`（一个会被 slash 浮层识别的内置命令）。
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession({ msgs: [userMsg('older'), userMsg('/context')] }) },
      global: { directives: { tooltip: vTooltip } },
    })
    // 等内置 slash 列表（含 `context`）加载完，否则浮层根本不会开，测不到劫持。
    await flushPromises()
    const ta = wrapper.find('textarea')
    const el = ta.element as HTMLTextAreaElement

    // ↑ 回填最新一条 `/context`
    await ta.trigger('keydown', { key: 'ArrowUp' })
    await wrapper.vm.$nextTick()
    expect(el.value).toBe('/context')
    expect(wrapper.text()).toContain('History 2/2')

    // 方向键松开会触发 onCaretMove —— 历史浏览态下不该弹出 slash 浮层（否则 ↑/↓ 被它抢走）。
    await ta.trigger('keyup', { key: 'ArrowUp' })
    await wrapper.vm.$nextTick()
    expect(wrapper.find('.cc-slash').exists()).toBe(false)

    // 再按 ↑ 仍能翻到更旧一条，而不是去选浮层菜单。
    await ta.trigger('keydown', { key: 'ArrowUp' })
    await wrapper.vm.$nextTick()
    expect(el.value).toBe('older')
    expect(wrapper.text()).toContain('History 1/2')
  })

  it('does not hijack ↑ when there is no message history', async () => {
    const wrapper = mount(ChatComposer, {
      props: { session: baseSession({ msgs: [] }) },
      global: { directives: { tooltip: vTooltip } },
    })
    const ta = wrapper.find('textarea')
    await ta.trigger('keydown', { key: 'ArrowUp' })
    await wrapper.vm.$nextTick()
    expect((ta.element as HTMLTextAreaElement).value).toBe('')
    expect(wrapper.text()).not.toContain('History')
  })
})

describe('ChatComposer client slash commands', () => {
  beforeEach(() => setLang('en'))

  async function submitText(wrapper: ReturnType<typeof mount>, value: string) {
    const ta = wrapper.find('textarea')
    const el = ta.element as HTMLTextAreaElement
    el.value = value
    el.selectionStart = el.selectionEnd = value.length
    await ta.trigger('input')
    await ta.trigger('keydown', { key: 'Enter' })
    await flushPromises()
  }

  const mountComposer = (over = {}) =>
    mount(ChatComposer, {
      props: { session: baseSession(over) },
      global: { directives: { tooltip: vTooltip } },
    })

  it('emits "openExport" for "/export" without echoing into the chat', async () => {
    const wrapper = mountComposer()
    await submitText(wrapper, '/export')
    expect(wrapper.emitted('openExport')).toHaveLength(1)
    expect((wrapper.find('textarea').element as HTMLTextAreaElement).value).toBe('')
    expect(wrapper.props('session').msgs.length).toBe(0)
  })

  it('emits "rename" for "/rename"', async () => {
    const wrapper = mountComposer()
    await submitText(wrapper, '/rename')
    expect(wrapper.emitted('rename')).toHaveLength(1)
    expect(wrapper.props('session').msgs.length).toBe(0)
  })

  it('emits "fork" for "/fork" on a Claude session', async () => {
    const wrapper = mountComposer()
    await submitText(wrapper, '/fork')
    expect(wrapper.emitted('fork')).toHaveLength(1)
    expect(wrapper.props('session').msgs.length).toBe(0)
  })

  it('does NOT intercept "/fork" on a non-Claude session (sends normally)', async () => {
    const wrapper = mountComposer({ agent: 'codex', processModel: 'oneShotResume' })
    await submitText(wrapper, '/fork')
    expect(wrapper.emitted('fork')).toBeUndefined()
    // 落回普通发送：本地回显成一条用户消息（agentChatSend 在测试里未 mock，发送失败被吞）。
    expect(wrapper.props('session').msgs.length).toBe(1)
  })

  it('clears the visible messages for "/clear"', async () => {
    const userMsg = {
      role: 'user' as const,
      sidechain: false,
      blocks: [{ kind: 'text' as const, text: 'hi', isError: false }],
    }
    const wrapper = mountComposer({ msgs: [userMsg] })
    expect(wrapper.props('session').msgs.length).toBe(1)
    await submitText(wrapper, '/clear')
    expect(wrapper.props('session').msgs.length).toBe(0)
    expect((wrapper.find('textarea').element as HTMLTextAreaElement).value).toBe('')
  })

  it('expands the model panel for "/model" and does not send', async () => {
    const wrapper = mountComposer()
    expect(wrapper.find('.mm-menu').exists()).toBe(false)
    await submitText(wrapper, '/model')
    expect(wrapper.find('.mm-menu').exists()).toBe(true)
    expect(wrapper.props('session').msgs.length).toBe(0)
  })

  it('lists a "System" group in the "/" popup', async () => {
    const wrapper = mountComposer()
    await flushPromises() // loadSlashCommands 异步 resolve 后 system 指令才入列
    const ta = wrapper.find('textarea')
    const el = ta.element as HTMLTextAreaElement
    el.value = '/'
    el.selectionStart = el.selectionEnd = 1
    await ta.trigger('input')
    await flushPromises()
    expect(wrapper.find('.cc-slash').exists()).toBe(true)
    const text = wrapper.text()
    expect(text).toContain('System')
    expect(text).toContain('/export')
    expect(text).toContain('/model')
    expect(text).toContain('/clear')
  })
})
