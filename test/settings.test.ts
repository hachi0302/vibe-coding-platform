import { afterEach, describe, expect, it, vi } from 'vitest'
import { nextTick } from 'vue'
import { applyTheme, clearAppCache, lang, setLang, setTheme, theme } from '../src/settings'

const DARK = 'theme-dark'

// Replace window.matchMedia so `theme: 'system'` resolves deterministically.
function stubMatchMedia(matches: boolean) {
  vi.stubGlobal(
    'matchMedia',
    vi.fn().mockImplementation((query: string) => ({
      matches,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  )
}

afterEach(() => {
  vi.unstubAllGlobals()
  document.documentElement.classList.remove(DARK)
  setLang('en')
  setTheme('system')
})

describe('setLang', () => {
  it('updates the ref and persists to localStorage', () => {
    setLang('ja')
    expect(lang.value).toBe('ja')
    expect(localStorage.getItem('lang')).toBe('ja')
  })
})

describe('setTheme', () => {
  it('updates the ref and persists to localStorage', () => {
    setTheme('dark')
    expect(theme.value).toBe('dark')
    expect(localStorage.getItem('theme')).toBe('dark')
  })
})

describe('applyTheme', () => {
  it('adds the dark class when the theme is dark', () => {
    setTheme('dark')
    applyTheme()
    expect(document.documentElement.classList.contains(DARK)).toBe(true)
  })

  it('removes the dark class when the theme is light', () => {
    document.documentElement.classList.add(DARK)
    setTheme('light')
    applyTheme()
    expect(document.documentElement.classList.contains(DARK)).toBe(false)
  })

  it('follows the system preference when the theme is system', () => {
    stubMatchMedia(true)
    setTheme('system')
    applyTheme()
    expect(document.documentElement.classList.contains(DARK)).toBe(true)

    stubMatchMedia(false)
    applyTheme()
    expect(document.documentElement.classList.contains(DARK)).toBe(false)
  })

  it('re-applies automatically (via watchEffect) when the theme ref changes', async () => {
    setTheme('dark')
    await nextTick()
    expect(document.documentElement.classList.contains(DARK)).toBe(true)

    setTheme('light')
    await nextTick()
    expect(document.documentElement.classList.contains(DARK)).toBe(false)
  })
})

describe('clearAppCache', () => {
  it('removes the project-prefs cache key', () => {
    localStorage.setItem('projPrefs:v1', '{"pinned":[]}')
    clearAppCache()
    expect(localStorage.getItem('projPrefs:v1')).toBeNull()
  })
})

// detectSystemLang is module-private and only runs at import time, so we
// re-import a fresh copy of settings.ts under controlled navigator state.
describe('language detection on first load', () => {
  async function freshLoad(opts: {
    languages?: unknown
    storedLang?: string
    storedTheme?: string
  }) {
    localStorage.clear()
    if (opts.storedLang) localStorage.setItem('lang', opts.storedLang)
    if (opts.storedTheme) localStorage.setItem('theme', opts.storedTheme)
    Object.defineProperty(window.navigator, 'languages', {
      value: opts.languages,
      configurable: true,
    })
    vi.resetModules()
    return import('../src/settings')
  }

  it.each([
    ['zh-Hant-TW', 'zh-TW'],
    ['zh-TW', 'zh-TW'],
    ['zh-HK', 'zh-TW'],
    ['zh-MO', 'zh-TW'],
    ['zh-CN', 'zh'],
    ['zh', 'zh'],
    ['ja-JP', 'ja'],
    ['ja', 'ja'],
    ['en-GB', 'en'],
  ])('maps %s to %s', async (tag, expected) => {
    const mod = await freshLoad({ languages: [tag] })
    expect(mod.lang.value).toBe(expected)
  })

  it('falls back to English for an unsupported language', async () => {
    const mod = await freshLoad({ languages: ['fr-FR'] })
    expect(mod.lang.value).toBe('en')
  })

  it('skips empty entries and uses the first usable tag', async () => {
    const mod = await freshLoad({ languages: ['', 'ja-JP'] })
    expect(mod.lang.value).toBe('ja')
  })

  it('falls back to navigator.language when languages is unavailable', async () => {
    const mod = await freshLoad({ languages: undefined })
    expect(mod.lang.value).toBe('en')
  })

  it('prefers an explicit localStorage language over detection', async () => {
    const mod = await freshLoad({ languages: ['ja-JP'], storedLang: 'zh' })
    expect(mod.lang.value).toBe('zh')
  })

  it('restores a persisted theme, defaulting to system', async () => {
    const stored = await freshLoad({ languages: ['en-US'], storedTheme: 'dark' })
    expect(stored.theme.value).toBe('dark')
    const fallback = await freshLoad({ languages: ['en-US'] })
    expect(fallback.theme.value).toBe('system')
  })
})

describe('stats scope / range persistence', () => {
  async function freshStats(opts: { scope?: string; range?: string }) {
    localStorage.clear()
    if (opts.scope) localStorage.setItem('statsScope:v1', opts.scope)
    if (opts.range) localStorage.setItem('statsRange:v1', opts.range)
    vi.resetModules()
    return import('../src/settings')
  }

  it('defaults to all agents + last 3 months when no preference is stored', async () => {
    const mod = await freshStats({})
    expect(mod.statsScope.value).toBe('all')
    expect(mod.statsRange.value).toBe('months3')
  })

  it('restores a valid persisted scope and range', async () => {
    const mod = await freshStats({ scope: 'codex', range: 'days7' })
    expect(mod.statsScope.value).toBe('codex')
    expect(mod.statsRange.value).toBe('days7')
  })

  it('restores a valid persisted custom date range', async () => {
    const mod = await freshStats({ range: 'custom:2026-01-05:2026-07-05' })
    expect(mod.statsRange.value).toBe('custom:2026-01-05:2026-07-05')
  })

  // 老用户 localStorage 里可能存的 'all'（已废弃）；这里 pin 死回退到 months3
  // 而不是再写 'all'，否则 startAgentStats 会被后端拒掉。
  it('migrates legacy "all" range to months3 (and rejects bogus values)', async () => {
    const mod = await freshStats({ scope: 'bogus', range: 'all' })
    expect(mod.statsScope.value).toBe('all')
    expect(mod.statsRange.value).toBe('months3')
    const mod2 = await freshStats({ range: 'forever' })
    expect(mod2.statsRange.value).toBe('months3')
  })

  it('writes back to localStorage when the ref changes', async () => {
    const mod = await freshStats({})
    mod.statsScope.value = 'codex'
    mod.statsRange.value = 'days30'
    await nextTick()
    expect(localStorage.getItem('statsScope:v1')).toBe('codex')
    expect(localStorage.getItem('statsRange:v1')).toBe('days30')
  })
})

describe('agent visibility (enabledAgents / visibleAgents / setAgentEnabled)', () => {
  async function freshAgents(stored?: string) {
    localStorage.clear()
    if (stored !== undefined) localStorage.setItem('enabledAgents:v1', stored)
    vi.resetModules()
    return import('../src/settings')
  }

  it('defaults to all agents enabled when nothing is stored', async () => {
    const mod = await freshAgents()
    expect(mod.visibleAgents.value).toEqual(['claude', 'codex', 'agy', 'opencode'])
  })

  it('restores a persisted subset, preserving the canonical order', async () => {
    const mod = await freshAgents(
      JSON.stringify({ claude: true, codex: false, agy: true, opencode: false }),
    )
    expect(mod.visibleAgents.value).toEqual(['claude', 'agy'])
  })

  it('treats agents missing from stored data as enabled (new agent rollout)', async () => {
    // 旧版本存的 JSON 没有 opencode 键 —— 升级后它应默认可见。
    const mod = await freshAgents(JSON.stringify({ claude: true, codex: false, agy: false }))
    expect(mod.visibleAgents.value).toEqual(['claude', 'opencode'])
  })

  it('falls back to all-enabled when stored data has every agent off', async () => {
    const mod = await freshAgents(
      JSON.stringify({ claude: false, codex: false, agy: false, opencode: false }),
    )
    expect(mod.visibleAgents.value).toEqual(['claude', 'codex', 'agy', 'opencode'])
  })

  it('falls back to all-enabled on corrupt JSON', async () => {
    const mod = await freshAgents('{not json')
    expect(mod.visibleAgents.value).toEqual(['claude', 'codex', 'agy', 'opencode'])
  })

  it('setAgentEnabled disables an agent and persists', async () => {
    const mod = await freshAgents()
    mod.setAgentEnabled('agy', false)
    expect(mod.visibleAgents.value).toEqual(['claude', 'codex', 'opencode'])
    expect(JSON.parse(localStorage.getItem('enabledAgents:v1')!).agy).toBe(false)
  })

  it('refuses to disable the last remaining agent', async () => {
    const mod = await freshAgents(
      JSON.stringify({ claude: true, codex: false, agy: false, opencode: false }),
    )
    mod.setAgentEnabled('claude', false)
    expect(mod.visibleAgents.value).toEqual(['claude'])
  })

  it('re-enables a previously hidden agent', async () => {
    const mod = await freshAgents(
      JSON.stringify({ claude: true, codex: false, agy: false, opencode: false }),
    )
    mod.setAgentEnabled('codex', true)
    expect(mod.visibleAgents.value).toEqual(['claude', 'codex'])
  })
})
