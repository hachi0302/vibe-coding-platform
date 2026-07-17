# GitHub CI 自动打包 & 发布

参考 [cc-switch](https://github.com/farion1231/cc-switch/releases) 的形态：每个版本页面包含 ① 版本介绍 + commit 信息、② 各平台安装包、③ Contributors。本文拆解三者如何通过一份 workflow 自动生成。

---

## 总体结构

```
.github/
├── release.yml              # PR 分类规则（决定 changelog 章节）
└── workflows/
    └── release.yml          # 两段式 workflow：build → publish
```

```
build job (matrix)
├── macos-latest   → .dmg / .app.tar.gz
├── windows-latest → .msi / *-setup.exe
└── ubuntu-latest  → .AppImage / .deb (可选)
     │
     └─ upload-artifact  ←  各 runner 上传产物
                │
publish job (ubuntu-latest)
└── download-artifact + softprops/action-gh-release
                │
                └─ Release 页面（含 assets + auto-notes + contributors）
```

---

## 1. Assets（各平台安装包）

每个 runner 跑一次 `tauri build`，把产物上传成 workflow artifact：

```yaml
- name: Build Tauri app
  uses: tauri-apps/tauri-action@v0
  with:
    args: ${{ matrix.args }}

- name: Upload bundle artifacts
  uses: actions/upload-artifact@v4
  with:
    name: bundle-${{ matrix.platform }}
    path: |
      src-tauri/target/release/bundle/**/*.dmg
      src-tauri/target/release/bundle/**/*.msi
      src-tauri/target/release/bundle/**/*-setup.exe
      src-tauri/target/release/bundle/**/*.app.tar.gz
```

`publish` job 把所有 artifact 下载到一个目录，再交给发布 action：

```yaml
- uses: actions/download-artifact@v4
  with: { pattern: bundle-*, path: dist, merge-multiple: true }

- uses: softprops/action-gh-release@v3
  with:
    files: dist/**/*
```

`files` glob 命中的文件就成了 Release 页面的 **Assets** 区。

---

## 2. 版本介绍 & commit 信息

三种来源，**推荐第二种**（GitHub 原生 auto-notes）：

| 方式 | 来源 | 适用 |
| --- | --- | --- |
| 手写 body | workflow 里 `body: \| ...` | 内容固定（cc-switch 用这种） |
| **`generate_release_notes: true`** | GitHub 后端扫描 PR | 自动生成 "What's Changed" + "New Contributors" + "Full Changelog" |
| `release-drafter` | 每次 PR 合并就 patch draft | 持续累积式 changelog |

打开开关只需一行：

```yaml
- uses: softprops/action-gh-release@v3
  with:
    tag_name: ${{ github.ref_name }}
    generate_release_notes: true        # ← 关键
    body: |                              # ← 可在自动内容之前拼一段静态说明
      ## 下载
      - macOS: `cc-sessions-viewer_${{ github.ref_name }}_universal.dmg`
      - Windows: `cc-sessions-viewer_${{ github.ref_name }}_x64-setup.exe`
    files: dist/**/*
```

GitHub 会做这些事：

1. 找上一个 tag (`vN-1`) → 当前 tag (`vN`) 之间所有 merged PR
2. 列出 `* {PR 标题} by @{作者} in #{PR 号}`
3. 检测首次提交者，单独列 **New Contributors** 章节
4. 末尾追加 `Full Changelog: vN-1...vN` 对比链接

⚠️ 如果团队只用 push 不走 PR，自动 notes 会基于 commit message。**推荐合并强制走 PR**，标题就是 changelog 条目。

---

## 3. 分类（Features / Bug Fixes / Other）

在仓库根加一份 `.github/release.yml`：

```yaml
changelog:
  categories:
    - title: 🚀 Features
      labels: [enhancement, feature]
    - title: 🐛 Bug Fixes
      labels: [bug, fix]
    - title: 📚 Docs
      labels: [documentation]
    - title: 🔧 Internal
      labels: [chore, refactor, ci]
    - title: Other Changes
      labels: ['*']                   # 兜底
  exclude:
    labels: [skip-changelog, dependencies]
    authors: [dependabot, renovate-bot]
```

`generate_release_notes: true` 会按这个规则把 PR 分到对应章节。**配置只在打 release 时生效**，平时 PR 流程不受影响。

---

## 4. Contributors

不需要单独配置。`generate_release_notes` 内置：

- **What's Changed** → 每条 PR 末尾的 `by @user` 就是贡献者
- **New Contributors** → 第一次往本仓库合并 PR 的人，自动单列章节

> 想要更花哨的 contributors 头像墙，可加 [`all-contributors`](https://allcontributors.org/) 或在 README 自动维护，但 Release 页面默认这一行就够了。

---

## 5. 完整最小 workflow

`.github/workflows/release.yml`：

```yaml
name: release

on:
  push:
    tags: ['v*']
  workflow_dispatch:

permissions:
  contents: write

concurrency:
  group: release-${{ github.ref_name }}
  cancel-in-progress: false

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: macos-latest
            args: '--target universal-apple-darwin'
          - platform: windows-latest
            args: ''
    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with: { node-version: 20, cache: npm }

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.platform == 'macos-latest' && 'aarch64-apple-darwin,x86_64-apple-darwin' || '' }}

      - uses: Swatinem/rust-cache@v2
        with: { workspaces: src-tauri }

      - run: npm ci

      - uses: tauri-apps/tauri-action@v0
        with:
          args: ${{ matrix.args }}

      - uses: actions/upload-artifact@v4
        with:
          name: bundle-${{ matrix.platform }}
          path: |
            src-tauri/target/**/release/bundle/**/*.dmg
            src-tauri/target/**/release/bundle/**/*.msi
            src-tauri/target/**/release/bundle/**/*-setup.exe
            src-tauri/target/**/release/bundle/**/*.app.tar.gz
          if-no-files-found: error

  publish:
    needs: build
    if: github.ref_type == 'tag'
    runs-on: ubuntu-latest
    permissions: { contents: write }
    steps:
      - uses: actions/download-artifact@v4
        with: { pattern: bundle-*, path: dist, merge-multiple: true }

      - uses: softprops/action-gh-release@v3
        with:
          tag_name: ${{ github.ref_name }}
          name: ${{ github.ref_name }}
          draft: true
          prerelease: false
          generate_release_notes: true
          body: |
            ## 下载
            - **macOS (Apple Silicon + Intel)**: `*_universal.dmg`
            - **Windows x64**: `*_x64-setup.exe` / `*_x64_en-US.msi`
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
```

---

## 6. 触发流程

```bash
# 1. 改 src-tauri/tauri.conf.json 的 version
# 2. 改 package.json 的 version
git commit -am "chore: release v0.2.0"

# 3. 打 tag 推 GitHub
git tag v0.2.0
git push --tags
```

push 后：

1. matrix build job 并行跑 macOS + Windows，约 6–10 min 出包
2. publish job 拉齐所有 artifacts，建一个 **draft** Release
3. GitHub 后端基于上一个 tag 起的 PR 列表生成 changelog
4. 在 GitHub UI 复核 Release → 点 "Publish"

---

## 7. CHANGELOG 自动维护

[`CHANGELOG.md`](../CHANGELOG.md) 也用 CI 维护，不手写。三种主流方案对比：

| 方案 | 触发 | 产物 | 适用 |
| --- | --- | --- | --- |
| **release-please** (Google) | merge to `main` | 自动开 release PR：bump 版本 + 写 CHANGELOG.md + 打 tag + 发 Release | **推荐**：和 monorepo / Tauri 多版本文件契合 |
| `git-cliff` (Rust) | tag push | 重新生成 CHANGELOG.md 并 commit 回去 | 已有 tag 历史想批量补 changelog |
| `release-drafter` | 每次 PR 合并 | 在 GitHub 维护一个 draft Release（不写 CHANGELOG.md） | 不需要 CHANGELOG.md 文件、只要 Release 页面 |

下面以 **release-please** 为例完整接入。

### 7.1 工作原理

```
你 push 一个 feat: / fix: commit 到 main
                │
release-please 监听 main 的 push
                │
读取所有 commit ↓
按 Conventional Commits 解析：
  feat:    → minor bump
  fix:     → patch bump
  feat!:   → major bump（BREAKING CHANGE）
                │
                ↓
更新 / 开启一个名为 "chore(main): release vX.Y.Z" 的 PR：
  - 改 package.json / Cargo.toml / tauri.conf.json 的 version
  - 追加 CHANGELOG.md 条目
                │
                ↓ 你 review 并 merge
release-please 再次触发：
  - 给当前 commit 打 tag vX.Y.Z
  - 创建 GitHub Release（含 changelog）
                │
                ↓
（前面写过的 `release` workflow 因 tag push 触发，构建并上传 assets）
```

要点：CHANGELOG 不是临时生成给 Release 页面看的，**它是真实存在的 markdown 文件**，每次 release PR 都会把新条目 commit 进仓库。这样源码里 / GitHub Release 页面 / 镜像站三处看到的 changelog 完全一致。

### 7.2 配置 release-please

`.github/workflows/release-please.yml`：

```yaml
name: release-please

on:
  push:
    branches: [main]

permissions:
  contents: write
  pull-requests: write

jobs:
  release-please:
    runs-on: ubuntu-latest
    steps:
      - uses: googleapis/release-please-action@v4
        with:
          # 多文件 bump 时使用 manifest 模式
          config-file: .github/release-please-config.json
          manifest-file: .github/.release-please-manifest.json
```

`.github/release-please-config.json` —— 决定 release-please 怎么生成 changelog 以及改哪些文件：

```json
{
  "release-type": "node",
  "changelog-sections": [
    { "type": "feat",     "section": "🚀 Features" },
    { "type": "fix",      "section": "🐛 Bug Fixes" },
    { "type": "perf",     "section": "⚡️ Performance" },
    { "type": "refactor", "section": "🛠 Refactor" },
    { "type": "docs",     "section": "📚 Docs" },
    { "type": "chore",    "hidden": true },
    { "type": "ci",       "hidden": true },
    { "type": "test",     "hidden": true }
  ],
  "include-component-in-tag": false,
  "include-v-in-tag": true,
  "packages": {
    ".": {
      "package-name": "cc-sessions-viewer",
      "extra-files": [
        "src-tauri/Cargo.toml",
        "src-tauri/tauri.conf.json"
      ]
    }
  }
}
```

`.github/.release-please-manifest.json` —— 记录当前版本号（首次手动写入，之后由 release-please 自动维护）：

```json
{
  ".": "0.1.0"
}
```

> ⚠️ `extra-files` 让 release-please 在 release PR 里同时改 `Cargo.toml` 的 `[package].version` 和 `tauri.conf.json` 的 `"version"`。如果你不写，三个文件版本号会漂移。

### 7.3 Conventional Commits 速查

```
feat: add japanese locale          # → 0.1.0 → 0.2.0
fix: tooltip clipped on edge       # → 0.1.0 → 0.1.1
feat!: drop legacy claude v1 path  # → 0.1.0 → 0.1.2
docs: clarify build prereqs        # → CHANGELOG 里出现，版本不变
chore: bump deps                   # → CHANGELOG 隐藏，版本不变
```

PR title 也建议使用同样格式 —— GitHub squash-merge 时 PR title 会变成 commit message，被 release-please 读到。

### 7.4 与现有 release workflow 配合

`release-please` 负责 **打 tag**，前文的 `release` workflow 监听 `tags: ['v*']` 负责 **构建并上传 assets**。两者通过 tag 解耦：

```
push to main
  └─ release-please 维护 release PR
     └─ merge release PR
        └─ release-please 打 tag v0.2.0
           └─ release workflow 触发 → tauri build → Release 挂 assets
```

第一次接入时仓库里需要有一个起步 tag（哪怕是 `v0.0.0`），release-please 才知道从哪里开始算 changelog。

### 7.5 不想跑 release-please？

最简陋方案：让 `softprops/action-gh-release` 在 publish 时把 `generate_release_notes: true` 生成的内容写到 CHANGELOG.md 头部，再提交回 main。但这会有 commit-loop 风险（CI commit 触发 CI），不如 release-please 干净。

---

## 8. 可选：代码签名 / 公证

| 平台 | 需要的 secret | 步骤 |
| --- | --- | --- |
| macOS | `APPLE_CERTIFICATE` (base64 .p12)、`APPLE_CERTIFICATE_PASSWORD`、`APPLE_ID`、`APPLE_PASSWORD`、`APPLE_TEAM_ID`、`KEYCHAIN_PASSWORD` | 导入 cert 到临时 keychain，`tauri build` 自动签 + notarize |
| Windows | `WINDOWS_CERTIFICATE` (base64 .pfx)、`WINDOWS_CERTIFICATE_PASSWORD` | `signtool.exe` 在 tauri-action env 里设好即可 |
| Tauri Updater | `TAURI_SIGNING_PRIVATE_KEY`、`TAURI_SIGNING_PRIVATE_KEY_PASSWORD` | 用 `tauri signer generate` 产出，配置到 `tauri.conf.json` 公钥字段 |

未签名也能用，但 macOS 会提示 "无法打开"（用户需右键 → 打开），Windows SmartScreen 会拦一道。要发给非技术用户就必须签。

---

## 参考

- [Tauri v2 distribution pipeline](https://v2.tauri.app/distribute/pipelines/)
- [tauri-apps/tauri-action](https://github.com/tauri-apps/tauri-action)
- [softprops/action-gh-release](https://github.com/softprops/action-gh-release)
- [GitHub: Automatically generated release notes](https://docs.github.com/en/repositories/releasing-projects-on-github/automatically-generated-release-notes)
- [cc-switch 的实际 workflow](https://github.com/farion1231/cc-switch/blob/main/.github/workflows/release.yml)
