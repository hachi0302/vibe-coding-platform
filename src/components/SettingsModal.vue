<script setup lang="ts">
import { ref, computed, watch, onMounted, onUnmounted } from 'vue'
import type { Agent } from '../types'
import { t } from '../i18n'
import {
  codexShowArchivedSessions,
  codexShowInternalSessions,
  lang,
  setCodexShowArchivedSessions,
  setCodexShowInternalSessions,
  setLang,
  setTheme,
  setFontScale,
  applyFontScale,
  fontFamily,
  setFontFamily,
  applyFontFamily,
  setUseExternalTerminal,
  setAutoRestoreTerminalTabs,
  setTerminalApp,
  applyTerminalDefault,
  launchArgs,
  setLaunchArgs,
  theme,
  fontScale,
  useExternalTerminal,
  autoRestoreTerminalTabs,
  terminalApp,
  enabledAgents,
  visibleAgents,
  setAgentEnabled,
  ALL_AGENTS,
  quickOpenTarget,
  setQuickOpenTarget,
  useReclaude,
  setUseReclaude,
  type Lang,
  type Theme,
  type TerminalApp,
  type QuickOpenTarget,
} from '../settings'
import { formatSize } from '../format'
import {
  IconClose,
  IconRefresh,
  IconExternalLink,
  IconCheck,
  IconChevronDown,
  IconSettings,
  IconSliders,
  IconKeyboard,
  IconDownload,
  IconTerminal,
  agentIcons,
  terminalIcons,
} from './icons'
import CliEnvironmentCheck from './CliEnvironmentCheck.vue'
import * as api from '../api'
import {
  checkAppUpdate,
  downloadAndInstallUpdate,
  latestVersion,
  openReleasePage,
  relaunchApp,
  updateDownloaded,
  updateDownloading,
  updateInstallError,
  updateProgress,
  updateAvailable,
  updaterUpdate,
} from '../updateCheck'

type SettingsTab = 'general' | 'advanced' | 'cli' | 'shortcuts' | 'updates'

// 左侧导航：图标 + 文案，激活项高亮（参考 Claude 客户端设置面板）。
const navItems = [
  { id: 'general', icon: IconSettings, key: 'settings.tab.general' },
  { id: 'advanced', icon: IconSliders, key: 'settings.tab.advanced' },
  { id: 'cli', icon: IconTerminal, key: 'settings.tab.cli' },
  { id: 'shortcuts', icon: IconKeyboard, key: 'settings.tab.shortcuts' },
  { id: 'updates', icon: IconDownload, key: 'settings.tab.updates' },
] as const

const isMac = /Mac/i.test(navigator.platform)
const mod = isMac ? '⌘' : 'Ctrl'
const shift = isMac ? '⇧' : 'Shift'
const opt = isMac ? '⌥' : 'Alt'
const sep = isMac ? '' : '+'
const k = (parts: string[]) => parts.join(sep)
// 分两组展示：全局（应用级，随处可用）/ 会话（作用于当前会话或其 tab）。
const shortcutGroups = [
  {
    title: 'settings.shortcut.groupGlobal',
    items: [
      { key: k([mod, shift, 'F']), label: 'settings.shortcut.globalSearch' },
      { key: k([mod, 'N']), label: 'settings.shortcut.newSession' },
      { key: k([mod, 'T']), label: 'settings.shortcut.newTab' },
      { key: k([mod, 'O']), label: 'settings.shortcut.addFolder' },
      { key: k([mod, 'B']), label: 'settings.shortcut.toggleSidebar' },
      { key: k([mod, shift, 'S']), label: 'settings.shortcut.stats' },
      { key: k([mod, shift, 'T']), label: 'settings.shortcut.trash' },
      { key: k([mod, ',']), label: 'settings.shortcut.settings' },
      { key: k([mod, '/']), label: 'settings.shortcut.shortcuts' },
      { key: 'Esc', label: 'settings.shortcut.escape' },
    ],
  },
  {
    title: 'settings.shortcut.groupSession',
    items: [
      { key: k([mod, 'F']), label: 'settings.shortcut.findInSession' },
      { key: k([mod, 'G']), label: 'settings.shortcut.findNext' },
      { key: k([mod, shift, 'G']), label: 'settings.shortcut.findPrev' },
      { key: k([mod, 'W']), label: 'settings.shortcut.closeTab' },
      { key: k([mod, 'R']), label: 'settings.shortcut.renameTab' },
      { key: k([mod, 'E']), label: 'settings.shortcut.exportSession' },
    ],
  },
  {
    title: 'settings.shortcut.groupChat',
    items: [
      { key: k([mod, 'U']), label: 'settings.shortcut.attachFiles' },
      { key: k([mod, 'J']), label: 'settings.shortcut.btwSideChat' },
      { key: 'Ctrl+S', label: 'settings.shortcut.stashInput' },
      { key: 'Ctrl+Del', label: 'settings.shortcut.deleteLine' },
      { key: 'Shift+Enter', label: 'settings.shortcut.newline' },
    ],
  },
  {
    title: 'settings.shortcut.groupPanes',
    items: [
      { key: k([mod, 'D']), label: 'settings.shortcut.splitRight' },
      { key: k([mod, shift, 'D']), label: 'settings.shortcut.splitDown' },
      { key: k([mod, shift, 'W']), label: 'settings.shortcut.closePane' },
      { key: `${k([mod, opt])} ←↑↓→`, label: 'settings.shortcut.focusPane' },
    ],
  },
]

const agentLabel = (a: Agent) =>
  a === 'codex' ? 'Codex' : a === 'agy' ? 'Antigravity CLI' : a === 'opencode' ? 'opencode' : 'Claude'

const props = defineProps<{ cacheBytes: number; initialTab?: SettingsTab }>()
const emit = defineEmits<{ close: []; clearCache: []; clearTabs: [] }>()

const activeTab = ref<SettingsTab>(props.initialTab ?? 'general')
// 切换左侧导航时，右侧内容回到顶部（否则会沿用上一个 tab 的滚动位置）。
const bodyEl = ref<HTMLElement>()
watch(activeTab, () => {
  if (bodyEl.value) bodyEl.value.scrollTop = 0
})

const cacheLabel = computed(() =>
  props.cacheBytes > 0 ? formatSize(props.cacheBytes) : '0 B',
)

const version = ref('—')
const updateMsg = ref('')
const checking = ref(false)
const installingClaudeHooks = ref(false)
const claudeHooksMsg = ref('')

const reclaudeInstalled = ref(false)
const reclaudeRunning = ref(false)

// custom dropdown state
const langMenuOpen = ref(false)
const themeMenuOpen = ref(false)
const terminalMenuOpen = ref(false)
const langWrapEl = ref<HTMLElement>()
const themeWrapEl = ref<HTMLElement>()
const terminalWrapEl = ref<HTMLElement>()

const isMacOS = /Mac/i.test(navigator.platform)
const availableTerminals = ref<string[]>([])
type TermOpt = { v: TerminalApp; key: string }
const terminalOptions = computed<TermOpt[]>(() => {
  const base: TermOpt[] = [{ v: 'terminal', key: 'settings.terminalApp.terminal' }]
  if (availableTerminals.value.includes('cmux'))
    base.push({ v: 'cmux', key: 'settings.terminalApp.cmux' })
  if (availableTerminals.value.includes('iterm2'))
    base.push({ v: 'iterm2', key: 'settings.terminalApp.iterm2' })
  if (availableTerminals.value.includes('ghostty'))
    base.push({ v: 'ghostty', key: 'settings.terminalApp.ghostty' })
  if (availableTerminals.value.includes('warp'))
    base.push({ v: 'warp', key: 'settings.terminalApp.warp' })
  return base
})
const currentTerminalLabel = computed(() => {
  const o = terminalOptions.value.find(o => o.v === terminalApp.value)
  return o ? t(o.key) : terminalApp.value
})

function pickLang(v: Lang) {
  setLang(v)
  langMenuOpen.value = false
}
function pickTheme(v: Theme) {
  setTheme(v)
  themeMenuOpen.value = false
}
function pickTerminal(v: TerminalApp) {
  setTerminalApp(v)
  terminalMenuOpen.value = false
}
function onDocClick(e: MouseEvent) {
  if (langMenuOpen.value && langWrapEl.value && !langWrapEl.value.contains(e.target as Node))
    langMenuOpen.value = false
  if (themeMenuOpen.value && themeWrapEl.value && !themeWrapEl.value.contains(e.target as Node))
    themeMenuOpen.value = false
  if (terminalMenuOpen.value && terminalWrapEl.value && !terminalWrapEl.value.contains(e.target as Node))
    terminalMenuOpen.value = false
}
onMounted(() => document.addEventListener('click', onDocClick, true))
onUnmounted(() => {
  document.removeEventListener('click', onDocClick, true)
  applyFontScale()
  applyFontFamily()
})

onMounted(async () => {
  try {
    version.value = await api.appVersion()
  } catch {
    /* ignore */
  }
  if (isMacOS) {
    try {
      const detected = await api.detectTerminals()
      availableTerminals.value = detected
      applyTerminalDefault(detected)
    } catch {
      /* ignore */
    }
  }
  if (updateAvailable.value && latestVersion.value) {
    updateMsg.value = t('settings.updateAvailable', {
      v: latestVersion.value,
      cur: version.value,
    })
  }
  try {
    const info = await api.reclaudeInfo()
    reclaudeInstalled.value = info.installed
    reclaudeRunning.value = info.daemonRunning
  } catch {
    /* ignore */
  }
})

const langOptions: { v: Lang; key: string }[] = [
  { v: 'en', key: 'settings.lang.en' },
  { v: 'zh', key: 'settings.lang.zh' },
  { v: 'zh-TW', key: 'settings.lang.zhTw' },
  { v: 'ja', key: 'settings.lang.ja' },
]
type ThemeOpt = { v: Theme; key: string }
const themeOptions: ThemeOpt[] = [
  { v: 'light', key: 'settings.theme.light' },
  { v: 'dark', key: 'settings.theme.dark' },
  { v: 'system', key: 'settings.theme.system' },
  { v: 'codex', key: 'settings.theme.codex' },
  { v: 'dracula', key: 'settings.theme.dracula' },
]

function onFontSlider(e: Event) {
  setFontScale(Number((e.target as HTMLInputElement).value))
}

function onFontFamilyInput(e: Event) {
  setFontFamily((e.target as HTMLInputElement).value.trim())
}

type QuickOpenOpt = { v: QuickOpenTarget; key: string }
const quickOpenOptions: QuickOpenOpt[] = [
  { v: 'session', key: 'settings.quickOpen.session' },
  { v: 'terminal', key: 'settings.quickOpen.terminal' },
  { v: 'chat', key: 'settings.quickOpen.chat' },
]

const currentLangLabel = computed(() => {
  const o = langOptions.find(o => o.v === lang.value)
  return o ? t(o.key) : lang.value
})
const currentThemeLabel = computed(() => {
  const o = themeOptions.find(o => o.v === theme.value)
  return o ? t(o.key) : theme.value
})

async function doCheck() {
  if (checking.value) return
  checking.value = true
  updateMsg.value = t('settings.checking')
  try {
    const r = await checkAppUpdate()
    updateMsg.value = r.hasUpdate
      ? t('settings.updateAvailable', { v: r.latest, cur: r.current })
      : t('settings.upToDate', { v: r.current })
  } catch (e) {
    updateMsg.value = t('settings.updateFail', { e: String(e) })
  } finally {
    checking.value = false
  }
}

async function installUpdate() {
  if (updateDownloading.value) return
  updateMsg.value = t('settings.updateDownloading')
  try {
    await downloadAndInstallUpdate()
    updateMsg.value = t('settings.updateReady')
  } catch (e) {
    updateInstallError.value = String(e)
    updateMsg.value = ''
  }
}

async function installClaudeHooks() {
  if (installingClaudeHooks.value) return
  installingClaudeHooks.value = true
  claudeHooksMsg.value = t('settings.turnStatus.installing')
  try {
    const path = await api.installClaudeTurnHooks()
    claudeHooksMsg.value = t('settings.turnStatus.installed', { path })
  } catch (e) {
    claudeHooksMsg.value = t('settings.turnStatus.installFail', { e: String(e) })
  } finally {
    installingClaudeHooks.value = false
  }
}
</script>

<template>
  <div class="overlay">
    <div class="modal settings-modal">
      <!-- 左侧导航：分组标题 + 图标项，激活项高亮（参考 Claude 客户端设置面板） -->
      <nav class="set-nav">
        <div class="set-nav-group-label">{{ t('settings.title') }}</div>
        <button
          v-for="n in navItems"
          :key="n.id"
          class="set-nav-item"
          :class="{ active: activeTab === n.id }"
          @click="activeTab = n.id"
        >
          <component :is="n.icon" class="set-nav-icon" />
          <span>{{ t(n.key) }}</span>
          <span v-if="n.id === 'updates' && updateAvailable" class="set-nav-dot" aria-hidden="true" />
        </button>
        <!-- 左栏底部：当前 app 版本号（margin-top:auto 顶到底） -->
        <div class="set-nav-version">v{{ version }}</div>
      </nav>

      <button
        class="modal-close"
        v-tooltip="t('common.close')"
        @click="emit('close')"
      >
        <IconClose />
      </button>

      <div ref="bodyEl" class="set-body">
        <template v-if="activeTab === 'general'">
          <!-- 外观：语言 / 主题 / 字号 —— 单控件行，标题在左、控件在右 -->
          <div class="set-group">
            <div class="set-row">
              <div class="set-row-text">
                <div class="set-row-title">{{ t('settings.section.lang') }}</div>
              </div>
              <div ref="langWrapEl" class="set-dropdown-wrap set-row-control">
                <button
                  class="set-dropdown-btn"
                  :class="{ active: langMenuOpen }"
                  @click.stop="langMenuOpen = !langMenuOpen; themeMenuOpen = false"
                >
                  <span>{{ currentLangLabel }}</span>
                  <IconChevronDown class="set-dropdown-chev" />
                </button>
                <div v-if="langMenuOpen" class="set-dropdown-menu" role="menu">
                  <button
                    v-for="o in langOptions"
                    :key="o.v"
                    class="set-dropdown-item"
                    :class="{ active: lang === o.v }"
                    role="menuitem"
                    @click.stop="pickLang(o.v)"
                  >
                    <span class="set-dropdown-check"><IconCheck v-if="lang === o.v" /></span>
                    <span>{{ t(o.key) }}</span>
                  </button>
                </div>
              </div>
            </div>

            <div class="set-row">
              <div class="set-row-text">
                <div class="set-row-title">{{ t('settings.section.theme') }}</div>
              </div>
              <div ref="themeWrapEl" class="set-dropdown-wrap set-row-control">
                <button
                  class="set-dropdown-btn"
                  :class="{ active: themeMenuOpen }"
                  @click.stop="themeMenuOpen = !themeMenuOpen; langMenuOpen = false"
                >
                  <span class="theme-swatch theme-swatch-sm" :class="`theme-swatch-${theme}`">Aa</span>
                  <span>{{ currentThemeLabel }}</span>
                  <IconChevronDown class="set-dropdown-chev" />
                </button>
                <div v-if="themeMenuOpen" class="set-dropdown-menu" role="menu">
                  <button
                    v-for="o in themeOptions"
                    :key="o.v"
                    class="set-dropdown-item"
                    :class="{ active: theme === o.v }"
                    role="menuitem"
                    @click.stop="pickTheme(o.v)"
                  >
                    <span class="set-dropdown-check"><IconCheck v-if="theme === o.v" /></span>
                    <span class="theme-swatch theme-swatch-sm" :class="`theme-swatch-${o.v}`">Aa</span>
                    <span>{{ t(o.key) }}</span>
                  </button>
                </div>
              </div>
            </div>

            <div class="set-row">
              <div class="set-row-text">
                <div class="set-row-title">{{ t('settings.section.fontSize') }}</div>
              </div>
              <div class="set-font-slider set-row-control">
                <span class="set-font-label set-font-label-sm">A</span>
                <input
                  type="range" min="12" max="18" step="1"
                  :value="fontScale"
                  @input="onFontSlider"
                  class="set-slider"
                >
                <span class="set-font-label set-font-label-lg">A</span>
                <span class="set-font-value">{{ fontScale }}px</span>
              </div>
            </div>
            <div class="set-font-preview" :style="{ fontSize: fontScale + 'px' }">
              {{ t('settings.fontPreview') }}
            </div>

            <div class="set-row">
              <div class="set-row-text">
                <div class="set-row-title">{{ t('settings.section.fontFamily') }}</div>
              </div>
              <div class="set-row-control">
                <input
                  type="text"
                  class="set-input"
                  :value="fontFamily"
                  :placeholder="t('settings.fontFamilyPlaceholder')"
                  @input="onFontFamilyInput"
                >
              </div>
            </div>
            <div class="set-font-preview" :style="{ fontSize: fontScale + 'px', fontFamily: fontFamily || undefined }">
              {{ t('settings.fontPreview') }}
            </div>
          </div>

          <!-- Agents 显隐 —— 分组标题 + desc 直接显示，下面是每个 agent 的开关 -->
          <div class="set-group">
            <div class="set-group-head">
              <div class="set-group-title">{{ t('settings.section.agents') }}</div>
              <p class="set-group-desc">{{ t('settings.agentsVisibilityDesc') }}</p>
            </div>
            <label
              v-for="a in ALL_AGENTS"
              :key="a"
              class="set-row set-row-clickable"
              :class="{ disabled: enabledAgents[a] && visibleAgents.length === 1 }"
              @click.prevent="setAgentEnabled(a, !enabledAgents[a])"
            >
              <div class="set-row-text">
                <div class="set-row-title set-row-title-icon">
                  <component :is="agentIcons[a]" class="set-agent-toggle-icon" />
                  {{ agentLabel(a) }}
                </div>
              </div>
              <span class="set-toggle-track set-row-control" :class="{ on: enabledAgents[a] }">
                <span class="set-toggle-thumb" />
              </span>
            </label>
          </div>

          <!-- 数据 -->
          <div class="set-group">
            <div class="set-row">
              <div class="set-row-text">
                <div class="set-row-title">
                  {{ t('settings.section.data') }}
                  <span class="set-section-tail">{{ cacheLabel }}</span>
                </div>
                <p class="set-row-desc">{{ t('settings.clearCacheDesc') }}</p>
              </div>
              <button class="btn danger set-row-control" :disabled="false" @click="emit('clearCache')">
                {{ t('settings.clearCache') }}
              </button>
            </div>

            <div class="set-row">
              <div class="set-row-text">
                <div class="set-row-title">{{ t('settings.section.tabs') }}</div>
                <p class="set-row-desc">{{ t('settings.clearTabsDesc') }}</p>
              </div>
              <button class="btn danger set-row-control" @click="emit('clearTabs')">
                {{ t('settings.clearTabs') }}
              </button>
            </div>
          </div>
        </template>

        <template v-else-if="activeTab === 'advanced'">
          <!-- 终端 -->
          <div class="set-group">
            <div class="set-group-head">
              <div class="set-group-title">{{ t('settings.section.terminal') }}</div>
            </div>
            <label class="set-row set-row-clickable" @click.prevent="setAutoRestoreTerminalTabs(!autoRestoreTerminalTabs)">
              <div class="set-row-text">
                <div class="set-row-title">{{ t('settings.autoRestoreTerminalTabs') }}</div>
                <p class="set-row-desc">{{ t('settings.autoRestoreTerminalTabsDesc') }}</p>
              </div>
              <span class="set-toggle-track set-row-control" :class="{ on: autoRestoreTerminalTabs }">
                <span class="set-toggle-thumb" />
              </span>
            </label>

            <label class="set-row set-row-clickable" @click.prevent="setUseExternalTerminal(!useExternalTerminal)">
              <div class="set-row-text">
                <div class="set-row-title">{{ t('settings.useExternalTerminal') }}</div>
                <p class="set-row-desc">{{ t('settings.terminalDesc') }}</p>
              </div>
              <span class="set-toggle-track set-row-control" :class="{ on: useExternalTerminal }">
                <span class="set-toggle-thumb" />
              </span>
            </label>

            <div v-if="useExternalTerminal && isMacOS && terminalOptions.length > 1" class="set-row set-row-nosep">
              <div class="set-row-text">
                <div class="set-row-title">{{ t('settings.terminalApp.label') }}</div>
              </div>
              <div ref="terminalWrapEl" class="set-dropdown-wrap set-row-control">
                <button
                  class="set-dropdown-btn"
                  :class="{ active: terminalMenuOpen }"
                  @click.stop="terminalMenuOpen = !terminalMenuOpen; langMenuOpen = false; themeMenuOpen = false"
                >
                  <component :is="terminalIcons[terminalApp]" class="set-terminal-icon" />
                  <span>{{ currentTerminalLabel }}</span>
                  <IconChevronDown class="set-dropdown-chev" />
                </button>
                <div v-if="terminalMenuOpen" class="set-dropdown-menu" role="menu">
                  <button
                    v-for="o in terminalOptions"
                    :key="o.v"
                    class="set-dropdown-item"
                    :class="{ active: terminalApp === o.v }"
                    role="menuitem"
                    @click.stop="pickTerminal(o.v)"
                  >
                    <span class="set-dropdown-check"><IconCheck v-if="terminalApp === o.v" /></span>
                    <component :is="terminalIcons[o.v]" class="set-terminal-icon" />
                    <span>{{ t(o.key) }}</span>
                  </button>
                </div>
              </div>
            </div>
          </div>

          <!-- 双击 / 新建快捷键默认打开什么 -->
          <div class="set-group">
            <div class="set-row">
              <div class="set-row-text">
                <div class="set-row-title">{{ t('settings.section.quickOpen') }}</div>
                <p class="set-row-desc">{{ t('settings.quickOpenDesc') }}</p>
              </div>
              <div class="set-segment set-row-control">
                <button
                  v-for="o in quickOpenOptions"
                  :key="o.v"
                  class="set-segment-btn"
                  :class="{ active: quickOpenTarget === o.v }"
                  @click="setQuickOpenTarget(o.v)"
                >
                  {{ t(o.key) }}
                </button>
              </div>
            </div>
          </div>

          <!-- 启动参数 -->
          <div class="set-group">
            <div class="set-group-head">
              <div class="set-group-title">{{ t('settings.launchArgs') }}</div>
              <p class="set-group-desc">{{ t('settings.launchArgsDesc') }}</p>
            </div>
            <div class="set-launch-args">
              <div class="set-launch-args-row" v-for="a in (['claude', 'codex', 'agy', 'opencode'] as const)" :key="a">
                <component :is="agentIcons[a]" class="set-launch-args-icon" />
                <input
                  class="set-launch-args-input"
                  :value="launchArgs[a]"
                  @input="setLaunchArgs(a, ($event.target as HTMLInputElement).value)"
                  :placeholder="{ claude: '--dangerously-skip-permissions', codex: '--yolo', agy: '--dangerously-skip-permissions', opencode: '--auto' }[a]"
                  spellcheck="false"
                />
                <button
                  v-if="!launchArgs[a]"
                  class="set-launch-args-fill"
                  v-tooltip="t('settings.launchArgsFill')"
                  @click="setLaunchArgs(a, { claude: '--dangerously-skip-permissions', codex: '--yolo', agy: '--dangerously-skip-permissions', opencode: '--auto' }[a])"
                >↵</button>
              </div>
            </div>
          </div>

          <!-- 状态跟踪 -->
          <div class="set-group">
            <div class="set-group-head">
              <div class="set-group-title">{{ t('settings.section.turnStatus') }}</div>
              <p class="set-group-desc">{{ t('settings.turnStatus.desc') }}</p>
            </div>
            <div class="set-update-actions">
              <button
                class="btn"
                :disabled="installingClaudeHooks"
                @click="installClaudeHooks"
              >
                {{ installingClaudeHooks ? t('settings.turnStatus.installing') : t('settings.turnStatus.installClaude') }}
              </button>
            </div>
            <p v-if="claudeHooksMsg" class="set-group-desc set-toggle-hint">{{ claudeHooksMsg }}</p>
          </div>

          <!-- Codex -->
          <div class="set-group">
            <div class="set-group-head">
              <div class="set-group-title">Codex</div>
              <p class="set-group-desc">{{ t('settings.codexVisibilityDesc') }}</p>
            </div>
            <label class="set-row set-row-clickable" @click.prevent="setCodexShowInternalSessions(!codexShowInternalSessions)">
              <div class="set-row-text">
                <div class="set-row-title">{{ t('settings.codex.showInternal') }}</div>
              </div>
              <span class="set-toggle-track set-row-control" :class="{ on: codexShowInternalSessions }">
                <span class="set-toggle-thumb" />
              </span>
            </label>
            <label class="set-row set-row-clickable" @click.prevent="setCodexShowArchivedSessions(!codexShowArchivedSessions)">
              <div class="set-row-text">
                <div class="set-row-title">{{ t('settings.codex.showArchived') }}</div>
              </div>
              <span class="set-toggle-track set-row-control" :class="{ on: codexShowArchivedSessions }">
                <span class="set-toggle-thumb" />
              </span>
            </label>
          </div>

          <!-- ReClaude -->
          <div v-if="reclaudeInstalled" class="set-group">
            <div class="set-group-head">
              <div class="set-group-title">{{ t('settings.section.reclaude') }}</div>
              <p class="set-group-desc">{{ t('settings.reclaude.desc') }}</p>
            </div>
            <label class="set-row set-row-clickable" @click.prevent="setUseReclaude(!useReclaude)">
              <div class="set-row-text">
                <div class="set-row-title">{{ t('settings.reclaude.toggle') }}</div>
                <p v-if="useReclaude && !reclaudeRunning" class="set-row-desc" style="color:var(--danger)">{{ t('settings.reclaude.notRunning') }}</p>
              </div>
              <span class="set-toggle-track set-row-control" :class="{ on: useReclaude }">
                <span class="set-toggle-thumb" />
              </span>
            </label>
          </div>

        </template>

        <template v-else-if="activeTab === 'cli'">
          <CliEnvironmentCheck />
        </template>

        <template v-else-if="activeTab === 'updates'">
          <div class="set-group">
            <!-- 版本/更新状态卡片：标题 + 副标题 + 单个主操作按钮，不再堆一排按钮 -->
            <div class="set-update-card" :class="{ available: updateAvailable }">
              <span class="set-update-icon">
                <component :is="updateAvailable ? IconDownload : IconCheck" />
              </span>
              <div class="set-update-info">
                <div class="set-update-title">
                  {{ updateAvailable
                    ? t('settings.update.newVersion', { v: latestVersion ?? '' })
                    : t('settings.update.upToDateShort') }}
                </div>
                <div class="set-update-sub">
                  {{ updateAvailable
                    ? t('settings.update.fromTo', { cur: version, next: latestVersion ?? '' })
                    : t('settings.update.current', { v: version }) }}
                </div>
              </div>
              <div class="set-update-cta">
                <button
                  v-if="updateDownloaded"
                  class="btn primary"
                  @click="relaunchApp()"
                >
                  <IconCheck />
                  {{ t('settings.relaunch') }}
                </button>
                <button
                  v-else-if="updaterUpdate"
                  class="btn primary"
                  :disabled="updateDownloading"
                  @click="installUpdate"
                >
                  <IconRefresh v-if="updateDownloading" />
                  {{ updateDownloading ? t('settings.updateDownloading') : t('settings.installUpdate') }}
                </button>
                <button
                  v-else
                  class="btn"
                  :disabled="checking"
                  @click="doCheck"
                >
                  <IconRefresh v-if="!checking" />
                  {{ checking ? t('settings.checking') : t('settings.checkUpdate') }}
                </button>
              </div>
            </div>

            <!-- 下载进度条 -->
            <div v-if="updateDownloading && updateProgress !== null" class="set-update-progress">
              <span class="set-update-progress-track">
                <span class="set-update-progress-fill" :style="{ width: updateProgress + '%' }" />
              </span>
              <span class="set-update-progress-pct">{{ updateProgress }}%</span>
            </div>

            <!-- 下载/安装失败（不受 updateAvailable 门控） -->
            <p v-if="updateInstallError && !updateDownloading" class="set-update-status set-update-error">
              {{ t('settings.updateInstallFail', { e: updateInstallError }) }}
            </p>

            <!-- 检查结果（无新版本时显示，如"已是最新"或检查失败原因） -->
            <p v-if="updateMsg && !updateAvailable && !updateDownloading" class="set-update-status">
              {{ updateMsg }}
            </p>

            <!-- 次要操作：查看更新日志 / 手动下载 -->
            <button v-if="updateAvailable" class="set-update-notes" @click="openReleasePage()">
              <IconExternalLink />
              {{ t('settings.viewRelease', { v: latestVersion ?? '' }) }}
            </button>
          </div>
        </template>

        <template v-else>
          <div class="set-shortcuts">
            <div class="set-shortcut-group" v-for="g in shortcutGroups" :key="g.title">
              <div class="set-shortcut-group-title">{{ t(g.title) }}</div>
              <div class="set-shortcut-row" v-for="s in g.items" :key="s.key">
                <span class="set-shortcut-label">{{ t(s.label) }}</span>
                <kbd class="set-shortcut-key">{{ s.key }}</kbd>
              </div>
            </div>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>
