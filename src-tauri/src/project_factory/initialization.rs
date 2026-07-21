use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime};

use tauri::Emitter;

use crate::agent_command::{build_agent_process, AgentCommand};

use super::existing::prepare_existing_project_initialization;
use super::types::{
    ArtifactKind, ArtifactPlan, ExistingProjectInitResult, ExistingProjectInitializationProgress,
    InitializationRunState, InitializationState, InventoryFile, ProjectCommand, ProjectInventory,
    ProjectModule, SensitiveFinding, ValidationIssue,
};

const MAX_REPAIR_ATTEMPTS: usize = 2;
const INVENTORY_SNAPSHOT_FILE: &str = "inventory.json";
static PROGRESS_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredProjectInventory {
    schema_version: u32,
    project_name: String,
    frontend: bool,
    backend: bool,
    modules: Vec<ProjectModule>,
    source_roots: Vec<String>,
    files: Vec<InventoryFile>,
    commands: Vec<ProjectCommand>,
    risk_keys: Vec<SensitiveFinding>,
}

impl From<&ProjectInventory> for StoredProjectInventory {
    fn from(inventory: &ProjectInventory) -> Self {
        Self {
            schema_version: inventory.schema_version,
            project_name: inventory.project_name.clone(),
            frontend: inventory.layers.frontend,
            backend: inventory.layers.backend,
            modules: inventory.modules.clone(),
            source_roots: inventory.source_roots.clone(),
            files: inventory.files.clone(),
            commands: inventory.commands.clone(),
            risk_keys: inventory.risk_keys.clone(),
        }
    }
}

impl From<StoredProjectInventory> for ProjectInventory {
    fn from(inventory: StoredProjectInventory) -> Self {
        Self {
            schema_version: inventory.schema_version,
            project_name: inventory.project_name,
            layers: super::docs::ProjectLayers {
                frontend: inventory.frontend,
                backend: inventory.backend,
            },
            modules: inventory.modules,
            source_roots: inventory.source_roots,
            files: inventory.files,
            commands: inventory.commands,
            risk_keys: inventory.risk_keys,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitializationStage {
    Scan,
    Plan,
    Documents,
    Rules,
    Skills,
    Install,
    Verify,
    Complete,
    Failed,
    Interrupted,
    Conflict,
}

impl InitializationStage {
    pub(super) fn name(self) -> &'static str {
        match self {
            Self::Scan => "scan",
            Self::Plan => "plan",
            Self::Documents => "documents",
            Self::Rules => "rules",
            Self::Skills => "skills",
            Self::Install => "install",
            Self::Verify => "verify",
            Self::Complete => "complete",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
            Self::Conflict => "conflict",
        }
    }

    pub(super) fn kind(self) -> Option<ArtifactKind> {
        match self {
            Self::Documents => Some(ArtifactKind::Document),
            Self::Rules => Some(ArtifactKind::Rule),
            Self::Skills => Some(ArtifactKind::Skill),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentRunOutcome {
    pub exit_code: Option<i32>,
    pub diagnostic_tail: String,
}

impl AgentRunOutcome {
    pub fn success() -> Self {
        Self {
            exit_code: Some(0),
            diagnostic_tail: String::new(),
        }
    }

    #[cfg(test)]
    pub fn non_zero(exit_code: i32, diagnostic: impl Into<String>) -> Self {
        Self {
            exit_code: Some(exit_code),
            diagnostic_tail: diagnostic.into(),
        }
    }

    fn succeeded(&self) -> bool {
        self.exit_code == Some(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StageDecision {
    Advance,
    AdvanceWithWarning,
    Repair,
}

pub fn evaluate_agent_stage(
    outcome: &AgentRunOutcome,
    issues: &[ValidationIssue],
) -> StageDecision {
    if !issues.is_empty() {
        StageDecision::Repair
    } else if outcome.succeeded() {
        StageDecision::Advance
    } else {
        StageDecision::AdvanceWithWarning
    }
}

/// Content review is intentionally non-blocking.  It gives the agent and the
/// maintainer a record of omissions, but must never turn a usable project
/// initialization into a retry loop.  Only conditions that could make the
/// staged workspace or a later installation unsafe remain blocking.
fn blocking_initialization_issues(issues: &[ValidationIssue]) -> Vec<ValidationIssue> {
    issues
        .iter()
        .filter(|issue| {
            let code = issue.code.as_str();
            code == "workspace.source.modified"
                || code == "plan.secret.detected"
                || code == "plan.schema.unsupported"
                || code == "plan.project-name.mismatch"
                || code.starts_with("plan.path.")
                || code.starts_with("artifact.path.")
                || matches!(
                    code,
                    "artifact.file.missing"
                        | "artifact.file.invalid-text"
                        | "artifact.file.unsafe"
                        | "artifact.secret.detected"
                        | "stage.scope.violation"
                        | "stage.output.unplanned"
                )
        })
        .cloned()
        .collect()
}

fn record_content_audit(
    workspace: &Path,
    stage: InitializationStage,
    attempt: u32,
    outcome: &AgentRunOutcome,
    issues: &[ValidationIssue],
) {
    if issues.is_empty() {
        return;
    }
    let _ = super::context_memory::save_stage_diagnostic(
        workspace,
        stage,
        attempt,
        outcome.exit_code,
        &outcome.diagnostic_tail,
        issues,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepairDecision {
    Retry,
    NoProgress,
    Exhausted,
}

#[derive(Debug, Clone)]
pub struct RepairTracker {
    max_attempts: usize,
    attempts: usize,
    previous_fingerprint: Option<String>,
    previous_digest: Option<String>,
}

impl RepairTracker {
    pub fn new(max_attempts: usize) -> Self {
        Self {
            max_attempts,
            attempts: 0,
            previous_fingerprint: None,
            previous_digest: None,
        }
    }

    pub fn observe(&mut self, issues: &[ValidationIssue], staged_digest: &str) -> RepairDecision {
        let fingerprint = issue_fingerprint(issues);
        if self.previous_fingerprint.as_deref() == Some(&fingerprint)
            && self.previous_digest.as_deref() == Some(staged_digest)
        {
            return RepairDecision::NoProgress;
        }
        if self.attempts >= self.max_attempts {
            return RepairDecision::Exhausted;
        }
        self.attempts += 1;
        self.previous_fingerprint = Some(fingerprint);
        self.previous_digest = Some(staged_digest.to_string());
        RepairDecision::Retry
    }
}

fn issue_fingerprint(issues: &[ValidationIssue]) -> String {
    let mut keys = issues
        .iter()
        .map(|issue| {
            format!(
                "{}|{}|{}|{}",
                issue.code,
                issue.path.as_deref().unwrap_or_default(),
                issue.stage.as_deref().unwrap_or_default(),
                issue.detail
            )
        })
        .collect::<Vec<_>>();
    keys.sort();
    super::inventory::content_sha256(keys.join("\n").as_bytes())
}

fn compact_issue_summary(issues: &[ValidationIssue]) -> Vec<serde_json::Value> {
    let mut grouped = BTreeMap::<(String, Option<String>), usize>::new();
    for issue in issues {
        *grouped
            .entry((issue.code.clone(), issue.path.clone()))
            .or_default() += 1;
    }
    grouped
        .into_iter()
        .map(|((code, path), count)| {
            serde_json::json!({
                "code": code,
                "path": path,
                "count": count,
                "details": format!("{}/validation-issues.json", super::context_memory::CONTEXT_MEMORY_DIR),
            })
        })
        .collect()
}

#[cfg(test)]
pub fn resume_stage(state: InitializationRunState) -> InitializationStage {
    match state {
        InitializationRunState::Preflight => InitializationStage::Scan,
        InitializationRunState::SnapshotReady => InitializationStage::Plan,
        InitializationRunState::PlanReady => InitializationStage::Documents,
        InitializationRunState::DocumentsReady => InitializationStage::Rules,
        InitializationRunState::RulesReady => InitializationStage::Skills,
        InitializationRunState::SkillsReady => InitializationStage::Install,
        InitializationRunState::Installing => InitializationStage::Install,
        InitializationRunState::Verifying | InitializationRunState::Completed => {
            InitializationStage::Verify
        }
        InitializationRunState::Failed
        | InitializationRunState::Interrupted
        | InitializationRunState::Conflict => InitializationStage::Scan,
    }
}

fn active_state(state: InitializationRunState) -> bool {
    !matches!(
        state,
        InitializationRunState::Completed
            | InitializationRunState::Failed
            | InitializationRunState::Interrupted
            | InitializationRunState::Conflict
    )
}

pub fn interrupt_stale_state(state: &mut InitializationState, current_pid: u32) -> bool {
    if active_state(state.state)
        && state
            .process_id
            .is_some_and(|process_id| process_id != current_pid)
    {
        state.state = InitializationRunState::Interrupted;
        state.process_id = None;
        state.issues.push(ValidationIssue {
            code: "run.interrupted".to_string(),
            detail: "检测到上一次初始化进程已经结束，可从最后有效 checkpoint 恢复".to_string(),
            path: None,
            stage: Some("interrupted".to_string()),
        });
        true
    } else {
        false
    }
}

pub fn sanitize_user_intent(intent: &str, project_path: &str) -> String {
    let mut sanitized = intent.replace(project_path, "<project-root>");
    if let Ok(canonical) = fs::canonicalize(project_path) {
        sanitized = sanitized.replace(&canonical.to_string_lossy().to_string(), "<project-root>");
    }
    sanitized
}

fn artifact_kind_name(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Document => "document",
        ArtifactKind::Rule => "rule",
        ArtifactKind::Skill => "skill",
    }
}

const ARTIFACT_PLAN_SCHEMA: &str = r#"{
  "schemaVersion": 1,
  "projectName": "inventory.projectName",
  "artifacts": [{
    "id": "english-kebab-case-logical-id",
    "kind": "document | rule | skill",
    "layer": "common | contract | frontend | backend | database | integration",
    "topic": "evidence-derived-project-specific-topic",
    "targetPath": "exact allowed path for kind",
    "rationale": "Chinese project-specific reason",
    "evidence": [{"path": "exact inventory.files[].path", "symbol": "realDeclaredSymbolOrConfigurationKey"}],
    "covers": ["exact module.name, module.path, or sourceRoot value listed below"],
    "requiredSections": ["Chinese required section"]
  }],
  "exclusions": [{
    "target": "conditional-area",
    "reason": "Chinese evidence-based exclusion",
    "evidence": [{"path": "real/relative/path", "symbol": "realSymbol"}]
  }]
}"#;

fn exact_json_values<'a>(values: impl IntoIterator<Item = &'a str>) -> String {
    let values = values
        .into_iter()
        .map(|value| serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string()))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{values}]")
}

fn build_plan_stage_contract(inventory: &ProjectInventory) -> String {
    let module_names =
        exact_json_values(inventory.modules.iter().map(|module| module.name.as_str()));
    let module_paths =
        exact_json_values(inventory.modules.iter().map(|module| module.path.as_str()));
    let source_roots = exact_json_values(inventory.source_roots.iter().map(String::as_str));
    let document_template_contract = super::document_templates::plan_contract();
    format!(
        r#"工程产物只创建 `.vibe-coding-platform/artifact-plan.json`；分析过程中可更新临时 `context-memory/notes/project-memory.md`。计划文件必须是 JSON，严格匹配以下 exact JSON schema：
{ARTIFACT_PLAN_SCHEMA}

计划审核的精确契约（优先级高于任何推测）：
- `schemaVersion` 必须为 1；`projectName` 必须逐字复制 inventory.projectName。
- 每个 `id` 和 `targetPath` 必须唯一。document 的目录、文件名、标题和正文必须使用 IPS 风格中文（固定 `index.md`、`MOC.md` 除外）；rule 与 skill 的目录、文件名使用 English ASCII kebab-case，固定文件名 README.md 与 SKILL.md 除外。
- document 只允许 `docs/backend/latest/`、`docs/frontend/latest/`、`docs/product/latest/` 或 `docs/test/latest/` 下的 `.md` 文件。禁止创建 `docs/ai/`、项目地图、可复用资产总览、验证手册、文档漂移等 AI 审计文档；后端必须使用“系统架构/业务/接口文档/第三方集成/规范约束”中文分类，前端、产品、测试同样遵循 `document-template-library.md`。
- rule 只允许 `.claude/rules/project/<english-kebab-case>.md`（可按英文 kebab-case 子目录组织），并且必须包含精确路由项 `id: rule-router`、`kind: rule`、`targetPath: .claude/rules/project/README.md`。
- skill 只允许 `.claude/skills/<project-specific-kebab-case>/SKILL.md`，每个 skill 必须绑定本项目复杂或高风险流程，禁止通用开发/调试/评审/重构 skill。
- 通用 IPS 模板由平台原样安装，不得将 `详设文档模板.md`、`开发进度文档模板.md`、`前端接入说明模板.md` 写入计划、修改或重建。
- 文档内容采用 IPS 的资料展示结构；内部 `path + symbol` 只用于确保内容真实，不得在用户文档中创建“真实证据”“维护规则”章节。信息不足只在末尾“待补信息”说明缺什么。

{document_template_contract}

覆盖契约：
- module.name exact values: {module_names}
- module.path exact values: {module_paths}
- sourceRoot exact values: {source_roots}
- covers 只能逐字复制上述 exact values，不能填写能力名、主题名、中文翻译或自行改写。每个 module 和每个 sourceRoot 都必须由至少一个 artifact 覆盖；同一模块可用其 module.name 或 module.path 覆盖，但每个 sourceRoot 必须单独覆盖。
- 每个 covers 值必须至少有一条同一 module/sourceRoot 内的 evidence path；不能用其他模块证据代替。不能真实覆盖时才写 exclusion。
- exclusions.target 也只能逐字复制上述 exact values；必须提供不少于 8 个中文字的项目化原因，以及属于该 module/sourceRoot 的真实 evidence path + symbol。不要排除仅仅因为尚未阅读。

证据契约：
- `evidence.path` 必须逐字复制 inventory.files[].path，且文件必须属于对应 covers/exclusion；不得引用未进入清单的文件或生成中的产物。
- `evidence.symbol` 必须是该证据文件正文中真实 declaration 或 configuration key：例如真实 class/function/type/const/table/view 名或 YAML/JSON/properties 配置键。
- symbol 必须是单个标识符、限定名或配置键；调用表达式、注释、字符串或推测的名称都不是声明。不要填写 `ClassName.method()`、代码片段、自然语言或只有 `service`/`api`/`config` 等泛词。
- 先打开证据文件核对 symbol 的真实声明，再写入计划；路径存在但 symbol 只被调用、注释或字符串提及仍会失败。

层级与主题契约：
- `layer` 只能是 `common | contract | frontend | backend | database | integration`。
- `common` 仅用于同时具有真实前端与后端证据的跨层资料；`contract` 必须有 API/client/SDK/DTO/OpenAPI/proto 边界证据或同时具有前后端证据。
- `frontend`、`backend`、`database`、`integration` 必须分别由 inventory 中匹配的前端、后端、数据库迁移/模型、第三方/API 边界路径与声明证据支持；没有对应证据就不要规划该层产物。
- 除 rule-router 外，`id`/`topic`/`targetPath` 的主题词必须能在 evidence path 或 symbol 中找到；否则 rationale 必须明确写出该项目概念，并逐字引用支持它的 evidence path 或 symbol。不要使用泛化的 backend-engineering、frontend-development、coding-guidelines 等主题。

项目 skill 契约：
- skill 的 rationale 必须明确写出“项目资源全部内嵌在 SKILL.md 中”，`requiredSections` 必须包含语义等价的项目资源章节（例如 `项目资源`、`项目上下文` 或 `Project Resources`）。
- skill 还必须规划触发条件、项目资源、项目专属步骤、完成 Gate、失败处理与可执行验证；项目资源只列真实可读取的路径、命令和资料，不创建“真实证据”章节，不得规划 sidecar resource、模板目录或外部资源文件。"#
    )
}

pub fn build_v4_stage_prompt(
    stage: InitializationStage,
    inventory: &ProjectInventory,
    plan: Option<&ArtifactPlan>,
    issues: &[ValidationIssue],
) -> String {
    let inventory_json = serde_json::to_string_pretty(&serde_json::json!({
        "projectName": inventory.project_name,
        "layers": inventory.layers,
        "moduleCount": inventory.modules.len(),
        "sourceRootCount": inventory.source_roots.len(),
        "fileCount": inventory.files.len(),
        "allowedCommands": inventory.commands,
        "contextIndex": format!("{}/index.json", super::context_memory::CONTEXT_MEMORY_DIR),
    }))
    .unwrap_or_else(|_| "{\"error\":\"inventory summary serialization failed\"}".to_string());
    let issue_json = serde_json::to_string_pretty(&compact_issue_summary(issues))
        .unwrap_or_else(|_| "[]".to_string());
    let stage_contract = match stage {
        InitializationStage::Plan => build_plan_stage_contract(inventory),
        InitializationStage::Documents
        | InitializationStage::Rules
        | InitializationStage::Skills => {
            let kind = stage.kind().expect("generation stage has a kind");
            let scoped = plan
                .map(|plan| {
                    plan.artifacts
                        .iter()
                        .filter(|item| item.kind == kind)
                        .map(|item| {
                            serde_json::json!({
                                "id": item.id,
                                "topic": item.topic,
                                "targetPath": item.target_path,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            let scoped_json =
                serde_json::to_string_pretty(&scoped).unwrap_or_else(|_| "[]".to_string());
            format!(
                "本阶段类型：{}。工程产物只允许编辑本阶段 JSON 中的 targetPath；此外只可更新临时 `context-memory/notes/project-memory.md`，禁止编辑其他计划产物：\n{}",
                artifact_kind_name(kind),
                scoped_json
            )
        }
        _ => "该阶段不调用智能体。".to_string(),
    };
    format!(
        r#"你正在隔离的过滤工作区中执行无交互的项目初始化阶段 `{stage}`。不要询问用户或等待确认。

{stage_contract}

质量与安全契约：
- 文档目录、文件名、标题和正文必须使用中文，固定 `index.md`、`MOC.md` 除外；规则和技能的目录/文件名必须使用 English ASCII kebab-case（README.md、SKILL.md 固定），但标题、正文和章节必须使用中文。路径、代码符号和命令保持原样。
- 每条项目结论先用真实 `path + symbol` 在内部核对；最终用户文档只展示 IPS 风格的业务、接口、表字段、枚举、架构或前端资料，不展示取证过程。
- 严禁臆想或按常见实践补全项目事实。框架、版本、代码风格、命名、目录、模块边界、接口、模型、枚举、配置键、业务流程、历史陷阱、验证方式、rules 和 skills 的每项项目化结论，都必须来自当前清单中的真实文件；证据不足就不写结论，并在文档或 skill 的“待补信息”写明缺什么。
- 三个 IPS 通用模板已原样存在于工作区；不得改写、缩写或重新生成。其余资料必须按 IPS 结构结合项目真实内容生成。
- 对某个 skill、文档或规则缺少的项目事实，不得因此放弃生成或报称完成：在对应工作流下新增“待补信息”，简短写明缺少什么信息；该能力标记为待完善，后续获得信息后补齐。
- `allowedCommands` 是唯一允许写入产物的可执行命令集合。只能逐字使用其中同一条记录的 `cwd` + `command`，不得组合、补参数、改 cwd 或根据 Maven/NPM 等常识推导新命令；没有匹配项就只描述验证目标，不写命令。
- 只有发现前端证据才规划前端路由、状态、API 客户端、类型、组件、布局、composable、directive、主题、测试等；只有发现后端证据才规划模块、API、回调、枚举、业务生命周期等。
- 只有发现数据库证据才规划物理模型、约束与迁移；只有发现第三方集成证据才规划集成边界、失败处理与安全约束。条件不成立必须在 exclusions 中用证据说明，不得创建空壳。
- database 层产物必须引用真实数据库证据；清单存在 SQL/Flyway/Liquibase 文件时，不能只用 Java entity/DO/Mapper 代替迁移或表声明证据。
- 只有 package-info.java 的源码根应使用该文件中的精确 `package com.example...;` 声明作为排除证据，不得用源码根之外的父级 POM 代替。
- rules 必须项目专属，包含触发路由、路径/符号证据、复用优先、禁止替代、影响面、历史陷阱和可执行验证。
- `.claude/skills/skill-designer` 是平台内置能力，已由平台原样安装；需要新增或修改其他 skill 时，先读取并遵守它。其余 skills 只用于本项目复杂或高风险工作流，包含明确触发条件、前置证据、步骤、失败处理和验证；禁止通用 developer/debug/review/worktree/bug-fix/refactor 套件。
- 禁止敏感值、占位符、空表、杜撰命令、杜撰框架、业务代码修改、源码重写、Git hook/config 修改、固定 IPS 项目路径或复制 IPS 项目事实。IPS 的资料工程结构、中文模板与审查方式是本阶段强制基准，详见临时 `document-template-library.md`。
- 只能读取工作区内文件并写入本阶段允许目标及临时 `context-memory/notes/project-memory.md`；不得访问或提及原始项目的绝对路径。

{memory_contract}

项目清单（已脱敏）：
{inventory_json}

上次安全问题（仅处理真实安全问题，不重写已生成产物）：
{issue_json}

完成前在本阶段内自行审核：逐项比对计划、IPS 模板和真实项目内容，直接修正可以确认的问题；平台只记录审核结果，不会把内容质量问题退回给你反复修复。退出码不代表完成。"#,
        stage = stage.name(),
        memory_contract = super::context_memory::prompt_contract(),
    )
}

pub fn aggregate_stage_failure(
    stage: InitializationStage,
    issues: &[ValidationIssue],
    outcome: &AgentRunOutcome,
) -> String {
    let exit = outcome
        .exit_code
        .map(|code| format!("exit code {code}"))
        .unwrap_or_else(|| "exit code unavailable".to_string());
    let mut parts = vec![format!("{} stage failed ({exit})", stage.name())];
    if !outcome.diagnostic_tail.trim().is_empty() {
        parts.push(outcome.diagnostic_tail.trim().to_string());
    }
    parts.extend(issues.iter().map(|issue| {
        let path = issue
            .path
            .as_deref()
            .map(|path| format!(" [{path}]"))
            .unwrap_or_default();
        format!("{}{}: {}", issue.code, path, issue.detail)
    }));
    parts.join("；")
}

/// Compatibility wrapper retained for callers that still build a headless prompt directly.
/// V4 orchestration uses `build_v4_stage_prompt`, whose schema is owned by the backend.
pub fn build_headless_initialization_prompt(base: &str, review_note: Option<&str>) -> String {
    let repair = review_note
        .map(|note| format!("\n审核关注项：{note}"))
        .unwrap_or_default();
    format!(
        "后台非会话任务；不要询问用户或等待确认。\n{base}{repair}\n完成前进行内部审核；完成状态只由安全安装与所有权确认决定。"
    )
}

fn concise_cli_error(output: &[u8]) -> String {
    let text = String::from_utf8_lossy(output);
    let lines = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    lines[lines.len().saturating_sub(16)..].join("\n")
}

fn build_codex_process(project_path: &str, prompt: &str) -> std::process::Command {
    let command = AgentCommand::new("codex")
        .arg("exec")
        .arg("--sandbox")
        .arg("workspace-write")
        .arg("--ephemeral")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(project_path)
        .arg(prompt);
    build_agent_process(project_path, &command, false)
}

fn run_codex(project_path: &str, prompt: &str) -> Result<AgentRunOutcome, String> {
    let output = build_codex_process(project_path, prompt)
        .output()
        .map_err(|error| format!("无法启动 Codex CLI：{error}"))?;
    Ok(AgentRunOutcome {
        exit_code: output.status.code(),
        diagnostic_tail: concise_cli_error(&output.stderr),
    })
}

fn build_claude_process(project_path: &str, prompt: &str) -> std::process::Command {
    let command = AgentCommand::new("claude")
        .arg("--print")
        .arg("--no-session-persistence")
        .arg("--dangerously-skip-permissions")
        .arg("--output-format")
        .arg("text")
        .arg(prompt);
    build_agent_process(project_path, &command, false)
}

fn run_claude(project_path: &str, prompt: &str) -> Result<AgentRunOutcome, String> {
    let output = build_claude_process(project_path, prompt)
        .output()
        .map_err(|error| format!("无法启动 Claude Code：{error}"))?;
    Ok(AgentRunOutcome {
        exit_code: output.status.code(),
        diagnostic_tail: concise_cli_error(&output.stderr),
    })
}

fn run_agent(agent: &str, project_path: &str, prompt: &str) -> Result<AgentRunOutcome, String> {
    match agent {
        "codex" => run_codex(project_path, prompt),
        "claude" => run_claude(project_path, prompt),
        _ => Err("项目初始化只支持 Claude 或 Codex".to_string()),
    }
}

trait AgentRunner {
    fn run(
        &mut self,
        agent: &str,
        workspace: &Path,
        prompt: &str,
        heartbeat: &mut dyn FnMut(),
    ) -> Result<AgentRunOutcome, String>;
}

struct ProcessAgentRunner;

impl AgentRunner for ProcessAgentRunner {
    fn run(
        &mut self,
        agent: &str,
        workspace: &Path,
        prompt: &str,
        heartbeat: &mut dyn FnMut(),
    ) -> Result<AgentRunOutcome, String> {
        let agent = agent.to_string();
        let workspace = workspace.to_string_lossy().to_string();
        let prompt = prompt.to_string();
        let (sender, receiver) = mpsc::sync_channel(1);
        thread::spawn(move || {
            let _ = sender.send(run_agent(&agent, &workspace, &prompt));
        });
        loop {
            match receiver.recv_timeout(Duration::from_secs(1)) {
                Ok(result) => return result,
                Err(mpsc::RecvTimeoutError::Timeout) => heartbeat(),
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    return Err("项目初始化 Agent 进程意外中断".to_string());
                }
            }
        }
    }
}

fn stage_progress(stage: InitializationStage) -> (u8, &'static str) {
    match stage {
        InitializationStage::Scan => (5, "正在安全扫描项目"),
        InitializationStage::Plan => (18, "正在规划项目专属工程上下文"),
        InitializationStage::Documents => (35, "正在生成项目专属文档"),
        InitializationStage::Rules => (53, "正在生成项目专属规则"),
        InitializationStage::Skills => (68, "正在生成项目专属 skills"),
        InitializationStage::Install => (84, "正在进行冲突检查并安装产物"),
        InitializationStage::Verify => (94, "正在确认已安装产物与所有权"),
        InitializationStage::Complete => (100, "初始化完成"),
        InitializationStage::Failed => (0, "初始化失败"),
        InitializationStage::Interrupted => (0, "初始化已中断"),
        InitializationStage::Conflict => (0, "检测到安装冲突"),
    }
}

fn report<F>(
    reporter: &mut F,
    project_path: &str,
    stage: InitializationStage,
    percent: u8,
    detail: &str,
    state: Option<&InitializationState>,
) where
    F: FnMut(ExistingProjectInitializationProgress),
{
    reporter(ExistingProjectInitializationProgress {
        project_path: project_path.to_string(),
        run_id: state.map(|state| state.run_id.clone()),
        phase: stage.name().to_string(),
        percent,
        detail: detail.to_string(),
        attempt: state.map(|state| state.attempt).unwrap_or_default(),
        sequence: next_progress_sequence(),
        recoverable: state.is_none_or(|state| {
            !matches!(
                state.state,
                InitializationRunState::Completed | InitializationRunState::Conflict
            )
        }),
        issues: state.map(|state| state.issues.clone()).unwrap_or_default(),
        conflicts: state
            .map(|state| state.conflicts.clone())
            .unwrap_or_default(),
        warnings: state
            .map(|state| {
                state
                    .warnings
                    .iter()
                    .map(|warning| warning.detail.clone())
                    .collect()
            })
            .unwrap_or_default(),
        artifact_totals: state
            .filter(|state| state.artifact_totals.total > 0)
            .map(|state| state.artifact_totals),
    });
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn next_progress_sequence() -> u64 {
    let now = unix_time_ms();
    loop {
        let previous = PROGRESS_SEQUENCE.load(Ordering::Relaxed);
        let next = now.max(previous.saturating_add(1));
        if PROGRESS_SEQUENCE
            .compare_exchange_weak(previous, next, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
        {
            return next;
        }
    }
}

fn inventory_hash(inventory: &ProjectInventory) -> Result<String, String> {
    serde_json::to_vec(inventory)
        .map(|bytes| super::inventory::content_sha256(&bytes))
        .map_err(|error| format!("无法序列化项目清单：{error}"))
}

fn inventory_snapshot_path(project: &Path) -> Result<PathBuf, String> {
    Ok(super::initialization_state::state_directory(project)?.join(INVENTORY_SNAPSHOT_FILE))
}

fn save_inventory_snapshot(project: &Path, inventory: &ProjectInventory) -> Result<(), String> {
    let path = inventory_snapshot_path(project)?;
    if path.exists() {
        let existing = load_inventory_snapshot(project)?;
        if inventory_hash(&existing)? == inventory_hash(inventory)? {
            return Ok(());
        }
        return Err("初始化 inventory 快照已存在且内容不一致，拒绝覆盖".to_string());
    }
    let parent = path
        .parent()
        .ok_or_else(|| "inventory 快照缺少父目录".to_string())?;
    fs::create_dir_all(parent).map_err(|error| format!("无法创建状态目录：{error}"))?;
    let temporary = parent.join(format!(
        ".inventory-{}-{}.tmp",
        std::process::id(),
        unix_time_ms()
    ));
    let bytes = serde_json::to_vec_pretty(&StoredProjectInventory::from(inventory))
        .map_err(|error| format!("无法序列化 inventory 快照：{error}"))?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| format!("无法创建 inventory 临时文件：{error}"))?;
    file.write_all(&bytes)
        .and_then(|_| file.sync_all())
        .map_err(|error| format!("无法持久化 inventory 快照：{error}"))?;
    fs::rename(&temporary, &path).map_err(|error| {
        let _ = fs::remove_file(&temporary);
        format!("无法原子安装 inventory 快照：{error}")
    })
}

fn load_inventory_snapshot(project: &Path) -> Result<ProjectInventory, String> {
    let path = inventory_snapshot_path(project)?;
    let bytes = fs::read(&path)
        .map_err(|error| format!("无法读取 inventory 快照 {}：{error}", path.display()))?;
    serde_json::from_slice::<StoredProjectInventory>(&bytes)
        .map(ProjectInventory::from)
        .map_err(|error| format!("inventory 快照 JSON 无法解析：{error}"))
}

fn managed_recovery_path(path: &str, plan: &ArtifactPlan) -> bool {
    matches!(
        path,
        "CLAUDE.md" | "AGENTS.md" | ".vibe-coding-platform/.initialization-manifest.json"
    ) || path.starts_with(".agents/rules/")
        || path.starts_with(".agents/skills/")
        || path.starts_with(".agents/scripts/")
        || plan
            .artifacts
            .iter()
            .any(|artifact| artifact.target_path == path)
}

fn source_inventory_signature(
    inventory: &ProjectInventory,
    plan: &ArtifactPlan,
) -> BTreeSet<(String, u64, String)> {
    inventory
        .files
        .iter()
        .filter(|file| !managed_recovery_path(&file.path, plan))
        .map(|file| (file.path.clone(), file.size, file.sha256.clone()))
        .collect()
}

fn only_recoverable_install_changes(
    baseline: &ProjectInventory,
    current: &ProjectInventory,
    plan: &ArtifactPlan,
) -> bool {
    baseline.layers == current.layers
        && source_inventory_signature(baseline, plan) == source_inventory_signature(current, plan)
}

fn plan_hash(plan: &ArtifactPlan) -> Result<String, String> {
    serde_json::to_vec(plan)
        .map(|bytes| super::inventory::content_sha256(&bytes))
        .map_err(|error| format!("无法序列化产物计划：{error}"))
}

fn make_run_id() -> String {
    format!("{}-{}", unix_time_ms(), std::process::id())
}

fn persist_state(project: &Path, state: &mut InitializationState) -> Result<(), String> {
    state.updated_at_unix_ms = unix_time_ms();
    super::initialization_state::save_initialization_state(project, state)
}

fn checkpoint(
    project: &Path,
    state: &mut InitializationState,
    run_state: InitializationRunState,
) -> Result<(), String> {
    state.state = run_state;
    state.process_id = None;
    state.issues.clear();
    state.attempt = 0;
    let completed_at_unix_ms = unix_time_ms();
    if state
        .checkpoints
        .last()
        .is_none_or(|entry| entry.state != run_state)
    {
        state
            .checkpoints
            .push(super::types::InitializationCheckpoint {
                state: run_state,
                artifact_totals: state.artifact_totals,
                completed_at_unix_ms,
            });
    }
    persist_state(project, state)
}

fn checkpoint_state(state: &InitializationState) -> InitializationRunState {
    state
        .checkpoints
        .last()
        .map(|checkpoint| checkpoint.state)
        .unwrap_or_else(|| {
            if matches!(
                state.state,
                InitializationRunState::Failed
                    | InitializationRunState::Interrupted
                    | InitializationRunState::Conflict
            ) {
                InitializationRunState::Preflight
            } else {
                state.state
            }
        })
}

fn reached(current: InitializationRunState, target: InitializationRunState) -> bool {
    fn rank(state: InitializationRunState) -> u8 {
        match state {
            InitializationRunState::Preflight => 0,
            InitializationRunState::SnapshotReady => 1,
            InitializationRunState::PlanReady => 2,
            InitializationRunState::DocumentsReady => 3,
            InitializationRunState::RulesReady => 4,
            InitializationRunState::SkillsReady => 5,
            InitializationRunState::Installing => 6,
            InitializationRunState::Verifying => 7,
            InitializationRunState::Completed => 8,
            InitializationRunState::Failed
            | InitializationRunState::Interrupted
            | InitializationRunState::Conflict => 0,
        }
    }
    rank(current) >= rank(target)
}

fn stage_digest(
    workspace: &Path,
    plan: Option<&ArtifactPlan>,
    stage: InitializationStage,
) -> String {
    let paths = match stage {
        InitializationStage::Plan => vec![".vibe-coding-platform/artifact-plan.json".to_string()],
        _ => plan
            .into_iter()
            .flat_map(|plan| plan.artifacts.iter())
            .filter(|item| stage.kind() == Some(item.kind))
            .map(|item| item.target_path.clone())
            .collect(),
    };
    let mut records = paths
        .into_iter()
        .map(|path| {
            let digest = fs::read(workspace.join(&path))
                .map(|bytes| super::inventory::content_sha256(&bytes))
                .unwrap_or_else(|_| "missing".to_string());
            format!("{path}:{digest}")
        })
        .collect::<Vec<_>>();
    records.sort();
    super::inventory::content_sha256(records.join("\n").as_bytes())
}

type StageSurface = BTreeMap<String, String>;

fn collect_stage_surface(root: &Path, relative: &Path, output: &mut StageSurface) {
    let path = root.join(relative);
    let Ok(metadata) = fs::symlink_metadata(&path) else {
        return;
    };
    if metadata.file_type().is_symlink() {
        output.insert(
            relative.to_string_lossy().replace('\\', "/"),
            "unsafe-link".to_string(),
        );
        return;
    }
    if metadata.is_file() {
        let digest = fs::read(&path)
            .map(|bytes| super::inventory::content_sha256(&bytes))
            .unwrap_or_else(|_| "unreadable".to_string());
        output.insert(relative.to_string_lossy().replace('\\', "/"), digest);
        return;
    }
    if !metadata.is_dir() {
        return;
    }
    let Ok(entries) = fs::read_dir(&path) else {
        return;
    };
    let mut entries = entries.flatten().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        collect_stage_surface(root, &relative.join(entry.file_name()), output);
    }
}

fn stage_surface(workspace: &Path) -> StageSurface {
    let mut output = BTreeMap::new();
    for relative in [
        ".vibe-coding-platform/artifact-plan.json",
        "docs/ai",
        "docs/backend/latest",
        "docs/frontend/latest",
        "docs/product/latest",
        "docs/test/latest",
        ".claude/rules",
        ".claude/skills",
        ".claude/scripts",
    ] {
        collect_stage_surface(workspace, Path::new(relative), &mut output);
    }
    output
}

fn stage_scope_issues(
    before: &StageSurface,
    after: &StageSurface,
    stage: InitializationStage,
    plan: Option<&ArtifactPlan>,
) -> Vec<ValidationIssue> {
    let allowed = if stage == InitializationStage::Plan {
        [".vibe-coding-platform/artifact-plan.json".to_string()]
            .into_iter()
            .collect::<BTreeSet<_>>()
    } else {
        plan.into_iter()
            .flat_map(|plan| plan.artifacts.iter())
            .filter(|item| stage.kind() == Some(item.kind))
            .map(|item| item.target_path.clone())
            .collect()
    };
    before
        .keys()
        .chain(after.keys())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .filter(|path| before.get(*path) != after.get(*path) && !allowed.contains(path.as_str()))
        .map(|path| ValidationIssue {
            code: "stage.scope.violation".to_string(),
            detail: "Agent 修改了当前阶段计划之外的工程上下文产物".to_string(),
            path: Some(path.clone()),
            stage: Some(stage.name().to_string()),
        })
        .collect()
}

fn unplanned_surface_issues(
    surface: &StageSurface,
    inventory: &ProjectInventory,
    plan: Option<&ArtifactPlan>,
    stage: InitializationStage,
) -> Vec<ValidationIssue> {
    let mut known = inventory
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect::<BTreeSet<_>>();
    known.insert(".vibe-coding-platform/artifact-plan.json");
    known.extend(
        surface
            .keys()
            .filter(|path| {
                super::initialization_state::is_builtin_skill_designer_path(path)
                    || super::initialization_state::is_builtin_document_template_path(path)
                    || super::initialization_state::is_builtin_foundation_path(path)
            })
            .map(String::as_str),
    );
    if let Some(plan) = plan {
        known.extend(plan.artifacts.iter().map(|item| item.target_path.as_str()));
    }
    surface
        .keys()
        .filter(|path| !known.contains(path.as_str()))
        .map(|path| ValidationIssue {
            code: "stage.output.unplanned".to_string(),
            detail: "隔离工作区包含未列入项目清单或产物计划的新增文件".to_string(),
            path: Some(path.clone()),
            stage: Some(stage.name().to_string()),
        })
        .collect()
}

fn runner_failure_issue(stage: InitializationStage, detail: String) -> ValidationIssue {
    ValidationIssue {
        code: "agent.spawn.failed".to_string(),
        detail,
        path: None,
        stage: Some(stage.name().to_string()),
    }
}

fn agent_warning(stage: InitializationStage, outcome: &AgentRunOutcome) -> ValidationIssue {
    ValidationIssue {
        code: "agent.exit.non-zero-valid".to_string(),
        detail: format!(
            "{} 阶段 Agent 以 {} 退出，平台已保留产物审核记录",
            stage.name(),
            outcome
                .exit_code
                .map(|code| code.to_string())
                .unwrap_or_else(|| "unknown".to_string())
        ),
        path: None,
        stage: Some(stage.name().to_string()),
    }
}

fn fail_stage(
    project: &Path,
    state: &mut InitializationState,
    stage: InitializationStage,
    issues: Vec<ValidationIssue>,
    outcome: &AgentRunOutcome,
) -> Result<(), String> {
    if !state.workspace_path.is_empty() {
        let _ = super::context_memory::save_stage_diagnostic(
            Path::new(&state.workspace_path),
            stage,
            state.attempt,
            outcome.exit_code,
            &outcome.diagnostic_tail,
            &issues,
        );
    }
    state.state = InitializationRunState::Failed;
    state.process_id = None;
    state.issues = issues;
    persist_state(project, state)?;
    let stage_name = match stage {
        InitializationStage::Plan => "规划产物",
        InitializationStage::Documents => "生成文档",
        InitializationStage::Rules => "生成规则",
        InitializationStage::Skills => "生成 skills",
        InitializationStage::Install => "安全安装",
        InitializationStage::Verify => "验证结果",
        _ => "项目初始化",
    };
    Err(format!(
        "{stage_name}无法安全完成。平台已保留恢复诊断，请处理安全问题后重试。"
    ))
}

fn mark_install_conflict(
    project: &Path,
    state: &mut InitializationState,
    issues: Vec<ValidationIssue>,
) -> String {
    if !state.workspace_path.is_empty() {
        let _ = super::context_memory::save_stage_diagnostic(
            Path::new(&state.workspace_path),
            InitializationStage::Install,
            state.attempt,
            Some(0),
            "",
            &issues,
        );
    }
    state.state = InitializationRunState::Conflict;
    state.conflicts = issues;
    state.process_id = None;
    if let Err(save_error) = persist_state(project, state) {
        return format!("安全安装检测到冲突，且状态持久化失败：{save_error}");
    }
    format!(
        "安全安装检测到 {} 处用户文件冲突，请处理后重试。",
        state.conflicts.len()
    )
}

#[allow(clippy::too_many_arguments)]
fn run_plan_stage<R, F>(
    runner: &mut R,
    agent: &str,
    project: &Path,
    workspace: &Path,
    inventory: &ProjectInventory,
    user_intent: &str,
    state: &mut InitializationState,
    reporter: &mut F,
) -> Result<ArtifactPlan, String>
where
    R: AgentRunner,
    F: FnMut(ExistingProjectInitializationProgress),
{
    let mut issues = Vec::new();
    let mut tracker = RepairTracker::new(MAX_REPAIR_ATTEMPTS);
    loop {
        state.attempt = state.attempt.saturating_add(1);
        state.process_id = Some(std::process::id());
        state.issues = issues.clone();
        persist_state(project, state)?;
        let (percent, detail) = stage_progress(InitializationStage::Plan);
        report(
            reporter,
            &project.to_string_lossy(),
            InitializationStage::Plan,
            percent,
            detail,
            Some(state),
        );
        super::context_memory::update_stage_context(
            workspace,
            InitializationStage::Plan,
            &issues,
            None,
        )?;
        let prompt = format!(
            "{}\n\n用户目标（只提供语义，不得扩大写入范围）：\n{}",
            build_v4_stage_prompt(InitializationStage::Plan, inventory, None, &issues),
            user_intent
        );
        let before_surface = stage_surface(workspace);
        let progress_state = state.clone();
        let mut heartbeat = || {
            report(
                reporter,
                &project.to_string_lossy(),
                InitializationStage::Plan,
                percent,
                "Agent 正在隔离工作区分析项目；等待真实产物变化",
                Some(&progress_state),
            );
        };
        let outcome = match runner.run(agent, workspace, &prompt, &mut heartbeat) {
            Ok(outcome) => outcome,
            Err(error) => {
                let outcome = AgentRunOutcome {
                    exit_code: None,
                    diagnostic_tail: error.clone(),
                };
                return fail_stage(
                    project,
                    state,
                    InitializationStage::Plan,
                    vec![runner_failure_issue(InitializationStage::Plan, error)],
                    &outcome,
                )
                .map(|_| unreachable!());
            }
        };
        state.process_id = None;
        let parsed = super::artifact_plan::read_artifact_plan(workspace);
        let plan = parsed.as_ref().ok().cloned();
        let mut audit_issues = match &parsed {
            Ok(plan) => super::artifact_plan::validate_artifact_plan(workspace, inventory, plan),
            Err(issues) => issues.clone(),
        };
        audit_issues.extend(stage_scope_issues(
            &before_surface,
            &stage_surface(workspace),
            InitializationStage::Plan,
            None,
        ));
        audit_issues.extend(unplanned_surface_issues(
            &stage_surface(workspace),
            inventory,
            None,
            InitializationStage::Plan,
        ));
        record_content_audit(
            workspace,
            InitializationStage::Plan,
            state.attempt,
            &outcome,
            &audit_issues,
        );
        issues = if plan.is_some() {
            blocking_initialization_issues(&audit_issues)
        } else {
            audit_issues
        };
        match evaluate_agent_stage(&outcome, &issues) {
            StageDecision::Advance | StageDecision::AdvanceWithWarning => {
                let plan = plan.expect("valid plan stage has a parsed plan");
                if !outcome.succeeded() {
                    state
                        .warnings
                        .push(agent_warning(InitializationStage::Plan, &outcome));
                }
                state.plan_sha256 = Some(plan_hash(&plan)?);
                state.artifact_totals = super::artifact_plan::artifact_totals(&plan);
                checkpoint(project, state, InitializationRunState::PlanReady)?;
                return Ok(plan);
            }
            StageDecision::Repair => {
                let digest = stage_digest(workspace, None, InitializationStage::Plan);
                match tracker.observe(&issues, &digest) {
                    RepairDecision::Retry => persist_state(project, state)?,
                    RepairDecision::NoProgress | RepairDecision::Exhausted => {
                        return fail_stage(
                            project,
                            state,
                            InitializationStage::Plan,
                            issues,
                            &outcome,
                        )
                        .map(|_| unreachable!());
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn run_artifact_stage<R, F>(
    runner: &mut R,
    agent: &str,
    project: &Path,
    workspace: &Path,
    inventory: &ProjectInventory,
    plan: &ArtifactPlan,
    user_intent: &str,
    stage: InitializationStage,
    state: &mut InitializationState,
    reporter: &mut F,
) -> Result<(), String>
where
    R: AgentRunner,
    F: FnMut(ExistingProjectInitializationProgress),
{
    let kind = stage.kind().expect("artifact stage has a kind");
    if !plan.artifacts.iter().any(|item| item.kind == kind) {
        return Ok(());
    }
    let existing_targets = plan
        .artifacts
        .iter()
        .filter(|item| item.kind == kind)
        .all(|item| workspace.join(&item.target_path).is_file());
    if existing_targets {
        state.issues.clear();
        persist_state(project, state)?;
        return Ok(());
    }
    let mut issues = Vec::new();
    let mut tracker = RepairTracker::new(MAX_REPAIR_ATTEMPTS);
    loop {
        state.attempt = state.attempt.saturating_add(1);
        state.process_id = Some(std::process::id());
        state.issues = issues.clone();
        persist_state(project, state)?;
        let (percent, detail) = stage_progress(stage);
        report(
            reporter,
            &project.to_string_lossy(),
            stage,
            percent,
            detail,
            Some(state),
        );
        super::context_memory::update_stage_context(workspace, stage, &issues, Some(plan))?;
        let prompt = format!(
            "{}\n\n用户目标（只提供语义，不得扩大写入范围）：\n{}",
            build_v4_stage_prompt(stage, inventory, Some(plan), &issues),
            user_intent
        );
        let before_surface = stage_surface(workspace);
        let progress_state = state.clone();
        let mut heartbeat = || {
            report(
                reporter,
                &project.to_string_lossy(),
                stage,
                percent,
                "Agent 正在隔离工作区工作；等待真实产物变化与审核",
                Some(&progress_state),
            );
        };
        let outcome = match runner.run(agent, workspace, &prompt, &mut heartbeat) {
            Ok(outcome) => outcome,
            Err(error) => {
                let outcome = AgentRunOutcome {
                    exit_code: None,
                    diagnostic_tail: error.clone(),
                };
                return fail_stage(
                    project,
                    state,
                    stage,
                    vec![runner_failure_issue(stage, error)],
                    &outcome,
                );
            }
        };
        state.process_id = None;
        let mut audit_issues =
            super::artifact_plan::validate_staged_artifacts(workspace, inventory, plan, Some(kind));
        audit_issues.extend(stage_scope_issues(
            &before_surface,
            &stage_surface(workspace),
            stage,
            Some(plan),
        ));
        audit_issues.extend(unplanned_surface_issues(
            &stage_surface(workspace),
            inventory,
            Some(plan),
            stage,
        ));
        record_content_audit(workspace, stage, state.attempt, &outcome, &audit_issues);
        issues = blocking_initialization_issues(&audit_issues);
        match evaluate_agent_stage(&outcome, &issues) {
            StageDecision::Advance | StageDecision::AdvanceWithWarning => {
                if !outcome.succeeded() {
                    state.warnings.push(agent_warning(stage, &outcome));
                }
                return Ok(());
            }
            StageDecision::Repair => {
                let digest = stage_digest(workspace, Some(plan), stage);
                match tracker.observe(&issues, &digest) {
                    RepairDecision::Retry => persist_state(project, state)?,
                    RepairDecision::NoProgress | RepairDecision::Exhausted => {
                        return fail_stage(project, state, stage, issues, &outcome);
                    }
                }
            }
        }
    }
}

fn initialization_summary(inventory: &ProjectInventory) -> super::types::InventorySummary {
    super::types::InventorySummary {
        modules: inventory.modules.len(),
        source_roots: inventory.source_roots.len(),
        files: inventory.files.len(),
        frontend: inventory.layers.frontend,
        backend: inventory.layers.backend,
    }
}

fn initialize_with_runner<R, F>(
    project_path: &str,
    agent: &str,
    base_prompt: &str,
    runner: &mut R,
    mut reporter: F,
) -> Result<ExistingProjectInitResult, String>
where
    R: AgentRunner,
    F: FnMut(ExistingProjectInitializationProgress),
{
    let project = Path::new(project_path);
    if !project.is_dir() {
        return Err("项目路径不存在或不是目录".to_string());
    }
    if !matches!(agent, "codex" | "claude") {
        return Err("项目初始化只支持 Claude 或 Codex".to_string());
    }
    super::initialization_state::discard_orphaned_completed_state(project)?;
    let current_status = super::existing::existing_project_init_status(project_path)?;
    if current_status.status == "current-v4" {
        return super::existing::finalize_existing_project_initialization(project_path);
    }
    if current_status.status == "needs-attention" && !current_status.recoverable {
        return Err(current_status.detail);
    }
    let preparation = prepare_existing_project_initialization(project_path)?;
    let (percent, detail) = stage_progress(InitializationStage::Scan);
    report(
        &mut reporter,
        project_path,
        InitializationStage::Scan,
        percent,
        detail,
        None,
    );
    let current_inventory = super::inventory::inspect_project(project)?;
    let current_inventory_sha256 = inventory_hash(&current_inventory)?;
    let user_intent = sanitize_user_intent(base_prompt, project_path);
    let saved_state = super::initialization_state::load_initialization_state(project)?;
    let new_run = saved_state.is_none();
    let mut state = saved_state.unwrap_or_else(|| InitializationState {
        schema_version: super::initialization_state::INITIALIZATION_STATE_SCHEMA_VERSION,
        run_id: make_run_id(),
        state: InitializationRunState::Preflight,
        started_at_unix_ms: unix_time_ms(),
        ..InitializationState::default()
    });
    PROGRESS_SEQUENCE.fetch_max(state.updated_at_unix_ms, Ordering::Relaxed);
    let inventory = if new_run {
        save_inventory_snapshot(project, &current_inventory)?;
        current_inventory.clone()
    } else {
        load_inventory_snapshot(project).map_err(|error| {
            format!("无法安全恢复初始化：{error}。缺少原始清单时不会猜测项目是否变化")
        })?
    };
    if interrupt_stale_state(&mut state, std::process::id()) {
        persist_state(project, &mut state)?;
    }
    let checkpoint_before_resume = checkpoint_state(&state);
    let inventory_changed = state
        .inventory_sha256
        .as_deref()
        .is_some_and(|hash| hash != current_inventory_sha256);
    let recoverable_install_change = inventory_changed
        && reached(
            checkpoint_before_resume,
            InitializationRunState::SkillsReady,
        )
        && !state.workspace_path.is_empty()
        && Path::new(&state.workspace_path).is_dir()
        && super::artifact_plan::read_artifact_plan(Path::new(&state.workspace_path)).is_ok_and(
            |plan| only_recoverable_install_changes(&inventory, &current_inventory, &plan),
        );
    if inventory_changed
        && checkpoint_before_resume != InitializationRunState::Preflight
        && !recoverable_install_change
    {
        state.state = InitializationRunState::Conflict;
        state.conflicts.push(ValidationIssue {
            code: "resume.inventory.changed".to_string(),
            detail: "项目在未完成初始化期间发生变化，拒绝复用旧工作区".to_string(),
            path: None,
            stage: Some("scan".to_string()),
        });
        persist_state(project, &mut state)?;
        return Err(
            "项目在未完成初始化期间发生变化；请处理 needs-attention 冲突后重试".to_string(),
        );
    }
    if state.inventory_sha256.is_none() {
        state.inventory_sha256 = Some(inventory_hash(&inventory)?);
    }
    let mut completed_checkpoint = checkpoint_state(&state);
    let workspace = if reached(completed_checkpoint, InitializationRunState::SnapshotReady)
        && !state.workspace_path.is_empty()
        && Path::new(&state.workspace_path).is_dir()
    {
        PathBuf::from(&state.workspace_path)
    } else {
        let workspace = super::initialization_state::state_directory(project)?
            .join(format!("workspace-{}", state.run_id));
        state.workspace_path = workspace.to_string_lossy().to_string();
        persist_state(project, &mut state)?;
        super::inventory::create_filtered_workspace(project, &workspace, &inventory)?;
        checkpoint(project, &mut state, InitializationRunState::SnapshotReady)?;
        completed_checkpoint = InitializationRunState::SnapshotReady;
        workspace
    };
    super::context_memory::prepare_context_memory(&workspace, &inventory)?;
    super::initialization_state::install_builtin_skill_designer(&workspace).map_err(|issues| {
        aggregate_stage_failure(
            InitializationStage::Plan,
            &issues,
            &AgentRunOutcome::success(),
        )
    })?;
    super::initialization_state::install_builtin_document_templates(&workspace).map_err(
        |issues| {
            aggregate_stage_failure(
                InitializationStage::Plan,
                &issues,
                &AgentRunOutcome::success(),
            )
        },
    )?;
    super::initialization_state::install_builtin_foundation_assets(&workspace, &inventory)
        .map_err(|issues| {
            aggregate_stage_failure(
                InitializationStage::Plan,
                &issues,
                &AgentRunOutcome::success(),
            )
        })?;

    let mut plan = if reached(completed_checkpoint, InitializationRunState::PlanReady) {
        super::artifact_plan::read_artifact_plan(&workspace).map_err(|issues| {
            aggregate_stage_failure(
                InitializationStage::Plan,
                &issues,
                &AgentRunOutcome::success(),
            )
        })?
    } else {
        run_plan_stage(
            runner,
            agent,
            project,
            &workspace,
            &inventory,
            &user_intent,
            &mut state,
            &mut reporter,
        )?
    };
    let plan_issues = super::artifact_plan::validate_artifact_plan(&workspace, &inventory, &plan);
    record_content_audit(
        &workspace,
        InitializationStage::Plan,
        state.attempt,
        &AgentRunOutcome::success(),
        &plan_issues,
    );
    if !blocking_initialization_issues(&plan_issues).is_empty() {
        plan = run_plan_stage(
            runner,
            agent,
            project,
            &workspace,
            &inventory,
            &user_intent,
            &mut state,
            &mut reporter,
        )?;
    }
    completed_checkpoint = checkpoint_state(&state);

    for (stage, ready) in [
        (
            InitializationStage::Documents,
            InitializationRunState::DocumentsReady,
        ),
        (
            InitializationStage::Rules,
            InitializationRunState::RulesReady,
        ),
        (
            InitializationStage::Skills,
            InitializationRunState::SkillsReady,
        ),
    ] {
        if !reached(completed_checkpoint, ready) {
            run_artifact_stage(
                runner,
                agent,
                project,
                &workspace,
                &inventory,
                &plan,
                &user_intent,
                stage,
                &mut state,
                &mut reporter,
            )?;
            checkpoint(project, &mut state, ready)?;
            completed_checkpoint = ready;
        }
    }

    let staged_issues =
        super::artifact_plan::validate_staged_artifacts(&workspace, &inventory, &plan, None);
    record_content_audit(
        &workspace,
        InitializationStage::Verify,
        state.attempt,
        &AgentRunOutcome::success(),
        &staged_issues,
    );
    let blocking_staged_issues = blocking_initialization_issues(&staged_issues);
    if !blocking_staged_issues.is_empty() {
        return fail_stage(
            project,
            &mut state,
            InitializationStage::Verify,
            blocking_staged_issues,
            &AgentRunOutcome::success(),
        )
        .map(|_| unreachable!());
    }

    let (percent, detail) = stage_progress(InitializationStage::Install);
    report(
        &mut reporter,
        project_path,
        InitializationStage::Install,
        percent,
        detail,
        Some(&state),
    );
    state.state = InitializationRunState::Installing;
    persist_state(project, &mut state)?;
    let previous = super::initialization_state::load_ownership_manifest(project)?;
    let mut manifest = match super::initialization_state::install_planned_artifacts(
        project,
        &workspace,
        &plan,
        previous.as_ref(),
    ) {
        Ok(manifest) => manifest,
        Err(issues) => {
            let error = mark_install_conflict(project, &mut state, issues);
            report(
                &mut reporter,
                project_path,
                InitializationStage::Conflict,
                0,
                &error,
                Some(&state),
            );
            return Err(error);
        }
    };
    manifest.inventory_summary = Some(initialization_summary(&inventory));
    if let Err(issues) = super::initialization_state::install_builtin_skill_designer(project) {
        let error = mark_install_conflict(project, &mut state, issues);
        report(
            &mut reporter,
            project_path,
            InitializationStage::Conflict,
            0,
            &error,
            Some(&state),
        );
        return Err(error);
    }
    if let Err(issues) = super::initialization_state::install_builtin_document_templates(project) {
        let error = mark_install_conflict(project, &mut state, issues);
        report(
            &mut reporter,
            project_path,
            InitializationStage::Conflict,
            0,
            &error,
            Some(&state),
        );
        return Err(error);
    }
    if let Err(issues) =
        super::initialization_state::install_builtin_foundation_assets(project, &inventory)
    {
        let error = mark_install_conflict(project, &mut state, issues);
        report(
            &mut reporter,
            project_path,
            InitializationStage::Conflict,
            0,
            &error,
            Some(&state),
        );
        return Err(error);
    }
    if let Err(issues) =
        super::initialization_state::install_managed_entries(project, &mut manifest)
    {
        let error = mark_install_conflict(project, &mut state, issues);
        report(
            &mut reporter,
            project_path,
            InitializationStage::Conflict,
            0,
            &error,
            Some(&state),
        );
        return Err(error);
    }
    if let Err(issues) = super::initialization_state::share_agent_assets(project, &mut manifest) {
        let error = mark_install_conflict(project, &mut state, issues);
        report(
            &mut reporter,
            project_path,
            InitializationStage::Conflict,
            0,
            &error,
            Some(&state),
        );
        return Err(error);
    }

    let (percent, detail) = stage_progress(InitializationStage::Verify);
    report(
        &mut reporter,
        project_path,
        InitializationStage::Verify,
        percent,
        detail,
        Some(&state),
    );
    state.state = InitializationRunState::Verifying;
    persist_state(project, &mut state)?;
    manifest.state = InitializationRunState::Completed;
    manifest.completed_at_unix_ms = unix_time_ms();
    manifest
        .checkpoints
        .push(super::types::InitializationCheckpoint {
            state: InitializationRunState::Completed,
            artifact_totals: manifest.artifact_totals,
            completed_at_unix_ms: manifest.completed_at_unix_ms,
        });
    let verification = super::initialization_state::verify_ownership_manifest(project, &manifest);
    if !verification.is_empty() {
        return fail_stage(
            project,
            &mut state,
            InitializationStage::Verify,
            verification,
            &AgentRunOutcome::success(),
        )
        .map(|_| unreachable!());
    }
    super::initialization_state::save_ownership_manifest(project, &manifest)?;
    state.artifact_totals = manifest.artifact_totals;
    checkpoint(project, &mut state, InitializationRunState::Completed)?;
    let state_root = super::initialization_state::state_directory(project)?;
    if workspace.starts_with(&state_root) && workspace.is_dir() {
        fs::remove_dir_all(&workspace)
            .map_err(|error| format!("初始化已完成，但无法清理隔离工作区：{error}"))?;
    }
    state.workspace_path.clear();
    persist_state(project, &mut state)?;
    if let Ok(path) = inventory_snapshot_path(project) {
        let _ = fs::remove_file(path);
    }
    report(
        &mut reporter,
        project_path,
        InitializationStage::Complete,
        100,
        "初始化完成并确认所有权",
        Some(&state),
    );
    Ok(ExistingProjectInitResult {
        project_path: project.to_string_lossy().to_string(),
        status: "current-v4".to_string(),
        phase: "complete".to_string(),
        run_id: state.run_id.clone(),
        percent: 100,
        detail: "初始化完成并确认所有权".to_string(),
        attempt: state.attempt,
        sequence: next_progress_sequence(),
        recoverable: false,
        issues: Vec::new(),
        conflicts: Vec::new(),
        warnings: state
            .warnings
            .iter()
            .map(|warning| warning.detail.clone())
            .collect(),
        artifact_totals: manifest.artifact_totals,
        layers: Some(preparation.layers),
        detected_stack: preparation.detected_stack,
        generated: plan
            .artifacts
            .into_iter()
            .map(|item| item.target_path)
            .collect(),
    })
}

pub fn initialize_existing_project_with_agent_progress(
    app: &tauri::AppHandle,
    project_path: &str,
    agent: &str,
    prompt: &str,
) -> Result<ExistingProjectInitResult, String> {
    initialize_with_runner(
        project_path,
        agent,
        prompt,
        &mut ProcessAgentRunner,
        |progress| {
            let _ = app.emit("project-factory://initialization-progress", progress);
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{
        aggregate_stage_failure, blocking_initialization_issues, build_claude_process,
        build_codex_process, build_v4_stage_prompt, evaluate_agent_stage, initialize_with_runner,
        interrupt_stale_state, resume_stage, sanitize_user_intent, stage_scope_issues,
        AgentRunOutcome, AgentRunner, InitializationStage, RepairDecision, RepairTracker,
        StageDecision, StageSurface,
    };
    use crate::project_factory::types::{
        ArtifactKind, ArtifactPlan, ArtifactPlanItem, CoverageExclusion, EvidenceReference,
        InitializationRunState, InitializationState, ProjectCommand, ProjectInventory,
        ProjectModule, ValidationIssue,
    };
    use std::fs;
    use std::path::Path;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn rendered_process(process: &std::process::Command) -> String {
        process
            .get_args()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn initialization_agents_use_packaged_gui_path_resolution() {
        let codex = build_codex_process("/tmp/project path", "initialize it");
        let claude = build_claude_process("/tmp/project path", "initialize it");
        let codex_rendered = rendered_process(&codex);
        let claude_rendered = rendered_process(&claude);

        assert!(codex_rendered.contains("codex"));
        assert!(claude_rendered.contains("claude"));
        #[cfg(unix)]
        {
            assert!(codex_rendered.starts_with("-l -i -c "));
            assert!(claude_rendered.starts_with("-l -i -c "));
        }
        #[cfg(windows)]
        {
            assert!(codex_rendered.contains("GetEnvironmentVariable('Path', 'Machine')"));
            assert!(claude_rendered.contains("GetEnvironmentVariable('Path', 'Machine')"));
        }
    }

    fn inventory() -> ProjectInventory {
        ProjectInventory {
            schema_version: 1,
            project_name: "sample-service".to_string(),
            layers: crate::project_factory::docs::ProjectLayers {
                frontend: false,
                backend: true,
            },
            modules: vec![ProjectModule {
                name: "service".to_string(),
                path: ".".to_string(),
                kind: "rust".to_string(),
                manifests: vec!["Cargo.toml".to_string()],
                source_roots: vec!["src".to_string()],
            }],
            source_roots: vec!["src".to_string()],
            files: vec![],
            commands: vec![ProjectCommand {
                name: "test".to_string(),
                command: "cargo test".to_string(),
                cwd: ".".to_string(),
            }],
            risk_keys: vec![],
        }
    }

    fn plan() -> ArtifactPlan {
        ArtifactPlan {
            schema_version: 1,
            project_name: "sample-service".to_string(),
            artifacts: vec![ArtifactPlanItem {
                id: "auth-boundary".to_string(),
                kind: ArtifactKind::Rule,
                layer: "backend".to_string(),
                topic: "authorization".to_string(),
                target_path: ".claude/rules/project/auth-boundary.md".to_string(),
                rationale: "记录真实鉴权边界".to_string(),
                evidence: vec![EvidenceReference {
                    path: "src/auth.rs".to_string(),
                    symbol: Some("authorize".to_string()),
                }],
                covers: vec!["authorization".to_string()],
                required_sections: vec!["触发条件".to_string(), "禁止替代".to_string()],
            }],
            exclusions: Vec::<CoverageExclusion>::new(),
        }
    }

    fn fixture(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("vibe-init-{name}-{suffix}"));
        fs::create_dir_all(root.join("src")).expect("fixture source");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"sample-auth-service\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\naxum = \"0.8\"\n",
        )
        .expect("fixture manifest");
        fs::write(
            root.join("src/lib.rs"),
            "pub fn auth_service() -> bool { true }\n",
        )
        .expect("fixture source");
        root
    }

    fn complete_plan(inventory: &ProjectInventory) -> ArtifactPlan {
        let covers = inventory
            .modules
            .iter()
            .map(|module| module.name.clone())
            .chain(inventory.source_roots.iter().cloned())
            .collect::<Vec<_>>();
        let item = |id: &str, kind: ArtifactKind, target: &str, topic: &str| ArtifactPlanItem {
            id: id.to_string(),
            kind,
            layer: "backend".to_string(),
            topic: topic.to_string(),
            target_path: target.to_string(),
            rationale: "记录当前认证服务的真实工程边界与复用约束".to_string(),
            evidence: vec![EvidenceReference {
                path: "src/lib.rs".to_string(),
                symbol: Some("auth_service".to_string()),
            }],
            covers: covers.clone(),
            required_sections: vec!["待补信息".to_string()],
        };
        let mut skill = item(
            "auth-change-review",
            ArtifactKind::Skill,
            ".claude/skills/sample-auth-change-review/SKILL.md",
            "authentication-change-review",
        );
        skill.rationale = "项目资源全部内嵌在 SKILL.md 中，避免未受计划约束的旁路文件".to_string();
        skill.required_sections.push("项目资源".to_string());
        let mut plan = ArtifactPlan {
            schema_version: 1,
            project_name: inventory.project_name.clone(),
            artifacts: vec![
                item(
                    "backend-system-architecture",
                    ArtifactKind::Document,
                    "docs/backend/latest/系统架构/系统架构详解.md",
                    "architecture",
                ),
                item(
                    "backend-business-overview",
                    ArtifactKind::Document,
                    "docs/backend/latest/业务/业务功能总览.md",
                    "business-overview",
                ),
                item(
                    "backend-index",
                    ArtifactKind::Document,
                    "docs/backend/latest/index.md",
                    "backend-index",
                ),
                item(
                    "product-index",
                    ArtifactKind::Document,
                    "docs/product/latest/index.md",
                    "product-index",
                ),
                item(
                    "test-index",
                    ArtifactKind::Document,
                    "docs/test/latest/index.md",
                    "test-index",
                ),
                item(
                    "rule-router",
                    ArtifactKind::Rule,
                    ".claude/rules/project/README.md",
                    "rule-router",
                ),
                item(
                    "auth-lifecycle",
                    ArtifactKind::Rule,
                    ".claude/rules/project/backend/auth-lifecycle.md",
                    "authentication-lifecycle",
                ),
                skill,
            ],
            exclusions: vec![],
        };
        for artifact in plan
            .artifacts
            .iter_mut()
            .filter(|artifact| artifact.kind == ArtifactKind::Document)
        {
            artifact.required_sections.push("待补信息".to_string());
        }
        for artifact in plan.artifacts.iter_mut().filter(|artifact| {
            matches!(
                artifact.id.as_str(),
                "backend-system-architecture"
                    | "backend-business-overview"
                    | "backend-index"
                    | "product-index"
                    | "test-index"
            )
        }) {
            if matches!(artifact.id.as_str(), "product-index" | "test-index") {
                artifact.layer = "common".into();
            }
            artifact
                .required_sections
                .extend(match artifact.id.as_str() {
                    "backend-system-architecture" => vec![
                        "目录".into(),
                        "架构总览".into(),
                        "分层架构设计".into(),
                        "模块架构详解".into(),
                    ],
                    "backend-business-overview" => vec![
                        "系统架构与模块划分".into(),
                        "业务能力总览".into(),
                        "接口全景索引".into(),
                    ],
                    "backend-index" => vec!["文档索引".into()],
                    "product-index" => vec!["产品资料索引".into()],
                    "test-index" => vec!["测试资料索引".into()],
                    _ => Vec::new(),
                });
        }
        for artifact in plan
            .artifacts
            .iter_mut()
            .filter(|artifact| artifact.kind == ArtifactKind::Rule)
        {
            artifact.required_sections =
                vec!["触发条件".into(), "验证方式".into(), "待补信息".into()];
        }
        let skill = plan
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.kind == ArtifactKind::Skill)
            .expect("skill remains present");
        skill.required_sections = vec![
            "触发条件".into(),
            "项目资源".into(),
            "执行步骤".into(),
            "完成 Gate".into(),
            "失败处理".into(),
            "待补信息".into(),
        ];
        plan
    }

    fn artifact_content(item: &ArtifactPlanItem) -> String {
        let evidence = "`src/lib.rs` 中的 `auth_service` 是已确认的认证入口。";
        match item.kind {
            ArtifactKind::Document => {
                let sections = item
                    .required_sections
                    .iter()
                    .map(|section| {
                        if section == "待补信息" {
                            format!("## {section}\n\n已证实认证入口；未发现其他可验证事实时不补默认值，后续读取真实源码补齐。")
                        } else {
                            format!("## {section}\n\n当前认证服务的模块边界、复用入口、风险与验证方式均以源码为准。")
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n\n");
                format!("# 项目工程事实\n\n{sections}\n\n{}", "当前认证服务的模块边界、复用入口、风险与验证方式均以源码为准。".repeat(8))
            }
            ArtifactKind::Rule => format!(
                "# 认证生命周期规则\n\n## 触发条件\n\n修改 `{evidence}` 对应入口或其调用方时执行。\n\n## 复用与禁止替代\n\n优先扩展现有 `auth_service`，禁止新建并行认证框架。\n\n## 验证方式\n\n运行 `cargo test`。\n\n## 待补信息\n\n尚未发现更多认证模块时，不补写推测性约束。\n\n{}",
                "本规则仅约束当前项目已经确认的认证生命周期和影响面。".repeat(8)
            ),
            ArtifactKind::Skill => format!(
                "---\nname: sample-auth-change-review\ndescription: 修改认证边界时使用。\n---\n\n# 认证变更审查\n\n## 触发条件\n\n修改 {evidence} 或其调用链时使用。\n\n## 项目资源\n\n先读取认证入口和现有测试。\n\n## 执行步骤\n\n1. 沿已确认入口检查调用链。\n2. 复用当前错误处理和测试结构。\n\n## 完成 Gate\n\n源码事实与测试结果一致。\n\n## 失败处理\n\n立即停止并保留真实失败信息。\n\n## 待补信息\n\n未发现的认证模块获得证据后再补充。\n\n{}",
                "该流程只处理当前项目已确认认证边界的高风险修改。".repeat(8)
            ),
        }
    }

    struct FakeRunner {
        plan: ArtifactPlan,
        calls: Vec<String>,
        documents_exit_non_zero: bool,
    }

    impl AgentRunner for FakeRunner {
        fn run(
            &mut self,
            _agent: &str,
            workspace: &Path,
            prompt: &str,
            heartbeat: &mut dyn FnMut(),
        ) -> Result<AgentRunOutcome, String> {
            heartbeat();
            let stage = ["plan", "documents", "rules", "skills"]
                .into_iter()
                .find(|stage| prompt.contains(&format!("阶段 `{stage}`")))
                .expect("known stage");
            self.calls.push(stage.to_string());
            if stage == "plan" {
                let path = workspace.join(".vibe-coding-platform/artifact-plan.json");
                fs::create_dir_all(path.parent().expect("plan parent")).expect("plan parent");
                fs::write(
                    path,
                    serde_json::to_vec_pretty(&self.plan).expect("plan json"),
                )
                .expect("write plan");
            } else {
                let kind = match stage {
                    "documents" => ArtifactKind::Document,
                    "rules" => ArtifactKind::Rule,
                    "skills" => ArtifactKind::Skill,
                    _ => unreachable!(),
                };
                for item in self.plan.artifacts.iter().filter(|item| item.kind == kind) {
                    let path = workspace.join(&item.target_path);
                    fs::create_dir_all(path.parent().expect("artifact parent"))
                        .expect("artifact parent");
                    fs::write(path, artifact_content(item)).expect("write artifact");
                }
            }
            if stage == "documents" && self.documents_exit_non_zero {
                Ok(AgentRunOutcome::non_zero(9, "simulated nonzero"))
            } else {
                Ok(AgentRunOutcome::success())
            }
        }
    }

    fn seed_skills_ready_state(
        root: &Path,
        inventory: &ProjectInventory,
        plan: &ArtifactPlan,
        run_id: &str,
    ) -> (std::path::PathBuf, std::path::PathBuf) {
        let state_dir = crate::project_factory::initialization_state::state_directory(root)
            .expect("state directory");
        let workspace = state_dir.join(format!("workspace-{run_id}"));
        fs::create_dir_all(&state_dir).expect("state root");
        crate::project_factory::inventory::create_filtered_workspace(root, &workspace, inventory)
            .expect("workspace");
        let plan_path = workspace.join(".vibe-coding-platform/artifact-plan.json");
        fs::create_dir_all(plan_path.parent().expect("plan parent")).expect("plan parent");
        fs::write(
            plan_path,
            serde_json::to_vec_pretty(plan).expect("plan json"),
        )
        .expect("plan file");
        for item in &plan.artifacts {
            let path = workspace.join(&item.target_path);
            fs::create_dir_all(path.parent().expect("artifact parent")).expect("artifact parent");
            fs::write(path, artifact_content(item)).expect("artifact");
        }
        super::save_inventory_snapshot(root, inventory).expect("inventory snapshot");
        let now = super::unix_time_ms();
        let state = InitializationState {
            schema_version:
                crate::project_factory::initialization_state::INITIALIZATION_STATE_SCHEMA_VERSION,
            run_id: run_id.to_string(),
            state: InitializationRunState::SkillsReady,
            workspace_path: workspace.to_string_lossy().to_string(),
            inventory_sha256: Some(super::inventory_hash(inventory).expect("inventory hash")),
            plan_sha256: Some(super::plan_hash(plan).expect("plan hash")),
            artifact_totals: crate::project_factory::artifact_plan::artifact_totals(plan),
            checkpoints: vec![crate::project_factory::types::InitializationCheckpoint {
                state: InitializationRunState::SkillsReady,
                artifact_totals: crate::project_factory::artifact_plan::artifact_totals(plan),
                completed_at_unix_ms: now,
            }],
            started_at_unix_ms: now,
            updated_at_unix_ms: now,
            ..InitializationState::default()
        };
        crate::project_factory::initialization_state::save_initialization_state(root, &state)
            .expect("save skills-ready state");
        (state_dir, workspace)
    }

    #[test]
    fn v4_plan_prompt_is_schema_driven_and_project_specific() {
        let prompt = build_v4_stage_prompt(InitializationStage::Plan, &inventory(), None, &[]);

        assert!(prompt.contains("artifact-plan.json"));
        assert!(prompt.contains("schemaVersion"));
        assert!(prompt.contains("targetPath"));
        assert!(prompt.contains("requiredSections"));
        assert!(prompt.contains("English ASCII kebab-case"));
        assert!(prompt.contains("`.claude/rules/project/"));
        assert!(prompt.contains("path + symbol"));
        assert!(prompt.contains("复用优先"));
        assert!(prompt.contains("前端证据"));
        assert!(prompt.contains("后端证据"));
        assert!(prompt.contains("数据库证据"));
        assert!(prompt.contains("第三方集成证据"));
        assert!(prompt.contains("严禁臆想"));
        assert!(prompt.contains("allowedCommands"));
        assert!(prompt.contains("通用 IPS 模板由平台原样安装"));
        assert!(prompt.contains("docs/backend/latest/接口文档/API接口总览.md"));
        assert!(prompt.contains("业务功能总览.md"));
        assert!(prompt.contains("规范约束"));
    }

    #[test]
    fn v4_plan_prompt_declares_the_exact_validator_contract() {
        let prompt = build_v4_stage_prompt(InitializationStage::Plan, &inventory(), None, &[]);

        assert!(prompt.contains("document-template-library.md"));
        assert!(prompt.contains(".claude/rules/project/<english-kebab-case>.md"));
        assert!(prompt.contains(".claude/rules/project/README.md"));
        assert!(prompt.contains(".claude/skills/<project-specific-kebab-case>/SKILL.md"));
        assert!(prompt.contains("禁止创建 `docs/ai/`"));
        assert!(prompt.contains("物理模型总览.md"));
        assert!(prompt.contains("枚举值总览.md"));
        assert!(prompt.contains("common | contract | frontend | backend | database | integration"));
        assert!(prompt.contains("真实 declaration 或 configuration key"));
        assert!(prompt.contains("调用表达式、注释、字符串或推测的名称"));
        assert!(prompt.contains("项目资源"));
        assert!(prompt.contains("全部内嵌在 SKILL.md"));
    }

    #[test]
    fn v4_plan_prompt_enumerates_exact_inventory_coverage_values() {
        let mut inventory = inventory();
        inventory.modules.push(ProjectModule {
            name: "billing-service".to_string(),
            path: "services/billing".to_string(),
            kind: "backend".to_string(),
            manifests: vec!["services/billing/Cargo.toml".to_string()],
            source_roots: vec!["services/billing/src".to_string()],
        });
        inventory
            .source_roots
            .push("services/billing/src".to_string());

        let prompt = build_v4_stage_prompt(InitializationStage::Plan, &inventory, None, &[]);

        assert!(prompt.contains("module.name exact values: [\"service\", \"billing-service\"]"));
        assert!(prompt.contains("module.path exact values: [\".\", \"services/billing\"]"));
        assert!(prompt.contains("sourceRoot exact values: [\"src\", \"services/billing/src\"]"));
        assert!(prompt.contains("covers 只能逐字复制上述 exact values"));
        assert!(prompt.contains("每个 module 和每个 sourceRoot"));
        assert!(prompt.contains("同一 module/sourceRoot 内的 evidence path"));
        assert!(prompt.contains("exclusions.target 也只能逐字复制上述 exact values"));
        assert!(prompt.contains("document-templates.json"));
        assert!(prompt.contains("api-contracts"));
        assert!(prompt.contains("待补信息"));
        assert!(!prompt.contains("project-specific-capability"));
    }

    #[test]
    fn generation_prompt_contains_only_the_matching_planned_kind() {
        let mut plan = plan();
        plan.artifacts.push(ArtifactPlanItem {
            id: "project-map".to_string(),
            kind: ArtifactKind::Document,
            layer: "common".to_string(),
            topic: "project-map".to_string(),
            target_path: "docs/ai/project-map.md".to_string(),
            rationale: "项目结构证据".to_string(),
            evidence: vec![],
            covers: vec!["project-map".to_string()],
            required_sections: vec!["项目边界".to_string()],
        });

        let prompt = build_v4_stage_prompt(
            InitializationStage::Documents,
            &inventory(),
            Some(&plan),
            &[],
        );

        assert!(prompt.contains("docs/ai/project-map.md"));
        assert!(!prompt.contains(".claude/rules/project/auth-boundary.md"));
        assert!(prompt.contains("只允许编辑本阶段 JSON 中的 targetPath"));
    }

    #[test]
    fn user_intent_never_leaks_the_original_project_path_into_agent_prompts() {
        let root = "/Users/example/private/sample-service";
        let intent = format!("请初始化 {root}，项目路径还是 {root}/src");
        let sanitized = sanitize_user_intent(&intent, root);

        assert!(!sanitized.contains(root));
        assert!(sanitized.contains("<project-root>"));
    }

    #[test]
    fn staged_validation_takes_precedence_over_agent_exit_code() {
        let success = AgentRunOutcome::success();
        let non_zero = AgentRunOutcome::non_zero(7, "agent stopped");
        let missing = ValidationIssue {
            code: "artifact.missing".to_string(),
            detail: "missing planned artifact".to_string(),
            path: Some("docs/ai/project-map.md".to_string()),
            stage: Some("documents".to_string()),
        };

        assert_eq!(evaluate_agent_stage(&success, &[]), StageDecision::Advance);
        assert_eq!(
            evaluate_agent_stage(&non_zero, &[]),
            StageDecision::AdvanceWithWarning
        );
        assert_eq!(
            evaluate_agent_stage(&success, std::slice::from_ref(&missing)),
            StageDecision::Repair
        );
        assert_eq!(
            evaluate_agent_stage(&non_zero, &[missing]),
            StageDecision::Repair
        );
    }

    #[test]
    fn content_audit_does_not_block_initialization_but_workspace_safety_does() {
        let review_only = ValidationIssue {
            code: "artifact.section.missing".to_string(),
            detail: "missing a documentation section".to_string(),
            path: Some("docs/ai/project-map.md".to_string()),
            stage: Some("documents".to_string()),
        };
        let unsafe_workspace = ValidationIssue {
            code: "workspace.source.modified".to_string(),
            detail: "source snapshot changed".to_string(),
            path: Some("src/lib.rs".to_string()),
            stage: Some("documents".to_string()),
        };

        assert!(blocking_initialization_issues(&[review_only]).is_empty());
        assert_eq!(blocking_initialization_issues(&[unsafe_workspace]).len(), 1);
    }

    #[test]
    fn repair_is_bounded_and_stops_unchanged_issue_fingerprints_early() {
        let issues = vec![ValidationIssue {
            code: "artifact.section.missing".to_string(),
            detail: "missing verification section".to_string(),
            path: Some("docs/ai/project-map.md".to_string()),
            stage: Some("documents".to_string()),
        }];
        let mut tracker = RepairTracker::new(2);

        assert_eq!(tracker.observe(&issues, "digest-a"), RepairDecision::Retry);
        assert_eq!(
            tracker.observe(&issues, "digest-a"),
            RepairDecision::NoProgress
        );

        let mut tracker = RepairTracker::new(2);
        assert_eq!(tracker.observe(&issues, "digest-a"), RepairDecision::Retry);
        assert_eq!(tracker.observe(&issues, "digest-b"), RepairDecision::Retry);
        assert_eq!(
            tracker.observe(&issues, "digest-c"),
            RepairDecision::Exhausted
        );
    }

    #[test]
    fn resume_uses_last_valid_checkpoint_and_stale_process_becomes_interrupted() {
        assert_eq!(
            resume_stage(InitializationRunState::PlanReady),
            InitializationStage::Documents
        );
        assert_eq!(
            resume_stage(InitializationRunState::DocumentsReady),
            InitializationStage::Rules
        );
        assert_eq!(
            resume_stage(InitializationRunState::RulesReady),
            InitializationStage::Skills
        );

        let mut state = InitializationState {
            schema_version: 4,
            run_id: "run-1".to_string(),
            state: InitializationRunState::PlanReady,
            process_id: Some(123),
            ..InitializationState::default()
        };
        assert!(interrupt_stale_state(&mut state, 456));
        assert_eq!(state.state, InitializationRunState::Interrupted);
        assert!(state.process_id.is_none());
        assert!(state
            .issues
            .iter()
            .any(|issue| issue.code == "run.interrupted"));
    }

    #[test]
    fn stage_errors_are_aggregated_without_losing_actionable_codes() {
        let outcome = AgentRunOutcome::non_zero(9, "last diagnostic line");
        let issues = vec![
            ValidationIssue {
                code: "artifact.missing".to_string(),
                detail: "project map missing".to_string(),
                path: Some("docs/ai/project-map.md".to_string()),
                stage: Some("documents".to_string()),
            },
            ValidationIssue {
                code: "artifact.command.unknown".to_string(),
                detail: "invented command".to_string(),
                path: None,
                stage: Some("documents".to_string()),
            },
        ];

        let rendered = aggregate_stage_failure(InitializationStage::Documents, &issues, &outcome);
        assert!(rendered.contains("artifact.missing"));
        assert!(rendered.contains("project map missing"));
        assert!(rendered.contains("artifact.command.unknown"));
        assert!(rendered.contains("exit code 9"));
        assert!(rendered.contains("last diagnostic line"));
    }

    #[test]
    fn stage_scope_rejects_changes_outside_the_matching_plan_kind() {
        let plan = plan();
        let mut before = StageSurface::new();
        before.insert(
            "docs/ai/project-map.md".to_string(),
            "document-before".to_string(),
        );
        let mut after = before.clone();
        after.insert(
            "docs/ai/project-map.md".to_string(),
            "document-after".to_string(),
        );

        let issues = stage_scope_issues(&before, &after, InitializationStage::Rules, Some(&plan));

        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].code, "stage.scope.violation");
        assert_eq!(issues[0].path.as_deref(), Some("docs/ai/project-map.md"));
    }

    #[test]
    fn fake_runner_executes_all_v4_stages_and_accepts_valid_nonzero_exit() {
        let root = fixture("full-pipeline");
        let inventory =
            crate::project_factory::inventory::inspect_project(&root).expect("fixture inventory");
        let mut runner = FakeRunner {
            plan: complete_plan(&inventory),
            calls: Vec::new(),
            documents_exit_non_zero: true,
        };
        let state_dir = crate::project_factory::initialization_state::state_directory(&root)
            .expect("state directory");
        let mut progress = Vec::new();

        let result = initialize_with_runner(
            &root.to_string_lossy(),
            "codex",
            "根据项目真实证据初始化",
            &mut runner,
            |event| progress.push(event),
        )
        .expect("v4 initialization");

        assert_eq!(runner.calls, ["plan", "documents", "rules", "skills"]);
        assert_eq!(result.status, "current-v4");
        assert_eq!(result.phase, "complete");
        assert_eq!(result.artifact_totals.total, 8);
        assert!(result
            .warnings
            .iter()
            .any(|warning| warning.contains("非零") || warning.contains("退出")));
        assert_eq!(
            fs::read_to_string(root.join("src/lib.rs")).expect("source remains"),
            "pub fn auth_service() -> bool { true }\n"
        );
        assert!(root
            .join("docs/backend/latest/系统架构/系统架构详解.md")
            .is_file());
        assert!(root
            .join("docs/backend/latest/规范约束/详设文档模板.md")
            .is_file());
        assert!(root
            .join(".vibe-coding-platform/.initialization-manifest.json")
            .is_file());
        assert!(progress.iter().any(|event| event.phase == "complete"));
        let state = crate::project_factory::initialization_state::load_initialization_state(&root)
            .expect("load state")
            .expect("state exists");
        assert_eq!(state.state, InitializationRunState::Completed);
        assert!(state.workspace_path.is_empty());

        runner.calls.clear();
        let repeated = initialize_with_runner(
            &root.to_string_lossy(),
            "codex",
            "不应重新初始化",
            &mut runner,
            |_| {},
        )
        .expect("current v4 returns existing result");
        assert_eq!(repeated.run_id, result.run_id);
        assert!(runner.calls.is_empty(), "current-v4 must not run an agent");

        fs::remove_dir_all(&root).expect("cleanup fixture");
        let _ = fs::remove_dir_all(state_dir);
    }

    #[test]
    fn fake_runner_resumes_after_plan_checkpoint_without_replanning() {
        let root = fixture("resume");
        let inventory =
            crate::project_factory::inventory::inspect_project(&root).expect("fixture inventory");
        let plan = complete_plan(&inventory);
        let state_dir = crate::project_factory::initialization_state::state_directory(&root)
            .expect("state directory");
        let workspace = state_dir.join("workspace-resume-run");
        fs::create_dir_all(&state_dir).expect("state root");
        crate::project_factory::inventory::create_filtered_workspace(&root, &workspace, &inventory)
            .expect("workspace");
        let plan_path = workspace.join(".vibe-coding-platform/artifact-plan.json");
        fs::create_dir_all(plan_path.parent().expect("plan parent")).expect("plan parent");
        fs::write(
            plan_path,
            serde_json::to_vec_pretty(&plan).expect("plan json"),
        )
        .expect("plan file");
        let now = super::unix_time_ms();
        super::save_inventory_snapshot(&root, &inventory).expect("inventory snapshot");
        let mut state = InitializationState {
            schema_version:
                crate::project_factory::initialization_state::INITIALIZATION_STATE_SCHEMA_VERSION,
            run_id: "resume-run".to_string(),
            state: InitializationRunState::PlanReady,
            workspace_path: workspace.to_string_lossy().to_string(),
            process_id: Some(std::process::id().saturating_add(10_000)),
            inventory_sha256: Some(super::inventory_hash(&inventory).expect("inventory hash")),
            plan_sha256: Some(super::plan_hash(&plan).expect("plan hash")),
            artifact_totals: crate::project_factory::artifact_plan::artifact_totals(&plan),
            checkpoints: vec![crate::project_factory::types::InitializationCheckpoint {
                state: InitializationRunState::PlanReady,
                artifact_totals: crate::project_factory::artifact_plan::artifact_totals(&plan),
                completed_at_unix_ms: now,
            }],
            started_at_unix_ms: now,
            updated_at_unix_ms: now,
            ..InitializationState::default()
        };
        crate::project_factory::initialization_state::save_initialization_state(&root, &state)
            .expect("save interrupted state");
        let mut runner = FakeRunner {
            plan,
            calls: Vec::new(),
            documents_exit_non_zero: false,
        };

        let result = initialize_with_runner(
            &root.to_string_lossy(),
            "claude",
            "恢复初始化",
            &mut runner,
            |_| {},
        )
        .expect("resume initialization");

        assert_eq!(runner.calls, ["documents", "rules", "skills"]);
        assert_eq!(result.status, "current-v4");
        state = crate::project_factory::initialization_state::load_initialization_state(&root)
            .expect("load completed state")
            .expect("completed state");
        assert_eq!(state.state, InitializationRunState::Completed);
        assert!(state
            .checkpoints
            .iter()
            .any(|checkpoint| checkpoint.state == InitializationRunState::PlanReady));

        fs::remove_dir_all(&root).expect("cleanup fixture");
        let _ = fs::remove_dir_all(state_dir);
    }

    #[test]
    fn installing_resume_accepts_only_journal_managed_live_changes() {
        let root = fixture("install-resume");
        let inventory =
            crate::project_factory::inventory::inspect_project(&root).expect("fixture inventory");
        let plan = complete_plan(&inventory);
        let (state_dir, workspace) =
            seed_skills_ready_state(&root, &inventory, &plan, "install-resume-run");
        let mut state =
            crate::project_factory::initialization_state::load_initialization_state(&root)
                .expect("state")
                .expect("state exists");
        state.state = InitializationRunState::Installing;
        crate::project_factory::initialization_state::save_initialization_state(&root, &state)
            .expect("installing state");
        let mut manifest = crate::project_factory::initialization_state::install_planned_artifacts(
            &root, &workspace, &plan, None,
        )
        .expect("partial artifact install");
        crate::project_factory::initialization_state::install_managed_entries(&root, &mut manifest)
            .expect("partial entry install");
        assert!(root
            .join("docs/backend/latest/系统架构/系统架构详解.md")
            .is_file());
        assert!(root.join("CLAUDE.md").is_file());
        let mut runner = FakeRunner {
            plan,
            calls: Vec::new(),
            documents_exit_non_zero: false,
        };

        let result = initialize_with_runner(
            &root.to_string_lossy(),
            "codex",
            "恢复安装",
            &mut runner,
            |_| {},
        )
        .expect("journal-managed changes are recoverable");

        assert_eq!(result.status, "current-v4");
        assert!(runner.calls.is_empty());
        assert_eq!(
            fs::read_to_string(root.join("src/lib.rs")).expect("source"),
            "pub fn auth_service() -> bool { true }\n"
        );
        fs::remove_dir_all(&root).expect("cleanup fixture");
        let _ = fs::remove_dir_all(state_dir);
    }

    #[test]
    fn installing_resume_rejects_unmanaged_source_changes() {
        let root = fixture("install-source-conflict");
        let inventory =
            crate::project_factory::inventory::inspect_project(&root).expect("fixture inventory");
        let plan = complete_plan(&inventory);
        let (state_dir, _) =
            seed_skills_ready_state(&root, &inventory, &plan, "source-conflict-run");
        fs::write(
            root.join("src/lib.rs"),
            "pub fn auth_service() -> bool { false }\n",
        )
        .expect("external source edit");
        let mut runner = FakeRunner {
            plan,
            calls: Vec::new(),
            documents_exit_non_zero: false,
        };

        let error = initialize_with_runner(
            &root.to_string_lossy(),
            "claude",
            "恢复安装",
            &mut runner,
            |_| {},
        )
        .expect_err("unmanaged source changes must conflict");

        assert!(error.contains("发生变化"));
        assert!(runner.calls.is_empty());
        let state = crate::project_factory::initialization_state::load_initialization_state(&root)
            .expect("state")
            .expect("state exists");
        assert_eq!(state.state, InitializationRunState::Conflict);
        fs::remove_dir_all(&root).expect("cleanup fixture");
        let _ = fs::remove_dir_all(state_dir);
    }
}
