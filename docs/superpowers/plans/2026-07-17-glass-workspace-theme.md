# Glass Workspace Theme Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an opt-in frosted-glass theme for the existing Vibe Coding Platform workspace without changing its current workflows or classic themes.

**Architecture:** Extend the persisted `Theme` union with `glass`, map it to root classes and the existing settings/native menu dispatch, then add narrowly scoped shell CSS under `:root.theme-glass`. Tauri window transparency is enabled once at the window level; classic themes retain opaque app CSS.

**Tech Stack:** Vue 3, TypeScript, Vite, Tauri 2, Vitest, CSS `backdrop-filter` and `color-mix`.

## Global Constraints

- The glass theme is opt-in and must be selectable from Settings.
- Existing light, dark, system, Codex, and Dracula themes must remain unchanged.
- Scope visual changes to the application workspace shell; keep code, terminal, and long-form text surfaces readable.
- Do not modify the currently dirty project-initialization files.
- Do not add a UI library or change application behavior.

---

### Task 1: Persist and expose the glass theme

**Files:**
- Modify: `src/settings.ts`
- Modify: `src/components/SettingsModal.vue`
- Modify: `src/App.vue`
- Modify: `src/locales/en.ts`
- Modify: `src/locales/zh.ts`
- Modify: `src/locales/zh-TW.ts`
- Modify: `src/locales/ja.ts`
- Test: `test/settings.theme.test.ts`

**Interfaces:**
- Consumes: `Theme`, `setTheme`, `applyTheme`, and `nativeAppearance` from `src/settings.ts`.
- Produces: `Theme = ... | 'glass'`; root class `theme-glass`; a settings selector and native menu action `theme:glass`.

- [ ] **Step 1: Write the failing theme persistence and root-class tests**

```ts
it('restores the persisted glass theme and applies its root class', async () => {
  localStorage.setItem('theme', 'glass')
  const settings = await import('../src/settings')
  expect(settings.theme.value).toBe('glass')
  expect(document.documentElement.classList.contains('theme-glass')).toBe(true)
  expect(settings.nativeAppearance('glass')).toBe('dark')
})
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `npm run test:run -- test/settings.theme.test.ts`

Expected: FAIL because `glass` is not in `Theme` and `applyTheme` does not add `theme-glass`.

- [ ] **Step 3: Implement the minimal theme wiring**

Extend the `Theme` union and its parser with `glass`, toggle the root `theme-glass` class, map its native titlebar appearance to dark, and add matching Settings, menu, and locale entries without changing existing ordering.

- [ ] **Step 4: Run focused tests**

Run: `npm run test:run -- test/settings.theme.test.ts`

Expected: PASS.

### Task 2: Apply the glass workspace shell

**Files:**
- Modify: `src/style.css`
- Modify: `src-tauri/tauri.conf.json`
- Test: `test/style.glassTheme.test.ts`

**Interfaces:**
- Consumes: root class `theme-glass` from Task 1 and existing shell selectors `.app`, `.app-topbar`, `.sidebar`, `.main`, `.pane-grid`, `.pane`, `.terminal-strip`.
- Produces: a CSS-only glass presentation for the existing workspace and an alpha-capable Tauri main window.

- [ ] **Step 1: Write a style contract test**

```ts
it('contains glass-only workspace shell selectors', () => {
  const css = readFileSync('src/style.css', 'utf8')
  expect(css).toContain(':root.theme-glass')
  expect(css).toContain('.theme-glass .app-topbar')
  expect(css).toContain('backdrop-filter: blur(')
})
```

- [ ] **Step 2: Run the focused test and verify it fails**

Run: `npm run test:run -- test/style.glassTheme.test.ts`

Expected: FAIL because the glass selectors do not exist.

- [ ] **Step 3: Add scoped glass tokens and shell rules**

Define `--glass-shell`, `--glass-panel`, and `--glass-border` under `:root.theme-glass`. Apply blur and saturation to the topbar and sidebar, light blur to workspace panels, and retain opaque terminal/code surfaces. Enable `transparent: true` only for the Tauri main window.

- [ ] **Step 4: Run focused tests and build**

Run: `npm run test:run -- test/style.glassTheme.test.ts && npm run build`

Expected: all tests pass and the Vue typecheck/Vite build exits `0`.

- [ ] **Step 5: Launch the local Tauri preview and manually verify theme switching**

Run: `npm run tauri dev`

Expected: Settings displays “磨砂玻璃（预览）”; choosing it changes the workspace shell while switching back to “深色” restores the prior UI.
