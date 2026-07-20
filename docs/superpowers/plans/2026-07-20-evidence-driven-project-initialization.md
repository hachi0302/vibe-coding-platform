# Evidence-Driven Existing Project Initialization Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace fixed-template existing-project initialization with a safe, resumable, evidence-driven v4 pipeline that produces project-specific English-path documents, rules, and skills.

**Architecture:** The Rust backend builds a redacted project inventory and isolated workspace, asks the selected agent for a machine-readable artifact plan, validates and generates each artifact class in stages, then installs only planned files through a conflict-checked ownership manifest. The Vue UI consumes real v4 status, stage, diagnostics, and artifact totals; the browser no longer owns a second fixed-path prompt contract.

**Tech Stack:** Rust 2021, Serde/serde_json, sha2, UUID, Tauri 2, Vue 3, TypeScript, Vitest.

## Global Constraints

- Existing-project initialization only; do not migrate new-project scaffolding in this patch.
- New generated path components are ASCII kebab-case; `README.md`, `SKILL.md`, `CLAUDE.md`, and `AGENTS.md` are standard exceptions.
- Generated prose is Chinese and every material claim cites a real relative source path and, when applicable, a real symbol.
- Agents run only in an isolated filtered workspace and never receive the original repository path.
- Existing unowned files, user-modified owned files, Git hooks, and Git configuration are never overwritten.
- Frontend/backend/database/integration artifacts are conditional on real evidence.
- Generic developer, debugging, code-review, worktree, and skill-designer skills are not copied into target projects.
- A non-zero agent exit may advance only when the staged output validates; exit zero never bypasses validation.
- Existing v3 markers and partial v3 output are legacy and remain untouched.
- Version moves from 0.1.3 to 0.1.4 only after all release checks pass.

---

## File Structure

- Create `src-tauri/src/project_factory/inventory.rs`: safe scan, structure inference, secret-aware inventory, and filtered workspace copy.
- Create `src-tauri/src/project_factory/artifact_plan.rs`: v4 plan schema, path/evidence/coverage validation, staged artifact validation, and report totals.
- Create `src-tauri/src/project_factory/initialization_state.rs`: state location, atomic state/manifest writes, ownership/conflict checks, managed entry blocks, and cross-platform `.agents` sharing.
- Modify `src-tauri/src/project_factory/initialization.rs`: v4 stage orchestration, backend-owned prompts, agent outcome handling, repair, progress, resume, and installation.
- Modify `src-tauri/src/project_factory/existing.rs`: retain v3 compatibility helpers but route public preparation/finalization/status through v4 pure checks without live prepare writes.
- Modify `src-tauri/src/project_factory/types.rs`: v4 status, progress, report, artifact totals, and diagnostics serialization.
- Modify `src-tauri/src/project_factory/mod.rs` and `src-tauri/src/lib.rs`: export and expose v4 APIs.
- Modify `src/workflows/prompt.ts`: remove fixed path generation contract and keep a short stable product intent.
- Modify `src/projectFactory/types.ts`, `src/projectFactory/initializationProgress.ts`, `src/App.vue`: v4 phases, resume/status rendering, exact error persistence, and real result counts.
- Modify Rust and Vitest tests under `src-tauri/src/project_factory/**`, `src-tauri/tests/project_factory.rs`, `test/workflows/prompt.test.ts`, `test/projectFactory/initializationProgress.test.ts`, and `test/App.initializationProgress.test.ts`.

### Task 1: Safe Inventory and Workspace Snapshot

**Files:**
- Create: `src-tauri/src/project_factory/inventory.rs`
- Modify: `src-tauri/src/project_factory/mod.rs`
- Modify: `src-tauri/src/project_factory/types.rs`
- Modify: `src-tauri/Cargo.toml`
- Test: `src-tauri/src/project_factory/inventory.rs`

**Interfaces:**
- Produces: `inspect_project(root: &Path) -> Result<ProjectInventory, String>`
- Produces: `create_filtered_workspace(root: &Path, workspace: &Path, inventory: &ProjectInventory) -> Result<(), String>`
- Produces: `content_sha256(bytes: &[u8]) -> String`
- Consumed by: Tasks 2–4.

- [ ] **Step 1: Write failing inventory tests**

Add tests that create a nested Maven/Vue fixture and assert:

```rust
let inventory = inspect_project(&root).expect("inventory");
assert!(inventory.layers.backend);
assert!(inventory.layers.frontend);
assert!(inventory.modules.iter().any(|module| module.path == "services/iam"));
assert!(inventory.files.iter().any(|file| file.path == "apps/web/src/router/index.ts"));
assert!(!inventory.files.iter().any(|file| file.path.contains("node_modules")));
```

Add separate tests for symlink escape/loop skipping, private-key exclusion, configuration redaction, deep modules beyond five levels, binary/oversized exclusion, and deterministic hashes.

- [ ] **Step 2: Run the focused Rust tests and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml project_factory::inventory::tests -- --nocapture`

Expected: compilation fails because `inventory` and its public interfaces do not exist.

- [ ] **Step 3: Implement the inventory**

Define serializable models with exact fields:

```rust
pub struct ProjectInventory {
    pub schema_version: u32,
    pub project_name: String,
    pub layers: ProjectLayers,
    pub modules: Vec<ProjectModule>,
    pub source_roots: Vec<String>,
    pub files: Vec<InventoryFile>,
    pub commands: Vec<ProjectCommand>,
    pub risk_keys: Vec<SensitiveFinding>,
}
```

Walk without following symlinks; skip `.git`, `.vibe-coding-platform`, dependency caches, build outputs, binary/media/private-key files, and files above the explicit size cap. Infer modules from Maven/Gradle/npm/Cargo/Go/Python manifests and source roots. Redact assignment values only in copied configuration files while retaining keys and structure. Add `sha2 = "0.10"` and compute stable SHA-256 hashes.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml project_factory::inventory::tests -- --nocapture`

Expected: all inventory tests pass and no test fixture path escapes its temporary root.

- [ ] **Step 5: Commit Task 1**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/project_factory/inventory.rs src-tauri/src/project_factory/mod.rs src-tauri/src/project_factory/types.rs
git commit -m "fix(project-factory): add safe project inventory"
```

### Task 2: Dynamic Artifact Plan and Semantic Validator

**Files:**
- Create: `src-tauri/src/project_factory/artifact_plan.rs`
- Modify: `src-tauri/src/project_factory/mod.rs`
- Modify: `src-tauri/src/project_factory/types.rs`
- Test: `src-tauri/src/project_factory/artifact_plan.rs`

**Interfaces:**
- Consumes: `ProjectInventory` and `content_sha256` from Task 1.
- Produces: `read_artifact_plan(workspace: &Path) -> Result<ArtifactPlan, Vec<ValidationIssue>>`
- Produces: `validate_artifact_plan(workspace: &Path, inventory: &ProjectInventory, plan: &ArtifactPlan) -> Vec<ValidationIssue>`
- Produces: `validate_staged_artifacts(workspace: &Path, inventory: &ProjectInventory, plan: &ArtifactPlan, kind: Option<ArtifactKind>) -> Vec<ValidationIssue>`
- Produces: `artifact_totals(plan: &ArtifactPlan) -> ArtifactTotals`
- Consumed by: Tasks 3–5.

- [ ] **Step 1: Write failing plan tests**

Construct plans directly and assert rejection codes for Chinese/uppercase paths, path traversal, paths outside `docs/ai`, `.claude/rules/project`, or `.claude/skills`, nonexistent evidence, uncovered modules, duplicate logical ids/paths, generic skill names, generic rule topics, missing rule router, missing common documents, and layer mismatch. Assert a backend-only IAM-like plan with API, Flyway, and auth evidence passes without frontend framework artifacts.

Use stable issue codes in assertions:

```rust
assert!(issues.iter().any(|issue| issue.code == "plan.path.not-kebab-case"));
assert!(issues.iter().any(|issue| issue.code == "plan.evidence.missing"));
assert!(issues.iter().any(|issue| issue.code == "plan.module.uncovered"));
```

Add content tests that reject placeholders, missing Chinese prose, missing headings, dangling generated links, secret-looking values, false commands, rules without paths/triggers/reuse/forbidden/impact/verification, and skills without resources/workflow/gates/failure handling.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml project_factory::artifact_plan::tests -- --nocapture`

Expected: compilation fails because plan types and validators are absent.

- [ ] **Step 3: Implement plan models and validators**

Use exact plan schema:

```rust
pub struct ArtifactPlan {
    pub schema_version: u32,
    pub project_name: String,
    pub artifacts: Vec<ArtifactPlanItem>,
    pub exclusions: Vec<CoverageExclusion>,
}

pub struct ArtifactPlanItem {
    pub id: String,
    pub kind: ArtifactKind,
    pub layer: String,
    pub topic: String,
    pub target_path: String,
    pub rationale: String,
    pub evidence: Vec<EvidenceReference>,
    pub covers: Vec<String>,
    pub required_sections: Vec<String>,
}
```

Validate path components and allowlists, resolve evidence only inside the staged workspace, require module/source-root coverage, and aggregate all issues. Parse Markdown links and backtick paths conservatively; require cited source paths to exist but allow external HTTP documentation links. Treat secret-key names plus assignment-like values as errors without echoing the value in diagnostics.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml project_factory::artifact_plan::tests -- --nocapture`

Expected: all plan and content tests pass with stable issue codes.

- [ ] **Step 5: Commit Task 2**

```bash
git add src-tauri/src/project_factory/artifact_plan.rs src-tauri/src/project_factory/mod.rs src-tauri/src/project_factory/types.rs
git commit -m "fix(project-factory): validate adaptive context plans"
```

### Task 3: Persistent State, Ownership, and Safe Installation

**Files:**
- Create: `src-tauri/src/project_factory/initialization_state.rs`
- Modify: `src-tauri/src/project_factory/mod.rs`
- Modify: `src-tauri/src/project_factory/types.rs`
- Test: `src-tauri/src/project_factory/initialization_state.rs`

**Interfaces:**
- Consumes: `ArtifactPlan`, `ArtifactTotals`, `content_sha256`.
- Produces: `state_directory(project: &Path) -> Result<PathBuf, String>`
- Produces: `load_initialization_state(project: &Path) -> Result<Option<InitializationState>, String>`
- Produces: `save_initialization_state(project: &Path, state: &InitializationState) -> Result<(), String>`
- Produces: `install_planned_artifacts(project: &Path, workspace: &Path, plan: &ArtifactPlan, previous: Option<&OwnershipManifest>) -> Result<OwnershipManifest, Vec<ValidationIssue>>`
- Produces: `install_managed_entries(project: &Path, manifest: &mut OwnershipManifest) -> Result<(), Vec<ValidationIssue>>`
- Produces: `share_agent_assets(project: &Path, manifest: &mut OwnershipManifest) -> Result<AgentAssetMode, Vec<ValidationIssue>>`
- Consumed by: Task 4.

- [ ] **Step 1: Write failing state and installer tests**

Test atomic state round-trips, corrupt/future schema handling, state path stability for canonical project aliases, managed block insertion/update preserving prefix/suffix bytes, no overwrite of unowned targets, no overwrite of modified owned targets, idempotent reinstallation, planned-file-only installation, source symlink escape rejection, and completed manifest hash verification.

Add platform-mode tests:

```rust
let mode = share_agent_assets_for_test(&root, false).expect("copy fallback");
assert_eq!(mode, AgentAssetMode::ManagedCopy);
assert_eq!(read(".agents/rules/project/README.md"), read(".claude/rules/project/README.md"));
```

On Unix, also assert safe relative links when both destinations are absent and preservation when a real `.agents` directory already exists.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml project_factory::initialization_state::tests -- --nocapture`

Expected: compilation fails because state and installer functions do not exist.

- [ ] **Step 3: Implement state and installation**

Store active run data under `dirs::data_local_dir()/vibe-coding-platform/project-initialization/<project-hash>/`; store the portable completed ownership manifest at `docs/ai/.initialization-manifest.json`. Write state and manifests via same-directory temporary files, `sync_all`, and rename.

Use these state values exactly: `preflight`, `snapshot-ready`, `plan-ready`, `documents-ready`, `rules-ready`, `skills-ready`, `installing`, `verifying`, `completed`, `failed`, `interrupted`, `conflict`.

Install only plan entries. First preflight every target and collect all conflicts; write nothing when any conflict exists. Then copy to target-directory temporary files and rename. Append/update `<!-- vibe-coding-platform:init:v4:start -->` through `<!-- vibe-coding-platform:init:v4:end -->` blocks in entry files. Never touch hooks or Git config. Use verified relative links where available; otherwise recursively synchronize managed copies and record their hashes.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run: `cargo test --manifest-path src-tauri/Cargo.toml project_factory::initialization_state::tests -- --nocapture`

Expected: all state, ownership, entry, Unix-link, and managed-copy tests pass.

- [ ] **Step 5: Commit Task 3**

```bash
git add src-tauri/src/project_factory/initialization_state.rs src-tauri/src/project_factory/mod.rs src-tauri/src/project_factory/types.rs
git commit -m "fix(project-factory): make initialization conflict safe"
```

### Task 4: V4 Agent Orchestration and Recovery

**Files:**
- Modify: `src-tauri/src/project_factory/initialization.rs`
- Modify: `src-tauri/src/project_factory/existing.rs`
- Modify: `src-tauri/src/project_factory/mod.rs`
- Modify: `src-tauri/src/project_factory/types.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: `src-tauri/src/project_factory/initialization.rs`
- Test: `src-tauri/tests/project_factory.rs`

**Interfaces:**
- Consumes: Tasks 1–3.
- Produces: existing Tauri command names with v4 result/status payloads.
- Produces: `build_v4_stage_prompt(stage: InitializationStage, inventory: &ProjectInventory, plan: Option<&ArtifactPlan>, issues: &[ValidationIssue]) -> String`
- Produces: `evaluate_agent_stage(outcome: &AgentRunOutcome, issues: &[ValidationIssue]) -> StageDecision`
- Produces: `initialize_existing_project_with_agent_progress(...) -> Result<ExistingProjectInitResult, String>` with resume semantics.

- [ ] **Step 1: Write failing orchestration tests**

Assert prompts contain the exact JSON schema, IPS-derived quality gates, English path policy, frontend/backend separation, source evidence rules, and no fixed Chinese output path. Assert the original project path is absent from agent prompts.

Test the stage truth table:

```rust
assert_eq!(evaluate_agent_stage(&non_zero, &[]), StageDecision::AdvanceWithWarning);
assert_eq!(evaluate_agent_stage(&success, &[missing]), StageDecision::Repair);
assert_eq!(evaluate_agent_stage(&non_zero, &[missing]), StageDecision::Repair);
```

Add a fake-agent runner seam so tests cover plan → docs → rules → skills → install → verify, bounded repairs, persisted attempts, restart from the last valid checkpoint, no repeated prepare writes, stale running state becoming interrupted, and aggregated diagnostic errors.

- [ ] **Step 2: Run focused tests and verify RED**

Run: `cargo test --manifest-path src-tauri/Cargo.toml project_factory::initialization::tests -- --nocapture`

Expected: tests fail because the current two-stage v3 orchestration, fixed prompt, and exit-code behavior remain.

- [ ] **Step 3: Implement v4 orchestration**

Replace Documents/RulesAndSkills with Scan/Plan/Documents/Rules/Skills/Install/Verify. Build the inventory and isolated workspace once per run. Plan stage writes `.vibe-coding-platform/artifact-plan.json` inside the snapshot. Each generation stage is given only matching plan entries and must edit only their target paths.

After every agent process exits, run stage validation before interpreting exit status. Persist the outcome, issue codes, attempt count, checkpoint, and warning. Do not advance percentages based only on elapsed time; use real spawn heartbeat, artifact changes, and validated checkpoints. A repeated issue fingerprint without staged changes ends repair early with an actionable error.

Remove live writes from preparation and remove `finalize -> prepare`. Keep v3 parsing only for status classification. After successful installation and verification, delete the large workspace but retain state/report/diagnostic tails.

- [ ] **Step 4: Run focused and integration tests and verify GREEN**

Run:

```bash
cargo test --manifest-path src-tauri/Cargo.toml project_factory::initialization::tests -- --nocapture
cargo test --manifest-path src-tauri/Cargo.toml --test project_factory -- --nocapture
```

Expected: both commands pass; fixtures show no source mutation and recover from an interrupted valid stage.

- [ ] **Step 5: Commit Task 4**

```bash
git add src-tauri/src/project_factory/initialization.rs src-tauri/src/project_factory/existing.rs src-tauri/src/project_factory/mod.rs src-tauri/src/project_factory/types.rs src-tauri/src/lib.rs src-tauri/tests/project_factory.rs
git commit -m "fix(project-factory): orchestrate resumable v4 initialization"
```

### Task 5: Frontend Status, Progress, and Single Prompt Contract

**Files:**
- Modify: `src/workflows/prompt.ts`
- Modify: `src/projectFactory/types.ts`
- Modify: `src/projectFactory/initializationProgress.ts`
- Modify: `src/App.vue`
- Test: `test/workflows/prompt.test.ts`
- Test: `test/projectFactory/initializationProgress.test.ts`
- Test: `test/App.initializationProgress.test.ts`

**Interfaces:**
- Consumes: v4 status/progress/result JSON from Task 4.
- Produces: stable UI types for `scan | plan | documents | rules | skills | install | verify | complete | failed | interrupted | conflict`.

- [ ] **Step 1: Write failing Vitest expectations**

Update prompt tests to assert it states the product goal but does not contain `docs/backend/latest/接口文档`, `.claude/rules/公共`, or any output allowlist owned by Rust. Add status/progress tests for legacy, recoverable, conflict, and current-v4 cases; assert failure detail remains visible and completion copy uses `documents/rules/skills` totals.

- [ ] **Step 2: Run focused tests and verify RED**

Run:

```bash
npm test -- --run test/workflows/prompt.test.ts test/projectFactory/initializationProgress.test.ts test/App.initializationProgress.test.ts
```

Expected: failures show the current fixed prompt, old phase union, transient failure overlay, and generic completion count.

- [ ] **Step 3: Implement the frontend contract**

Reduce `buildProjectInitializationPrompt` to project name plus stable goal and safety intent; Rust adds the authoritative schema and stage contract. Extend types with run id, status, attempt, sequence, recoverable, issues, conflicts, warnings, and artifact totals. Query status before start and resume an incomplete v4 run. Keep failed/interrupted/conflict details until explicit dismissal or retry. Render real stage names and counts.

- [ ] **Step 4: Run focused tests and verify GREEN**

Run the Step 2 command again.

Expected: all focused Vitest tests pass.

- [ ] **Step 5: Commit Task 5**

```bash
git add src/workflows/prompt.ts src/projectFactory/types.ts src/projectFactory/initializationProgress.ts src/App.vue test/workflows/prompt.test.ts test/projectFactory/initializationProgress.test.ts test/App.initializationProgress.test.ts
git commit -m "fix(project-factory): show truthful v4 initialization state"
```

### Task 6: Fixture Acceptance and IAM Dry Run

**Files:**
- Modify: `src-tauri/tests/project_factory.rs`
- Create: `docs/superpowers/verification/2026-07-20-iam-initialization-v4.md`

**Interfaces:**
- Consumes: complete v4 pipeline.
- Produces: regression fixture assertions and a redacted IAM dry-run verification record.

- [ ] **Step 1: Add failing fixture acceptance tests**

Create temporary frontend-only, backend-only, full-stack, and IAM-like multi-module repositories. Assert conditional plan requirements, no invented opposite-layer framework rules, module coverage, English paths, and exact preservation of preexisting CLAUDE/AGENTS/docs/hooks.

- [ ] **Step 2: Run fixture tests and verify RED for any uncovered behavior**

Run: `cargo test --manifest-path src-tauri/Cargo.toml --test project_factory -- --nocapture`

Expected: any missing conditional/coverage behavior fails with a stable validator code; if all behavior is already covered, introduce the assertion before its corresponding production adjustment and confirm that assertion fails against the unadjusted case.

- [ ] **Step 3: Make minimal production corrections and verify GREEN**

Adjust only the responsible inventory/plan/state/orchestration module for each failing fixture. Re-run the Step 2 command until all fixture tests pass.

- [ ] **Step 4: Run IAM dry-run without installation**

Invoke the inventory and plan-only diagnostic path against `/Users/wax/OtherCode/iam-identity-center`. Record hashes/status before and after and assert no IAM source, docs, rules, skills, entry, hooks, or Git configuration changed. Verify the plan covers module tiers, API reachability, auth/token/SSO/tenant/permission/catalog lifecycles, organization/department reuse, Flyway and soft-delete traps, integrations, contract drift, and real verification limitations without including secret values.

- [ ] **Step 5: Write the verification record and commit**

Record the exact commands, result counts, relevant plan artifact names, preservation hashes, and any environment-only limitations. Then:

```bash
git add src-tauri/tests/project_factory.rs docs/superpowers/verification/2026-07-20-iam-initialization-v4.md
git commit -m "test(project-factory): verify adaptive initialization fixtures"
```

### Task 7: Full Verification, Patch Release, and Local Self-Test

**Files:**
- Modify: `package.json`
- Modify: `package-lock.json`
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/tauri.conf.json`

**Interfaces:**
- Consumes: all prior tasks.
- Produces: verified v0.1.4 commit/tag/release and restarted local app.

- [ ] **Step 1: Run format and static checks**

```bash
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings
npm run build
```

Expected: all commands exit 0 with no warnings promoted to errors.

- [ ] **Step 2: Run complete automated suites**

```bash
cargo test --manifest-path src-tauri/Cargo.toml --all-targets
npm run test:run
```

Expected: every Rust and Vitest test passes.

- [ ] **Step 3: Review the complete diff and production safety**

Run `git diff --check`, inspect `git status --short`, inspect every changed file, search for fixed Chinese output paths in v4 prompts/validators, scan generated diagnostics/fixtures for credentials, and confirm no unrelated user changes are staged.

- [ ] **Step 4: Bump the existing-function patch version**

Set all four version sources to `0.1.4`, refresh lockfiles, and rerun Steps 1–2. Commit with:

```bash
git add package.json package-lock.json src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/tauri.conf.json
git commit -m "release: v0.1.4"
```

- [ ] **Step 5: Push, tag, and verify release**

Push `main`, create/push `v0.1.4`, inspect the GitHub Actions run and release assets, and report an external runner outage distinctly from a code failure. Do not claim publication until the release exists and required artifacts are present.

- [ ] **Step 6: Restart local development app**

Stop only the known existing development process/session, run `npm run tauri -- dev`, wait for Vite and Tauri readiness, and leave it running for the user's manual test. Report the local state, commit, tag, CI/release result, and IAM dry-run evidence.
