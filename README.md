<div align="center">

# Vibe Coding Platform

[![Version](https://img.shields.io/github/v/release/wangaixin0302-hachi/vibe-coding-platform?color=blue&label=version)](https://github.com/wangaixin0302-hachi/vibe-coding-platform/releases)
[![Platform](https://img.shields.io/badge/platform-Windows%20%7C%20macOS%20%7C%20Linux-lightgrey.svg)](https://github.com/wangaixin0302-hachi/vibe-coding-platform/releases)
[![Built with Tauri](https://img.shields.io/badge/built%20with-Tauri%202-orange.svg)](https://tauri.app/)
[![Vue 3](https://img.shields.io/badge/Vue-3-42b883?logo=vue.js&logoColor=fff)](https://vuejs.org)

**English** · [中文](README.zh-CN.md) · [日本語](README.ja.md) · [CHANGELOG](CHANGELOG.md)

<p align="center">A local-first desktop workspace for coding-agent sessions, project initialization, and AI-assisted technology selection.<br/>Keep the proven session viewer, then create or initialize a real project from the same app.</p>

</div>

https://github.com/user-attachments/assets/9bcb92a8-e5b8-40e5-b492-af252162309b

---

## Key Features

- **Project Factory** — turn a sentence, document, image, or link into a project plan, focused follow-up questions, an editable technical decision, and an executable project skeleton
- **Project Initialization** — inspect an existing project, surface the implementation context in the sidebar, and keep its project-specific instructions together with its agent sessions
- **Faithful replay** — thinking chains, tool-call pairings, structured diffs, and inline screenshots
- **Global search** — cross-project instant search (⌘⇧F) jumps to the exact message
- **In-app chat** — start or resume a session in a built-in chat with live model, reasoning-effort (incl. Opus **Ultracode**), and permission-mode pickers — no terminal required
- **One-click resume** — resume or start a session in an embedded terminal or external app — supports **Terminal.app**, **cmux**, **iTerm2**, **Ghostty**, and **Warp**
- **Shell terminal tabs** — open pure shell tabs alongside agent sessions for running arbitrary commands in the project directory; tabs persist across restarts
- **Split panes** — split any project into side-by-side or stacked panes, each with its own tab strip; drag tabs to reorder within a pane or move them between panes, with keyboard shortcuts for every action (see Settings → Shortcuts). Every project remembers its own layout across restarts
- **cmux deep integration** — auto-reuses existing workspace by cwd, locates running sessions with blue flash, smart split direction, and directory-named tabs
- **Launch arguments** — per-agent CLI flags (e.g. `--dangerously-skip-permissions`) appended on resume / new session
- **Jump to prompt** — locate button lists all user prompts; click to scroll and flash the target message
- **Views history** — per-project, searchable history of every view you've opened, with favorites; jump back to any past read or chat view in one click
- **Deep stats** — aggregate token spend and cost with live model pricing from LiteLLM; slice by project, model, or tool
- **Menu bar stats** — macOS tray icon shows at-a-glance Today / 7d / 30d cost and tokens per agent
- **Live model pricing** — browseable pricing table for Claude / Codex, auto-updated from upstream
- **Flexible export** — single session or batches to offline-readable Markdown, HTML, or lossless JSON
- **Bookmarks** — pin any folder to the sidebar for quick access, per agent
- **Rename & delete** — session renames sync back to the CLI; soft-delete moves to shared trash with restore support
- **Read-only safety** — original JSONL is never touched, never `rm`

## Screenshots

### Project Factory and Initialization

<table>
  <tr>
    <td width="50%">
      <img src="docs/screenshots/project-factory-create.png" alt="Project Factory requirement input" />
      <p align="center"><em>Describe a new project from a sentence, document, screenshot, or link</em></p>
    </td>
    <td width="50%">
      <img src="docs/screenshots/project-factory-selection.png" alt="AI-assisted technology selection" />
      <p align="center"><em>Review only the technology decisions that still need input</em></p>
    </td>
  </tr>
  <tr>
    <td width="50%">
      <img src="docs/screenshots/project-initialization.png" alt="Existing project initialization" />
      <p align="center"><em>Initialize an existing project and follow its live analysis</em></p>
    </td>
    <td width="50%"></td>
  </tr>
</table>

### Session Workspace

<table>
  <tr>
    <td width="50%">
      <img src="docs/screenshots/cover.png" alt="Main view — sidebar, sessions, and chat" />
      <p align="center"><em>Main view — sidebar, sessions, chat</em></p>
    </td>
    <td width="50%">
      <img src="docs/screenshots/chat.png" alt="Faithful replay — thinking, tool calls, structured diffs" />
      <p align="center"><em>Faithful replay — thinking, tool calls, structured diffs</em></p>
    </td>
  </tr>
  <tr>
    <td width="50%">
      <img src="docs/screenshots/split-screen.png" alt="Split panes — multiple sessions side by side" />
      <p align="center"><em>Split panes — multiple sessions side by side, drag tabs between panes</em></p>
    </td>
    <td width="50%">
      <img src="docs/screenshots/chat-preview.png" alt="In-app chat — Mermaid, tables, file mentions and image attachments" />
      <p align="center"><em>In-app chat — Mermaid & tables, @-mention files, attach images</em></p>
    </td>
  </tr>
  <tr>
    <td width="50%">
      <img src="docs/screenshots/session-resume.png" alt="Embedded terminal resume" />
      <p align="center"><em>Embedded terminal — one-click resume or new session</em></p>
    </td>
    <td width="50%">
      <img src="docs/screenshots/search.png" alt="Global search overlay" />
      <p align="center"><em>Global search (⌘⇧F) jumps to the message</em></p>
    </td>
  </tr>
  <tr>
    <td width="50%">
      <img src="docs/screenshots/stats.png" alt="Token & cost analytics" />
      <p align="center"><em>Token & cost analytics by project, model, tool</em></p>
    </td>
    <td width="50%">
      <img src="src/assets/sys-stats.png" alt="Menu bar stats — per-agent cost and token overview" />
      <p align="center"><em>Menu bar stats — per-agent cost & token overview</em></p>
    </td>
  </tr>
  <tr>
    <td width="50%">
      <img src="docs/screenshots/model-price.png" alt="Live model pricing table" />
      <p align="center"><em>Live model pricing</em></p>
    </td>
    <td width="50%">
      <img src="docs/screenshots/trash.png" alt="Shared trash with restore" />
      <p align="center"><em>Shared trash — soft-delete with one-click restore</em></p>
    </td>
  </tr>
  <tr>
    <td width="50%">
      <img src="src/assets/settings.png" alt="Settings — terminal picker and launch arguments" />
      <p align="center"><em>Settings — terminal picker & launch arguments</em></p>
    </td>
    <td width="50%">
      <img src="docs/screenshots/export.png" alt="Exported HTML preview" />
      <p align="center"><em>Exported HTML — fully offline, opens in any browser</em></p>
    </td>
  </tr>
</table>

## Install

Grab the latest installer from [Releases](https://github.com/wangaixin0302-hachi/vibe-coding-platform/releases):

| Platform | File |
| --- | --- |
| macOS (Apple Silicon + Intel) | `.dmg` |
| Windows x64 | `-setup.exe` / `.msi` |
| Linux x86_64 | `.deb` / `.AppImage` |

On macOS the `.app` is **ad-hoc signed but not notarized**, so first launch may show *"Apple cannot verify…"*. Two ways past it:

- Right-click the app in Finder → **Open** → confirm in the dialog (one-time).
- Or strip the quarantine attribute in Terminal:
  ```bash
  sudo xattr -dr com.apple.quarantine "/Applications/Vibe Coding Platform.app"
  ```

On Linux the `.AppImage` is portable — `chmod +x` and run. The `.deb` installs with:
```bash
sudo apt install ./vibe-coding-platform_<ver>_amd64.deb
```

## Development

```bash
git clone https://github.com/wangaixin0302-hachi/vibe-coding-platform.git
cd vibe-coding-platform
npm install
npm run tauri dev      # dev mode
npm run tauri build    # bundle
```

Prereqs: Node 20+, Rust stable. See [`CLAUDE.md`](CLAUDE.md) for architecture notes.

## Contributing

PRs welcome. Please use [Conventional Commits](https://www.conventionalcommits.org/) (`feat:`, `fix:`, `docs:`, ...).

## Support the Project
Maintaining an open-source project requires significant time and resources. Your sponsorship will directly support:

- 🛠️ Continuous development and updates

- 🐛 Swift bug fixes and issue resolution

- 📚 Documentation improvements and expanded examples

### Alipay / WeChat Pay
  
<table>
  <tr>
    <td align="center">
      <img width="190" src="docs/assets/alipay-qr.jpg" alt="Alipay QR code" />
      <br />Alipay
    </td>
    <td align="center">
      <img width="190" src="docs/assets/wechat-pay-qr.jpg" alt="WeChat Pay QR code" />
      <br />WeChat Pay
    </td>
  </tr>
</table>
