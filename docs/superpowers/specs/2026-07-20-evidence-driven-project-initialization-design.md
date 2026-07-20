# Evidence-Driven Existing Project Initialization Design

## 1. Goal

Existing-project initialization must turn the target repository into executable engineering context for future agents. After initialization, an agent receiving a feature request, bug, refactor, integration task, or review must be able to find the current architecture, business boundaries, reusable assets, known traps, and real verification commands before changing code.

Success is not “a minimum number of Markdown files exists.” Success means the generated context materially prevents architectural drift, duplicate implementations, incompatible contracts, unsafe fallbacks, and unverified delivery.

This design covers existing-project initialization v4. New-project scaffolding keeps its current behavior in this patch and can adopt the same plan model later. Existing user documents and v3 artifacts are never silently renamed or deleted.

## 2. Product Principles

- IPS is the quality reference, not a template to copy. Its important properties are evidence-backed rules, trigger routing, reuse inventories, architecture boundaries, historical incident prevention, and executable checks.
- Every project is different. Output topics, rules, and skills are selected from the target repository's real structure and behavior.
- New generated path components use ASCII kebab-case. Document and rule bodies use Chinese; code identifiers, commands, paths, protocol fields, and technical terms retain their original spelling.
- Frontend and backend are analyzed independently. A full-stack repository receives both contexts; a single-layer repository receives only that layer. Backend-owned frontend integration contracts may be documented without inventing a frontend framework.
- Existing source, documents, agent assets, hooks, and Git configuration are user-owned unless a v4 manifest proves platform ownership.
- General-purpose skills are not copied into every repository. Project skills are created only for recurring, project-specific, high-risk workflows.

## 3. Chosen Architecture

The v4 pipeline is an evidence compiler:

```text
safe repository snapshot
  -> deterministic inventory and structure detection
  -> agent-authored artifact plan with cited evidence
  -> plan validation
  -> staged docs generation
  -> staged rules generation
  -> staged project-skill generation
  -> semantic and safety validation
  -> conflict-checked installation
  -> persistent ownership/status manifest
```

Two rejected approaches are:

1. Enlarging the current prompt. It remains unreliable on large repositories and cannot prevent fixed-template output or unauthorized writes.
2. Installing static language/framework packs. It improves naming but still produces generic guidance and cannot capture project-specific business boundaries and historical traps.

### 3.1 Safe snapshot

The platform creates a resumable run directory under `.vibe-coding-platform/initialization-v4/runs/<run-id>/`. A filtered snapshot of the target repository is copied into `<run>/workspace`; build outputs, dependency caches, `.git`, previous run directories, binary media, oversized files, and known secret material are excluded. The agent runs only inside this snapshot and is not given the original repository path.

The inventory records relative paths, file kinds, sizes, content hashes, module ownership, detected manifests, source/test/config/document roles, and redacted risk findings. Secret values, connection strings, tokens, passwords, and private keys never enter prompts, plans, reports, or generated documents.

The original repository remains unchanged until every staged artifact passes validation. A before-snapshot guards relevant target paths so concurrent user edits become conflicts rather than overwrites.

### 3.2 Project inventory and structure

`ProjectInventory` replaces the frontend/backend-only mental model. It recognizes workspaces, applications, services, packages, libraries, CLI tools, tests, database material, API entrypoints, frontend routing/state/API/type/UI structures, integrations, and existing engineering context. `ProjectLayers` remains a compatibility projection for the current UI.

The inventory includes coverage data: every detected module/build boundary and every important source root must appear in the artifact plan or be explicitly excluded with a reason. Symlinks outside the repository and symlink cycles are not followed.

### 3.3 Artifact plan

The discovery stage writes a machine-readable `artifact-plan.json`. Each item contains:

- logical id, kind (`document`, `rule`, or `skill`), layer, and topic;
- ASCII kebab-case target path under an allowed root;
- why the artifact is needed;
- source paths and symbols that prove the topic;
- modules and workflows covered;
- required sections and cross-references;
- whether it is required or conditional.

Allowed installed roots are:

- `docs/ai/**` for generated project knowledge;
- `.claude/rules/project/**` for project rules;
- `.claude/skills/<project-specific-name>/**` for project skills;
- managed blocks in `CLAUDE.md` and `AGENTS.md` written by the platform, not the agent;
- `.agents/**` links or synchronized copies created by the platform.

The plan must include the common project map, architecture/boundary view, reusable-assets catalog, verification playbook, and known-risks/document-drift view. Other topics are conditional and adaptive.

Backend evidence may produce API and reachability catalogs, callback catalogs, public enum catalogs, physical data models, migration rules, business lifecycle maps, transaction/event/cache rules, security boundaries, and integration maps.

Frontend evidence may produce route/layout maps, state-flow maps, API-client and contract catalogs, shared type/enum catalogs, reusable components, composables, directives, tools, mocks, design-system/theme/interaction rules, and frontend verification playbooks.

The plan may add project-domain topics that do not fit those examples. It must not create an artifact merely to satisfy a count.

### 3.4 Documents

Documents explain the repository as it exists. Every material claim cites a real relative path and, when applicable, a class/function/type/config key. They distinguish implemented, partially implemented, deprecated, unreachable, drifted, and unverified behavior.

Backend API documentation distinguishes controller/router existence, gateway exposure, authentication/authorization, client/SDK contracts, and actual reachability. Physical models come from active schema/migration/entity evidence and distinguish active migrations from archives. Enum catalogs cover cross-boundary values and their consumers, not every internal enum.

Frontend documentation describes actual framework and repository conventions. It records routing, layout, state, API, types, shared UI and logic, mocks, styling/theme behavior, and verification only when those structures exist.

### 3.5 Rules

`.claude/rules/project/README.md` is the trigger router. It maps path patterns and task keywords to required rules.

Each project rule must contain:

- `paths` scope or an explicit task trigger;
- current architecture/behavior with real evidence;
- existing assets that must be searched and reused first;
- forbidden alternatives and the reason they are unsafe;
- affected contracts/modules/consumers;
- historical traps, drift, or known incomplete behavior when present;
- exact verification commands and stop conditions.

Rules are split by real responsibility, such as architecture, backend, frontend, domain, integration, history, and testing. Generic files such as “backend engineering rule” or copies of a platform baseline do not pass validation.

### 3.6 Skills

Skills are generated only when the inventory and plan identify a repeatable, multi-step, project-specific workflow with meaningful failure risk. Each skill uses a fitting pattern (Pipeline, Reviewer, Tool Wrapper, Generator, or a combination) and includes:

- real triggering situations;
- mandatory project documents, rules, source symbols, and commands;
- an impact matrix and ordered workflow;
- explicit gates and failure/stop behavior;
- project-specific verification and known traps.

General development, generic code review, generic debugging, worktree use, and generic skill design remain platform capabilities and are linked from the managed entry instead of copied into the project.

### 3.7 Semantic validation

Validation is driven by `artifact-plan.json`, not a second hard-coded path list. It rejects:

- non-ASCII or non-kebab-case generated paths, except standard names such as `README.md` and `SKILL.md`;
- placeholders, empty sections, generic filler, unsupported claims, or copied examples;
- missing or nonexistent evidence paths and symbols;
- modules/source roots with no coverage and no exclusion reason;
- rules without triggers, reuse anchors, anti-patterns, impact, or verification;
- skills without a real project workflow, project resources, gates, and failure behavior;
- conditional documents created without evidence or required documents omitted despite evidence;
- dangling links, references to generated files that do not exist, or commands not found in repository manifests/scripts/docs;
- secret values in generated content;
- any staged business-source change.

Agent exit status is evidence, not the completion decision. If an agent exits non-zero but the current stage validates, the pipeline advances. If it exits successfully but validation fails, the pipeline repairs only the reported gaps. Repair attempts are bounded and retain precise diagnostics.

## 4. Persistence, Ownership, and Recovery

`initialization-manifest.json` stores schema version, run id, state, stage checkpoints, plan hash, inventory summary, generated artifact paths and hashes, preserved conflicts, diagnostics, timestamps, and platform version.

States are:

- `preflight`
- `snapshot-ready`
- `plan-ready`
- `documents-ready`
- `rules-ready`
- `skills-ready`
- `installing`
- `verifying`
- `completed`
- `failed`
- `interrupted`

Retry resumes at the last valid checkpoint. A stale `running` state becomes `interrupted` after process ownership is gone. Only files recorded in a completed v4 manifest are platform-owned. Reinitialization may replace an owned file only when its current hash matches the previous owned hash; otherwise it is a user conflict.

Installation first validates all target conflicts, then writes files atomically through temporary siblings and rename. Existing unowned files are never overwritten. Partial installation has a journal so retry can complete or report the exact unresolved path without guessing.

Legacy v3 markers are reported as `legacy-v3`, not current. v3 files remain untouched. v4 installs into its isolated English paths and adds managed navigation blocks, allowing coexistence without destructive migration.

## 5. Entry Files and Cross-Platform Sharing

The platform appends or updates a clearly delimited v4 managed block in `CLAUDE.md` and `AGENTS.md`; text outside the block remains byte-for-byte unchanged. If a file is absent, the platform creates a minimal entry containing the block. The block points to the generated project map, rule router, and project skills and describes their read order.

On Unix, absent `.agents/rules`, `.agents/skills`, and `.agents/scripts` may be relative symlinks to `.claude`. Existing real directories are preserved. On Windows or when link creation is unavailable, the platform creates synchronized managed copies plus a small sync manifest; empty fallback directories are invalid. Existing dual trees are reported as a conflict or preserved with explicit navigation rather than forcibly replaced.

Initialization never changes `core.hooksPath`, overwrites pre-commit hooks, or installs repository hooks without a separate explicit user action.

## 6. UI and Observability

Progress phases become scan, plan, documents, rules, skills, install, verify, and complete. Percentages may move within a phase due to activity but cannot claim the next phase before its checkpoint validates.

The UI shows exact current work, artifact counts by type, resume state, and actionable validation/conflict errors. Completion uses the report's real totals rather than describing all artifacts as “Chinese documents.” The status API distinguishes `not-initialized`, `incomplete`, `legacy-v3`, `current-v4`, and `needs-attention`.

Diagnostics are persisted under the run directory with secret redaction. A failed run remains inspectable and resumable; a completed run keeps only the compact manifest/report and removes the large snapshot.

## 7. IAM Acceptance Scenario

`/Users/wax/OtherCode/iam-identity-center` is the primary manual acceptance project. The expected inventory is a backend-only, multi-module Maven repository. It must not invent Vue/React rules. It must identify and explain project-specific topics including:

- parent/common/api/sdk/gateway/service dependency tiers;
- service controller versus gateway route reachability;
- authentication, token, SSO, M2M, tenant-switch, permission, data-scope, and catalog lifecycles;
- the shared organization/department persistence model;
- active Flyway history versus archived SQL and soft-delete/unique-key traps;
- error-code and frontend-contract drift;
- Redis, RabbitMQ, Nacos, COS, SMS, and Feign integration boundaries;
- tests that exist versus CI paths that currently skip tests;
- security-sensitive fail-open or incomplete behavior without copying secret values.

Its project skills must be IAM-specific workflows such as authentication-flow review, token-revocation diagnosis, contract-impact review, permission-catalog review, schema-migration review, tenant-lifecycle review, security-boundary review, and release verification. A generic developer/debugging skill pack is a failure.

## 8. Testing and Release Criteria

Automated Rust tests cover inventory safety and redaction, structure detection, plan parsing/validation, English path rules, evidence coverage, conditional artifacts, staged-source isolation, conflict-safe installation, idempotency, managed blocks, v3/v4 status, interruption/resume, non-zero agent exit with valid artifacts, Unix links, and Windows synchronized-copy fallback.

Frontend tests cover prompt de-duplication, v4 statuses and phases, exact failure details, resume behavior, and report-based completion counts.

Before release:

- targeted Rust and Vitest tests pass;
- the full Rust suite, frontend suite, typecheck/build, formatting, and Clippy pass;
- a mock fixture proves frontend-only, backend-only, full-stack, and multi-module plan behavior;
- IAM dry-run plan/validation demonstrates project-specific output without modifying IAM source or user documents;
- version advances from 0.1.3 to 0.1.4 as an existing-function patch fix;
- code is committed, pushed, tagged, and the release workflow result is checked;
- the local development app is restarted for user self-testing.
