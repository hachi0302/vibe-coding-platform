# Project Factory Agent CLI Path Fix Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make project-factory Codex and Claude processes start reliably from packaged GUI applications on macOS, Linux, and Windows.

**Architecture:** Move the existing GUI Chat cross-platform `std::process::Command` construction into `agent_command`, then reuse it from GUI Chat and both project-factory agent execution paths. POSIX launches through a login interactive shell; Windows launches through PowerShell after refreshing registry and NVM paths.

**Tech Stack:** Rust, Tauri 2, Cargo tests, Clippy

## Global Constraints

- Preserve all existing Codex and Claude arguments and output handling.
- Cover technical analysis and existing-project initialization.
- Cover macOS, Linux, and Windows without embedding machine-specific absolute paths.
- Apply the existing-function patch bump from `0.1.2` to `0.1.3` before verification.

---

### Task 1: Central cross-platform process builder

**Files:**
- Modify: `src-tauri/src/agent_command.rs`
- Modify: `src-tauri/src/agent_chat.rs`

**Interfaces:**
- Consumes: `AgentCommand`, `powershell_set_location_and_run`, `windows_powershell_exe`
- Produces: `pub fn build_agent_process(cwd: &str, command: &AgentCommand, use_reclaude: bool) -> std::process::Command`

- [x] **Step 1: Write failing platform-specific unit tests**

Add tests that call `build_agent_process`, inspect the resulting program/arguments/current directory, and require POSIX `-l -i -c` command construction or Windows PowerShell PATH refresh construction.

- [x] **Step 2: Run the focused test and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml agent_command::tests -- --nocapture`

Expected: compilation fails because `build_agent_process` does not exist.

- [x] **Step 3: Implement the minimal shared builder**

Move the existing `agent_chat::build_piped_command` OS-specific behavior into `agent_command::build_agent_process`. Preserve login-shell flags, quoting, `npm_config_prefix` removal, current directory, Windows no-console flags, and the existing PowerShell PATH refresh helper.

- [x] **Step 4: Reuse the builder from GUI Chat**

Replace local `build_piped_command` calls with `crate::agent_command::build_agent_process` and remove the duplicated local functions.

- [x] **Step 5: Run the focused tests and verify GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml agent_command::tests -- --nocapture`

Expected: all `agent_command::tests` pass.

### Task 2: Route project-factory agents through the shared builder

**Files:**
- Modify: `src-tauri/src/project_factory/analysis.rs`
- Modify: `src-tauri/src/project_factory/initialization.rs`

**Interfaces:**
- Consumes: `AgentCommand`, `build_agent_process`
- Produces: project-factory Codex and Claude execution that resolves GUI PATH consistently

- [x] **Step 1: Write failing source-level regression tests**

Add module tests that require both project-factory files to construct Codex and Claude through the shared `AgentCommand` path rather than a bare `Command::new`.

- [x] **Step 2: Run the project-factory tests and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml project_factory -- --nocapture`

Expected: new regression assertions fail against the bare command implementation.

- [x] **Step 3: Implement the minimal call-site changes**

Build the same CLI argument lists with `AgentCommand`, pass the correct working directory to `build_agent_process`, and keep `.output()` plus existing error messages unchanged.

- [x] **Step 4: Run the project-factory tests and verify GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml project_factory -- --nocapture`

Expected: all project-factory tests pass.

### Task 3: Cross-project verification

**Files:**
- Verify only

**Interfaces:**
- Consumes: completed Tasks 1 and 2
- Produces: verification evidence for the final handoff

- [x] **Step 1: Format and lint Rust**

Run: `cargo fmt --manifest-path src-tauri/Cargo.toml --check` and `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings`.

Expected: both commands exit successfully without warnings.

- [x] **Step 2: Run all Rust tests**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --all-targets`.

Expected: all tests pass.

- [x] **Step 3: Run frontend verification**

Run: `npm run test:run` and `npm run build`.

Expected: all tests pass and the production build succeeds.
