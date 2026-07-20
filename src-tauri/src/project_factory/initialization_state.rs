use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use super::artifact_plan::artifact_totals;
use super::inventory::content_sha256;
use super::types::{
    AgentAssetMode, AgentAssetTarget, ArtifactKind, ArtifactPlan, InitializationCheckpoint,
    InitializationRunState, InitializationState, ManagedAgentAsset, ManagedEntryOwnership,
    OwnedArtifact, OwnershipManifest, ValidationIssue,
};

pub const INITIALIZATION_STATE_SCHEMA_VERSION: u32 = 4;
pub const MANAGED_BLOCK_START: &str = "<!-- vibe-coding-platform:init:v4:start -->";
pub const MANAGED_BLOCK_END: &str = "<!-- vibe-coding-platform:init:v4:end -->";

const STATE_FILE: &str = "state.json";
const INSTALL_JOURNAL_FILE: &str = "install-journal.json";
const OWNERSHIP_MANIFEST_PATH: &str = "docs/ai/.initialization-manifest.json";
const AGENT_ASSET_NAMES: &[&str] = &["rules", "skills", "scripts"];

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
struct InstallCandidate {
    path: String,
    kind: ArtifactKind,
    bytes: Vec<u8>,
    sha256: String,
    baseline_sha256: Option<String>,
}

#[derive(Debug, Clone)]
struct RemovalCandidate {
    path: String,
    baseline_sha256: String,
    operation: JournalOperation,
}

#[derive(Debug, Clone)]
struct LinkRemovalCandidate {
    path: String,
    expected_target: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum JournalOperation {
    WriteArtifact,
    WriteManagedEntry,
    WriteAgentCopy,
    RemoveOwnedArtifact,
    RemoveAgentCopy,
    RemoveAgentLink,
    CreateAgentLink,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum JournalEntryState {
    Pending,
    Applied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallJournalEntry {
    operation: JournalOperation,
    #[serde(default)]
    baseline_sha256: Option<String>,
    #[serde(default)]
    expected_sha256: Option<String>,
    state: JournalEntryState,
    #[serde(default)]
    link_target: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallJournal {
    schema_version: u32,
    plan_sha256: String,
    #[serde(default)]
    entries: BTreeMap<String, InstallJournalEntry>,
}

#[derive(Debug)]
struct EntryCandidate {
    path: String,
    bytes: Vec<u8>,
    block_sha256: String,
    baseline_sha256: Option<String>,
    expected_sha256: String,
}

#[derive(Debug)]
struct CopyCandidate {
    path: String,
    bytes: Vec<u8>,
    sha256: String,
    baseline_sha256: Option<String>,
}

fn issue(
    code: &str,
    detail: impl Into<String>,
    path: Option<&str>,
    stage: &str,
) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        detail: detail.into(),
        path: path.map(str::to_string),
        stage: Some(stage.to_string()),
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn canonical_directory(path: &Path, label: &str) -> Result<PathBuf, String> {
    let canonical = fs::canonicalize(path)
        .map_err(|error| format!("无法解析{label}目录 {}：{error}", path.display()))?;
    let metadata = fs::metadata(&canonical)
        .map_err(|error| format!("无法读取{label}目录 {}：{error}", canonical.display()))?;
    if !metadata.is_dir() {
        return Err(format!("{label}路径不是目录：{}", canonical.display()));
    }
    Ok(canonical)
}

fn normalized_relative_path(value: &str) -> Result<PathBuf, String> {
    if value.is_empty() || value.contains('\\') {
        return Err("路径为空或包含反斜杠".to_string());
    }
    let path = Path::new(value);
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("路径不是安全的相对路径".to_string());
    }
    Ok(path.to_path_buf())
}

fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    false
}

fn allowed_install_target(kind: ArtifactKind, target_path: &str) -> bool {
    match kind {
        ArtifactKind::Document => {
            target_path.starts_with("docs/ai/") && target_path.ends_with(".md")
        }
        ArtifactKind::Rule => {
            target_path.starts_with(".claude/rules/project/") && target_path.ends_with(".md")
        }
        ArtifactKind::Skill => {
            target_path.starts_with(".claude/skills/")
                && target_path.ends_with("/SKILL.md")
                && target_path.split('/').count() >= 4
        }
    }
}

fn allowed_agent_asset_path(target_path: &str) -> bool {
    normalized_relative_path(target_path).is_ok()
        && AGENT_ASSET_NAMES.iter().any(|name| {
            let prefix = format!(".agents/{name}/");
            target_path.starts_with(&prefix) && target_path.len() > prefix.len()
        })
}

fn validate_manifest_structure(manifest: &OwnershipManifest) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let mut artifact_paths = BTreeSet::new();
    for artifact in &manifest.artifacts {
        if normalized_relative_path(&artifact.path).is_err()
            || !allowed_install_target(artifact.kind, &artifact.path)
        {
            issues.push(issue(
                "manifest.artifact.path-invalid",
                "产物路径不在其 ArtifactKind 允许的受管根目录中",
                Some(&artifact.path),
                "verify",
            ));
        }
        if !artifact_paths.insert(artifact.path.as_str()) {
            issues.push(issue(
                "manifest.path.duplicate",
                "所有权 manifest 包含重复产物路径",
                Some(&artifact.path),
                "verify",
            ));
        }
    }

    let mut asset_paths = BTreeSet::new();
    for asset in &manifest.agent_assets {
        if !allowed_agent_asset_path(&asset.path) {
            issues.push(issue(
                "manifest.agent-asset.path-invalid",
                "智能体副本路径必须位于 .agents/rules、skills 或 scripts 下",
                Some(&asset.path),
                "verify",
            ));
        }
        if !asset_paths.insert(asset.path.as_str()) {
            issues.push(issue(
                "manifest.agent-asset.duplicate",
                "所有权 manifest 包含重复智能体副本路径",
                Some(&asset.path),
                "verify",
            ));
        }
    }

    let mut entry_paths = BTreeSet::new();
    for entry in &manifest.managed_entries {
        if !matches!(entry.path.as_str(), "CLAUDE.md" | "AGENTS.md") {
            issues.push(issue(
                "manifest.entry.path-invalid",
                "入口托管块只能记录 CLAUDE.md 或 AGENTS.md",
                Some(&entry.path),
                "verify",
            ));
        }
        if !entry_paths.insert(entry.path.as_str()) {
            issues.push(issue(
                "manifest.entry.duplicate",
                "入口托管块在 manifest 中重复",
                Some(&entry.path),
                "verify",
            ));
        }
    }

    let mut target_paths = BTreeSet::new();
    for target in &manifest.agent_asset_targets {
        let matching_name = AGENT_ASSET_NAMES.iter().find(|name| {
            target.path == format!(".agents/{name}")
                && target.source_path == format!(".claude/{name}")
        });
        let link_is_valid = match (matching_name, target.mode) {
            (Some(name), AgentAssetMode::RelativeSymlink) => {
                target.link_target.as_deref()
                    == Some(expected_agent_link(name).to_string_lossy().as_ref())
            }
            (Some(_), AgentAssetMode::ManagedCopy | AgentAssetMode::Preserved) => {
                target.link_target.is_none()
            }
            _ => false,
        };
        if !link_is_valid {
            issues.push(issue(
                "manifest.agent-target.path-invalid",
                "智能体目标必须是匹配的 .claude/{rules,skills,scripts} 到 .agents 记录",
                Some(&target.path),
                "verify",
            ));
        }
        if !target_paths.insert(target.path.as_str()) {
            issues.push(issue(
                "manifest.agent-target.duplicate",
                "智能体目标在 manifest 中重复",
                Some(&target.path),
                "verify",
            ));
        }
    }
    issues
}

pub fn state_directory(project: &Path) -> Result<PathBuf, String> {
    let canonical = canonical_directory(project, "项目")?;
    let project_hash = content_sha256(&path_identity_bytes(&canonical));
    let base = dirs::data_local_dir().ok_or_else(|| "无法确定应用本地数据目录".to_string())?;
    Ok(base
        .join("vibe-coding-platform")
        .join("project-initialization")
        .join(project_hash))
}

#[cfg(unix)]
fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::unix::ffi::OsStrExt;

    path.as_os_str().as_bytes().to_vec()
}

#[cfg(windows)]
fn path_identity_bytes(path: &Path) -> Vec<u8> {
    use std::os::windows::ffi::OsStrExt;

    path.as_os_str()
        .encode_wide()
        .flat_map(u16::to_le_bytes)
        .collect()
}

#[cfg(not(any(unix, windows)))]
fn path_identity_bytes(path: &Path) -> Vec<u8> {
    path.to_string_lossy().as_bytes().to_vec()
}

pub fn load_initialization_state(project: &Path) -> Result<Option<InitializationState>, String> {
    let path = state_directory(project)?.join(STATE_FILE);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("无法读取初始化状态 {}：{error}", path.display())),
    };
    let state: InitializationState = serde_json::from_slice(&bytes)
        .map_err(|error| format!("初始化状态 JSON 无法解析：{error}"))?;
    if state.schema_version != INITIALIZATION_STATE_SCHEMA_VERSION {
        return Err(format!(
            "不支持的初始化状态 schemaVersion={}，当前仅支持 {}",
            state.schema_version, INITIALIZATION_STATE_SCHEMA_VERSION
        ));
    }
    Ok(Some(state))
}

pub fn save_initialization_state(
    project: &Path,
    state: &InitializationState,
) -> Result<(), String> {
    if state.schema_version != INITIALIZATION_STATE_SCHEMA_VERSION {
        return Err(format!(
            "拒绝写入 schemaVersion={} 的初始化状态",
            state.schema_version
        ));
    }
    let bytes = serde_json::to_vec_pretty(state)
        .map_err(|error| format!("无法序列化初始化状态：{error}"))?;
    atomic_write(&state_directory(project)?.join(STATE_FILE), &bytes)
}

fn plan_sha256(plan: &ArtifactPlan) -> Result<String, Vec<ValidationIssue>> {
    serde_json::to_vec(plan)
        .map(|bytes| content_sha256(&bytes))
        .map_err(|error| {
            vec![issue(
                "install.plan.serialize",
                format!("无法序列化产物计划：{error}"),
                None,
                "install",
            )]
        })
}

fn journal_path(project: &Path) -> Result<PathBuf, String> {
    Ok(state_directory(project)?.join(INSTALL_JOURNAL_FILE))
}

fn load_install_journal(project: &Path) -> Result<Option<InstallJournal>, ValidationIssue> {
    let path = journal_path(project)
        .map_err(|error| issue("install.journal.path", error, None, "install"))?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(issue(
                "install.journal.read",
                format!("无法读取安装恢复日志：{error}"),
                None,
                "install",
            ));
        }
    };
    let journal: InstallJournal = serde_json::from_slice(&bytes).map_err(|error| {
        issue(
            "install.journal.corrupt",
            format!("安装恢复日志 JSON 无法解析：{error}"),
            None,
            "install",
        )
    })?;
    if journal.schema_version != INITIALIZATION_STATE_SCHEMA_VERSION {
        return Err(issue(
            "install.journal.schema",
            format!(
                "安装恢复日志 schemaVersion={} 不受支持",
                journal.schema_version
            ),
            None,
            "install",
        ));
    }
    Ok(Some(journal))
}

fn save_install_journal(project: &Path, journal: &InstallJournal) -> Result<(), ValidationIssue> {
    let bytes = serde_json::to_vec_pretty(journal).map_err(|error| {
        issue(
            "install.journal.serialize",
            format!("无法序列化安装恢复日志：{error}"),
            None,
            "install",
        )
    })?;
    atomic_write(
        &journal_path(project)
            .map_err(|error| issue("install.journal.path", error, None, "install"))?,
        &bytes,
    )
    .map_err(|error| issue("install.journal.write", error, None, "install"))
}

fn journal_for_plan(project: &Path, plan_sha256: &str) -> Result<InstallJournal, ValidationIssue> {
    match load_install_journal(project)? {
        Some(journal) if journal.plan_sha256 == plan_sha256 => Ok(journal),
        Some(_) => Err(issue(
            "install.journal.plan-mismatch",
            "存在另一份未完成计划的安装恢复日志",
            None,
            "install",
        )),
        None => Ok(InstallJournal {
            schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
            plan_sha256: plan_sha256.to_string(),
            entries: BTreeMap::new(),
        }),
    }
}

fn source_bytes(
    workspace: &Path,
    relative: &Path,
    display_path: &str,
) -> Result<Vec<u8>, ValidationIssue> {
    reject_symlink_components(workspace, relative).map_err(|detail| {
        issue(
            "install.source.symlink",
            detail,
            Some(display_path),
            "install",
        )
    })?;
    let path = workspace.join(relative);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        issue(
            "install.source.missing",
            format!("无法读取计划中的暂存产物：{error}"),
            Some(display_path),
            "install",
        )
    })?;
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_file() {
        return Err(issue(
            "install.source.symlink",
            "暂存产物必须是工作区内的普通文件",
            Some(display_path),
            "install",
        ));
    }
    fs::read(&path).map_err(|error| {
        issue(
            "install.source.read",
            format!("无法读取暂存产物：{error}"),
            Some(display_path),
            "install",
        )
    })
}

fn reject_symlink_components(root: &Path, relative: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err("路径包含目录穿越".to_string());
        };
        current.push(name);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata_is_link_or_reparse(&metadata) => {
                return Err(format!("路径组件是软链接：{}", current.display()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(format!("无法验证路径组件 {}：{error}", current.display())),
        }
    }
    Ok(())
}

fn validate_target_ancestors(root: &Path, relative: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    let component_count = relative.components().count();
    for (index, component) in relative.components().enumerate() {
        let Component::Normal(name) = component else {
            return Err("目标路径包含目录穿越".to_string());
        };
        current.push(name);
        let is_target = index + 1 == component_count;
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata_is_link_or_reparse(&metadata) => {
                return Err(format!("目标路径组件是软链接：{}", current.display()));
            }
            Ok(metadata) if !is_target && !metadata.is_dir() => {
                return Err(format!("目标父路径不是目录：{}", current.display()));
            }
            Ok(metadata) if is_target && !metadata.is_file() => {
                return Err(format!("目标不是普通文件：{}", current.display()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(format!("无法验证目标路径 {}：{error}", current.display())),
        }
    }
    Ok(())
}

pub fn install_planned_artifacts(
    project: &Path,
    workspace: &Path,
    plan: &ArtifactPlan,
    previous: Option<&OwnershipManifest>,
) -> Result<OwnershipManifest, Vec<ValidationIssue>> {
    let project = canonical_directory(project, "项目")
        .map_err(|error| vec![issue("install.project.invalid", error, None, "install")])?;
    let workspace = canonical_directory(workspace, "暂存工作区")
        .map_err(|error| vec![issue("install.workspace.invalid", error, None, "install")])?;
    let plan_sha256 = plan_sha256(plan)?;
    let mut issues = Vec::new();
    let mut seen = BTreeSet::new();
    let mut candidates = Vec::new();
    let mut removals = Vec::new();
    let initialization_state = match load_initialization_state(&project) {
        Ok(state) => state,
        Err(error) => {
            issues.push(issue("install.state.read", error, None, "install"));
            None
        }
    };

    if let Some(previous) = previous {
        if previous.schema_version != INITIALIZATION_STATE_SCHEMA_VERSION
            || previous.state != InitializationRunState::Completed
        {
            issues.push(issue(
                "install.previous.incomplete",
                "只有 completed 的 v4 manifest 可以证明文件所有权",
                None,
                "install",
            ));
        }
        issues.extend(validate_manifest_structure(previous));
    }

    let journal = match load_install_journal(&project) {
        Ok(journal) => journal,
        Err(error) => {
            issues.push(error);
            None
        }
    };
    if journal
        .as_ref()
        .is_some_and(|journal| journal.plan_sha256 != plan_sha256)
    {
        issues.push(issue(
            "install.journal.plan-mismatch",
            "存在另一份未完成计划的安装恢复日志，必须先恢复或明确处理冲突",
            None,
            "install",
        ));
    }

    let previous_artifacts: BTreeMap<&str, &OwnedArtifact> = previous
        .filter(|manifest| {
            manifest.schema_version == INITIALIZATION_STATE_SCHEMA_VERSION
                && manifest.state == InitializationRunState::Completed
                && validate_manifest_structure(manifest).is_empty()
        })
        .into_iter()
        .flat_map(|manifest| manifest.artifacts.iter())
        .map(|artifact| (artifact.path.as_str(), artifact))
        .collect();

    for item in &plan.artifacts {
        if !seen.insert(item.target_path.as_str()) {
            issues.push(issue(
                "install.target.duplicate",
                "产物计划包含重复目标路径",
                Some(&item.target_path),
                "install",
            ));
            continue;
        }
        if !allowed_install_target(item.kind, &item.target_path) {
            issues.push(issue(
                "install.target.outside-allowlist",
                "产物目标不在其类型允许的安装目录",
                Some(&item.target_path),
                "install",
            ));
            continue;
        }
        let relative = match normalized_relative_path(&item.target_path) {
            Ok(relative) => relative,
            Err(detail) => {
                issues.push(issue(
                    "install.target.invalid",
                    detail,
                    Some(&item.target_path),
                    "install",
                ));
                continue;
            }
        };
        let bytes = match source_bytes(&workspace, &relative, &item.target_path) {
            Ok(bytes) => bytes,
            Err(error) => {
                issues.push(error);
                continue;
            }
        };
        let sha256 = content_sha256(&bytes);
        if let Err(detail) = validate_target_ancestors(&project, &relative) {
            issues.push(issue(
                "install.target.unsafe",
                detail,
                Some(&item.target_path),
                "install",
            ));
            continue;
        }
        let target = project.join(&relative);
        let baseline_sha256 = match fs::read(&target) {
            Ok(current) => {
                let current_hash = content_sha256(&current);
                let matching_journal_entry = journal.as_ref().and_then(|journal| {
                    (journal.plan_sha256 == plan_sha256)
                        .then(|| journal.entries.get(&item.target_path))
                        .flatten()
                });
                let journal_owns = matching_journal_entry.is_some_and(|entry| {
                    entry.operation == JournalOperation::WriteArtifact
                        && entry.expected_sha256.as_deref() == Some(sha256.as_str())
                        && (entry.expected_sha256.as_deref() == Some(current_hash.as_str())
                            || entry.baseline_sha256.as_deref() == Some(current_hash.as_str()))
                });
                let journal_contract_mismatch = matching_journal_entry.is_some_and(|entry| {
                    entry.operation != JournalOperation::WriteArtifact
                        || entry.expected_sha256.as_deref() != Some(sha256.as_str())
                });
                if journal_contract_mismatch {
                    issues.push(issue(
                        "install.journal.target-diverged",
                        "安装恢复日志与当前暂存产物或目标内容不一致",
                        Some(&item.target_path),
                        "install",
                    ));
                } else if journal_owns {
                    // A matching in-flight journal is newer than the last completed manifest.
                } else if let Some(owned) = previous_artifacts.get(item.target_path.as_str()) {
                    if current_hash != owned.sha256 {
                        issues.push(issue(
                            "install.target.modified",
                            "平台此前生成的文件已被修改，拒绝覆盖用户改动",
                            Some(&item.target_path),
                            "install",
                        ));
                    }
                } else {
                    issues.push(issue(
                        "install.target.unowned",
                        "目标文件没有 completed v4 manifest 所有权，拒绝覆盖",
                        Some(&item.target_path),
                        "install",
                    ));
                }
                Some(current_hash)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if let Some(entry) = journal.as_ref().and_then(|journal| {
                    (journal.plan_sha256 == plan_sha256)
                        .then(|| journal.entries.get(&item.target_path))
                        .flatten()
                }) {
                    if entry.operation != JournalOperation::WriteArtifact
                        || entry.expected_sha256.as_deref() != Some(sha256.as_str())
                    {
                        issues.push(issue(
                            "install.journal.target-diverged",
                            "安装恢复日志中的目标操作与本轮产物不一致",
                            Some(&item.target_path),
                            "install",
                        ));
                    }
                }
                None
            }
            Err(error) => {
                issues.push(issue(
                    "install.target.read",
                    format!("无法读取目标文件：{error}"),
                    Some(&item.target_path),
                    "install",
                ));
                None
            }
        };
        candidates.push(InstallCandidate {
            path: item.target_path.clone(),
            kind: item.kind,
            bytes,
            sha256,
            baseline_sha256,
        });
    }

    for owned in previous_artifacts.values() {
        if seen.contains(owned.path.as_str()) {
            continue;
        }
        let relative = match normalized_relative_path(&owned.path) {
            Ok(relative) => relative,
            Err(detail) => {
                issues.push(issue(
                    "install.removed-owned.invalid",
                    detail,
                    Some(&owned.path),
                    "install",
                ));
                continue;
            }
        };
        if let Err(detail) = validate_target_ancestors(&project, &relative) {
            issues.push(issue(
                "install.removed-owned.unsafe",
                detail,
                Some(&owned.path),
                "install",
            ));
            continue;
        }
        match fs::read(project.join(&relative)) {
            Ok(bytes) => {
                let current_hash = content_sha256(&bytes);
                if current_hash != owned.sha256 {
                    issues.push(issue(
                        "install.removed-owned.modified",
                        "旧计划中的平台产物已被修改，拒绝删除并继续由旧 manifest 记录所有权",
                        Some(&owned.path),
                        "install",
                    ));
                } else {
                    removals.push(RemovalCandidate {
                        path: owned.path.clone(),
                        baseline_sha256: current_hash,
                        operation: JournalOperation::RemoveOwnedArtifact,
                    });
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let pending_remove_matches = journal.as_ref().is_some_and(|journal| {
                    journal.plan_sha256 == plan_sha256
                        && journal.entries.get(&owned.path).is_some_and(|entry| {
                            entry.operation == JournalOperation::RemoveOwnedArtifact
                                && entry.state == JournalEntryState::Pending
                                && entry.baseline_sha256.as_deref() == Some(owned.sha256.as_str())
                        })
                });
                if pending_remove_matches {
                    removals.push(RemovalCandidate {
                        path: owned.path.clone(),
                        baseline_sha256: owned.sha256.clone(),
                        operation: JournalOperation::RemoveOwnedArtifact,
                    });
                }
            }
            Err(error) => issues.push(issue(
                "install.removed-owned.read",
                format!("无法读取待撤销的旧产物：{error}"),
                Some(&owned.path),
                "install",
            )),
        }
    }

    if !issues.is_empty() {
        return Err(issues);
    }

    let mut journal = journal.unwrap_or(InstallJournal {
        schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
        plan_sha256: plan_sha256.clone(),
        entries: BTreeMap::new(),
    });
    for candidate in &candidates {
        journal
            .entries
            .entry(candidate.path.clone())
            .or_insert_with(|| InstallJournalEntry {
                operation: JournalOperation::WriteArtifact,
                baseline_sha256: candidate.baseline_sha256.clone(),
                expected_sha256: Some(candidate.sha256.clone()),
                state: JournalEntryState::Pending,
                link_target: None,
            });
    }
    for candidate in &removals {
        journal.entries.insert(
            candidate.path.clone(),
            InstallJournalEntry {
                operation: candidate.operation,
                baseline_sha256: Some(candidate.baseline_sha256.clone()),
                expected_sha256: None,
                state: JournalEntryState::Pending,
                link_target: None,
            },
        );
    }
    save_install_journal(&project, &journal).map_err(|error| vec![error])?;

    for candidate in &candidates {
        let relative = normalized_relative_path(&candidate.path)
            .expect("preflight accepted normalized target path");
        validate_target_ancestors(&project, &relative).map_err(|detail| {
            vec![issue(
                "install.target.changed-during-install",
                detail,
                Some(&candidate.path),
                "install",
            )]
        })?;
        let target = project.join(&relative);
        let current_sha256 = match fs::read(&target) {
            Ok(bytes) => Some(content_sha256(&bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(vec![issue(
                    "install.target.changed-during-install",
                    format!("写入前无法重新读取目标文件：{error}"),
                    Some(&candidate.path),
                    "install",
                )]);
            }
        };
        let already_applied = current_sha256.as_deref() == Some(candidate.sha256.as_str());
        if !already_applied && current_sha256 != candidate.baseline_sha256 {
            return Err(vec![issue(
                "install.target.changed-during-install",
                "目标文件在预检后发生变化，已停止安装并保留恢复日志",
                Some(&candidate.path),
                "install",
            )]);
        }
        if !already_applied {
            atomic_write(&target, &candidate.bytes).map_err(|error| {
                vec![issue(
                    "install.target.write",
                    error,
                    Some(&candidate.path),
                    "install",
                )]
            })?;
        }
        if let Some(entry) = journal.entries.get_mut(&candidate.path) {
            entry.state = JournalEntryState::Applied;
        }
        save_install_journal(&project, &journal).map_err(|error| vec![error])?;
    }

    for candidate in &removals {
        let relative = normalized_relative_path(&candidate.path)
            .expect("preflight accepted normalized removal path");
        validate_target_ancestors(&project, &relative).map_err(|detail| {
            vec![issue(
                "install.removed-owned.changed-during-install",
                detail,
                Some(&candidate.path),
                "install",
            )]
        })?;
        let target = project.join(relative);
        match fs::read(&target) {
            Ok(bytes) if content_sha256(&bytes) == candidate.baseline_sha256 => {
                fs::remove_file(&target).map_err(|error| {
                    vec![issue(
                        "install.removed-owned.delete",
                        format!("无法删除已撤销的旧产物：{error}"),
                        Some(&candidate.path),
                        "install",
                    )]
                })?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(vec![issue(
                    "install.removed-owned.changed-during-install",
                    "旧产物在预检后发生变化，已停止删除",
                    Some(&candidate.path),
                    "install",
                )]);
            }
            Err(error) => {
                return Err(vec![issue(
                    "install.removed-owned.changed-during-install",
                    format!("删除前无法重新读取旧产物：{error}"),
                    Some(&candidate.path),
                    "install",
                )]);
            }
        }
        if let Some(entry) = journal.entries.get_mut(&candidate.path) {
            entry.state = JournalEntryState::Applied;
        }
        save_install_journal(&project, &journal).map_err(|error| vec![error])?;
    }

    let installed_at_unix_ms = unix_time_ms();
    let totals = artifact_totals(plan);
    let mut checkpoints = initialization_state
        .as_ref()
        .map(|state| state.checkpoints.clone())
        .or_else(|| previous.map(|manifest| manifest.checkpoints.clone()))
        .unwrap_or_default();
    checkpoints.push(InitializationCheckpoint {
        state: InitializationRunState::Installing,
        artifact_totals: totals,
        completed_at_unix_ms: installed_at_unix_ms,
    });
    Ok(OwnershipManifest {
        schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
        platform_version: env!("CARGO_PKG_VERSION").to_string(),
        run_id: initialization_state
            .as_ref()
            .map(|state| state.run_id.clone())
            .filter(|run_id| !run_id.is_empty())
            .or_else(|| previous.map(|manifest| manifest.run_id.clone()))
            .unwrap_or_default(),
        state: InitializationRunState::Installing,
        inventory_sha256: initialization_state
            .as_ref()
            .and_then(|state| state.inventory_sha256.clone())
            .or_else(|| previous.map(|manifest| manifest.inventory_sha256.clone()))
            .unwrap_or_default(),
        inventory_summary: previous.and_then(|manifest| manifest.inventory_summary),
        plan_sha256,
        artifact_totals: totals,
        artifacts: candidates
            .into_iter()
            .map(|candidate| OwnedArtifact {
                path: candidate.path,
                sha256: candidate.sha256,
                kind: candidate.kind,
            })
            .collect(),
        managed_entries: previous
            .map(|manifest| manifest.managed_entries.clone())
            .unwrap_or_default(),
        agent_assets: previous
            .map(|manifest| manifest.agent_assets.clone())
            .unwrap_or_default(),
        agent_asset_targets: previous
            .map(|manifest| manifest.agent_asset_targets.clone())
            .unwrap_or_default(),
        agent_asset_mode: previous.and_then(|manifest| manifest.agent_asset_mode),
        checkpoints,
        conflicts: Vec::new(),
        diagnostics: Vec::new(),
        started_at_unix_ms: initialization_state
            .as_ref()
            .map(|state| state.started_at_unix_ms)
            .filter(|timestamp| *timestamp > 0)
            .or_else(|| previous.map(|manifest| manifest.started_at_unix_ms))
            .unwrap_or_default(),
        installed_at_unix_ms,
        completed_at_unix_ms: 0,
    })
}

fn managed_block(manifest: &OwnershipManifest) -> String {
    let mut documents = Vec::new();
    let mut rules = Vec::new();
    let mut skills = Vec::new();
    for artifact in &manifest.artifacts {
        match artifact.kind {
            ArtifactKind::Document
                if artifact.path.ends_with("project-map.md") || documents.is_empty() =>
            {
                documents.push(artifact.path.as_str());
            }
            ArtifactKind::Rule if artifact.path.ends_with("/README.md") || rules.is_empty() => {
                rules.push(artifact.path.as_str());
            }
            ArtifactKind::Skill => skills.push(artifact.path.as_str()),
            _ => {}
        }
    }
    documents.sort_unstable();
    rules.sort_unstable();
    skills.sort_unstable();

    let mut lines = vec![
        MANAGED_BLOCK_START.to_string(),
        "## 项目工程上下文（初始化 v4）".to_string(),
        String::new(),
        "开始开发、修复、重构或评审前，先按任务读取以下项目专属资料；优先复用现有架构、模块与公共能力。"
            .to_string(),
    ];
    for (title, paths) in [
        ("长期文档", documents),
        ("项目规则", rules),
        ("项目技能", skills),
    ] {
        if paths.is_empty() {
            continue;
        }
        lines.push(String::new());
        lines.push(format!(
            "- {title}：{}",
            paths
                .iter()
                .map(|path| format!("`{path}`"))
                .collect::<Vec<_>>()
                .join("、")
        ));
    }
    lines.push(String::new());
    lines.push(
        "只使用仓库中可验证的命令；遇到文档与代码不一致时，以真实代码为证据并同步长期文档。"
            .to_string(),
    );
    lines.push(MANAGED_BLOCK_END.to_string());
    lines.join("\n")
}

fn managed_block_range(content: &str) -> Result<Option<(usize, usize)>, String> {
    let starts: Vec<_> = content.match_indices(MANAGED_BLOCK_START).collect();
    let ends: Vec<_> = content.match_indices(MANAGED_BLOCK_END).collect();
    if starts.is_empty() && ends.is_empty() {
        return Ok(None);
    }
    if starts.len() != 1 || ends.len() != 1 || starts[0].0 >= ends[0].0 {
        return Err("托管块标记缺失、重复或顺序错误".to_string());
    }
    Ok(Some((starts[0].0, ends[0].0 + MANAGED_BLOCK_END.len())))
}

fn preflight_entry(
    project: &Path,
    relative: &str,
    block: &str,
    previous: &BTreeMap<&str, &ManagedEntryOwnership>,
    journal: &InstallJournal,
) -> Result<EntryCandidate, ValidationIssue> {
    let path = project.join(relative);
    validate_target_ancestors(project, Path::new(relative))
        .map_err(|detail| issue("install.entry.unsafe", detail, Some(relative), "install"))?;
    let (existing, baseline_sha256) = match fs::read(&path) {
        Ok(bytes) => {
            let baseline_sha256 = Some(content_sha256(&bytes));
            let content = String::from_utf8(bytes).map_err(|_| {
                issue(
                    "install.entry.encoding",
                    "入口文件不是 UTF-8，无法安全插入托管块",
                    Some(relative),
                    "install",
                )
            })?;
            (content, baseline_sha256)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => (String::new(), None),
        Err(error) => {
            return Err(issue(
                "install.entry.read",
                format!("无法读取入口文件：{error}"),
                Some(relative),
                "install",
            ));
        }
    };
    let journal_proves_current = journal.entries.get(relative).is_some_and(|entry| {
        entry.operation == JournalOperation::WriteManagedEntry
            && entry.expected_sha256 == baseline_sha256
    });
    let output = match managed_block_range(&existing)
        .map_err(|detail| issue("install.entry.malformed", detail, Some(relative), "install"))?
    {
        Some((start, end)) => {
            let current_block = &existing[start..end];
            if !journal_proves_current {
                let owned = previous.get(relative).ok_or_else(|| {
                    issue(
                        "install.entry.unowned",
                        "入口文件包含没有 v4 manifest 所有权的托管标记",
                        Some(relative),
                        "install",
                    )
                })?;
                if content_sha256(current_block.as_bytes()) != owned.block_sha256 {
                    return Err(issue(
                        "install.entry.modified",
                        "入口文件中的平台托管块已被修改，拒绝覆盖",
                        Some(relative),
                        "install",
                    ));
                }
            }
            let mut output = String::with_capacity(existing.len() + block.len());
            output.push_str(&existing[..start]);
            output.push_str(block);
            output.push_str(&existing[end..]);
            output
        }
        None if existing.is_empty() => format!("{block}\n"),
        None => {
            let separator = if existing.ends_with("\n\n") {
                ""
            } else if existing.ends_with('\n') {
                "\n"
            } else {
                "\n\n"
            };
            format!("{existing}{separator}{block}\n")
        }
    };
    let expected_sha256 = content_sha256(output.as_bytes());
    if let Some(entry) = journal.entries.get(relative) {
        if entry.operation != JournalOperation::WriteManagedEntry
            || entry.expected_sha256.as_deref() != Some(expected_sha256.as_str())
        {
            return Err(issue(
                "install.entry.journal-diverged",
                "入口文件恢复日志与本轮期望内容不一致",
                Some(relative),
                "install",
            ));
        }
    }
    Ok(EntryCandidate {
        path: relative.to_string(),
        bytes: output.into_bytes(),
        block_sha256: content_sha256(block.as_bytes()),
        baseline_sha256,
        expected_sha256,
    })
}

pub fn install_managed_entries(
    project: &Path,
    manifest: &mut OwnershipManifest,
) -> Result<(), Vec<ValidationIssue>> {
    let project = canonical_directory(project, "项目")
        .map_err(|error| vec![issue("install.project.invalid", error, None, "install")])?;
    let mut journal =
        journal_for_plan(&project, &manifest.plan_sha256).map_err(|error| vec![error])?;
    let block = managed_block(manifest);
    let previous: BTreeMap<&str, &ManagedEntryOwnership> = manifest
        .managed_entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();
    let mut issues = Vec::new();
    let mut candidates = Vec::new();
    for path in ["CLAUDE.md", "AGENTS.md"] {
        match preflight_entry(&project, path, &block, &previous, &journal) {
            Ok(candidate) => candidates.push(candidate),
            Err(error) => issues.push(error),
        }
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    for candidate in &candidates {
        journal
            .entries
            .entry(candidate.path.clone())
            .or_insert_with(|| InstallJournalEntry {
                operation: JournalOperation::WriteManagedEntry,
                baseline_sha256: candidate.baseline_sha256.clone(),
                expected_sha256: Some(candidate.expected_sha256.clone()),
                state: JournalEntryState::Pending,
                link_target: None,
            });
    }
    save_install_journal(&project, &journal).map_err(|error| vec![error])?;
    for candidate in &candidates {
        let target = project.join(&candidate.path);
        validate_target_ancestors(&project, Path::new(&candidate.path)).map_err(|detail| {
            vec![issue(
                "install.entry.changed-during-install",
                detail,
                Some(&candidate.path),
                "install",
            )]
        })?;
        let current_sha256 = match fs::read(&target) {
            Ok(bytes) => Some(content_sha256(&bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(vec![issue(
                    "install.entry.changed-during-install",
                    format!("写入前无法重新读取入口文件：{error}"),
                    Some(&candidate.path),
                    "install",
                )]);
            }
        };
        let already_applied = current_sha256.as_deref() == Some(candidate.expected_sha256.as_str());
        if !already_applied && current_sha256 != candidate.baseline_sha256 {
            return Err(vec![issue(
                "install.entry.changed-during-install",
                "入口文件在预检后发生变化，已保留恢复日志",
                Some(&candidate.path),
                "install",
            )]);
        }
        if !already_applied {
            atomic_write(&target, &candidate.bytes).map_err(|error| {
                vec![issue(
                    "install.entry.write",
                    error,
                    Some(&candidate.path),
                    "install",
                )]
            })?;
        }
        if let Some(entry) = journal.entries.get_mut(&candidate.path) {
            entry.state = JournalEntryState::Applied;
        }
        save_install_journal(&project, &journal).map_err(|error| vec![error])?;
    }
    manifest.managed_entries = candidates
        .into_iter()
        .map(|candidate| ManagedEntryOwnership {
            path: candidate.path,
            block_sha256: candidate.block_sha256,
        })
        .collect();
    Ok(())
}

fn collect_agent_source_files(
    source_root: &Path,
    relative: &Path,
    output: &mut Vec<(PathBuf, Vec<u8>)>,
) -> Result<(), String> {
    let directory = source_root.join(relative);
    let mut entries = fs::read_dir(&directory)
        .map_err(|error| format!("无法读取 {}：{error}", directory.display()))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("无法枚举 {}：{error}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let metadata = fs::symlink_metadata(entry.path())
            .map_err(|error| format!("无法读取 {}：{error}", entry.path().display()))?;
        let child = relative.join(entry.file_name());
        if metadata_is_link_or_reparse(&metadata) {
            return Err(format!("智能体资源包含软链接：{}", entry.path().display()));
        }
        if metadata.is_dir() {
            collect_agent_source_files(source_root, &child, output)?;
        } else if metadata.is_file() {
            let bytes = fs::read(entry.path())
                .map_err(|error| format!("无法读取 {}：{error}", entry.path().display()))?;
            output.push((child, bytes));
        }
    }
    Ok(())
}

fn expected_agent_link(name: &str) -> PathBuf {
    PathBuf::from(format!("../.claude/{name}"))
}

fn preflight_agent_link_removal(
    project: &Path,
    target: &AgentAssetTarget,
    journal: &InstallJournal,
) -> Result<Option<LinkRemovalCandidate>, ValidationIssue> {
    if target.mode != AgentAssetMode::RelativeSymlink {
        return Ok(None);
    }
    let expected_target = target.link_target.clone().ok_or_else(|| {
        issue(
            "manifest.agent-target.path-invalid",
            "相对链接目标缺少 linkTarget",
            Some(&target.path),
            "install",
        )
    })?;
    reject_symlink_components(project, Path::new(".agents")).map_err(|detail| {
        issue(
            "install.agent-link.remove-unsafe",
            detail,
            Some(&target.path),
            "install",
        )
    })?;
    let destination = project.join(&target.path);
    match fs::symlink_metadata(&destination) {
        Ok(metadata) if metadata_is_link_or_reparse(&metadata) => {
            let actual = fs::read_link(&destination).map_err(|error| {
                issue(
                    "install.agent-link.remove-read",
                    format!("无法读取待撤销的智能体链接：{error}"),
                    Some(&target.path),
                    "install",
                )
            })?;
            if actual != Path::new(&expected_target) {
                return Err(issue(
                    "install.agent-link.remove-mismatch",
                    format!(
                        "待撤销链接指向 {}，不等于 manifest 记录的 {}",
                        actual.display(),
                        expected_target
                    ),
                    Some(&target.path),
                    "install",
                ));
            }
            Ok(Some(LinkRemovalCandidate {
                path: target.path.clone(),
                expected_target,
            }))
        }
        Ok(_) => Err(issue(
            "install.agent-link.remove-mismatch",
            "待撤销目标不再是平台创建的相对链接",
            Some(&target.path),
            "install",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let pending_remove_matches = journal.entries.get(&target.path).is_some_and(|entry| {
                entry.operation == JournalOperation::RemoveAgentLink
                    && entry.state == JournalEntryState::Pending
                    && entry.link_target.as_deref() == Some(expected_target.as_str())
            });
            Ok(pending_remove_matches.then(|| LinkRemovalCandidate {
                path: target.path.clone(),
                expected_target,
            }))
        }
        Err(error) => Err(issue(
            "install.agent-link.remove-read",
            format!("无法检查待撤销的智能体链接：{error}"),
            Some(&target.path),
            "install",
        )),
    }
}

fn preflight_agent_copy_removal(
    project: &Path,
    asset: &ManagedAgentAsset,
    journal: &InstallJournal,
) -> Result<Option<RemovalCandidate>, ValidationIssue> {
    if !allowed_agent_asset_path(&asset.path) {
        return Err(issue(
            "manifest.agent-asset.path-invalid",
            "智能体副本路径必须位于 .agents/rules、skills 或 scripts 下",
            Some(&asset.path),
            "install",
        ));
    }
    let relative = normalized_relative_path(&asset.path).map_err(|detail| {
        issue(
            "install.agent-copy.remove-invalid",
            detail,
            Some(&asset.path),
            "install",
        )
    })?;
    validate_target_ancestors(project, &relative).map_err(|detail| {
        issue(
            "install.agent-copy.remove-unsafe",
            detail,
            Some(&asset.path),
            "install",
        )
    })?;
    match fs::read(project.join(relative)) {
        Ok(bytes) => {
            let current_sha256 = content_sha256(&bytes);
            if current_sha256 != asset.sha256 {
                return Err(issue(
                    "install.agent-copy.modified",
                    "不再需要的同步副本已被修改，拒绝删除并继续由旧 manifest 记录所有权",
                    Some(&asset.path),
                    "install",
                ));
            }
            Ok(Some(RemovalCandidate {
                path: asset.path.clone(),
                baseline_sha256: current_sha256,
                operation: JournalOperation::RemoveAgentCopy,
            }))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let pending_remove_matches = journal.entries.get(&asset.path).is_some_and(|entry| {
                entry.operation == JournalOperation::RemoveAgentCopy
                    && entry.state == JournalEntryState::Pending
                    && entry.baseline_sha256.as_deref() == Some(asset.sha256.as_str())
            });
            Ok(pending_remove_matches.then(|| RemovalCandidate {
                path: asset.path.clone(),
                baseline_sha256: asset.sha256.clone(),
                operation: JournalOperation::RemoveAgentCopy,
            }))
        }
        Err(error) => Err(issue(
            "install.agent-copy.remove-read",
            format!("无法读取待撤销的同步副本：{error}"),
            Some(&asset.path),
            "install",
        )),
    }
}

pub fn share_agent_assets(
    project: &Path,
    manifest: &mut OwnershipManifest,
) -> Result<AgentAssetMode, Vec<ValidationIssue>> {
    share_agent_assets_for_test(project, manifest, cfg!(unix))
}

fn share_agent_assets_for_test(
    project: &Path,
    manifest: &mut OwnershipManifest,
    prefer_relative_symlinks: bool,
) -> Result<AgentAssetMode, Vec<ValidationIssue>> {
    let project = canonical_directory(project, "项目")
        .map_err(|error| vec![issue("install.project.invalid", error, None, "install")])?;
    let structural_issues = validate_manifest_structure(manifest);
    if !structural_issues.is_empty() {
        return Err(structural_issues);
    }
    let mut journal =
        journal_for_plan(&project, &manifest.plan_sha256).map_err(|error| vec![error])?;
    let previous_assets: BTreeMap<&str, &ManagedAgentAsset> = manifest
        .agent_assets
        .iter()
        .map(|asset| (asset.path.as_str(), asset))
        .collect();
    let previous_targets: BTreeMap<&str, &AgentAssetTarget> = manifest
        .agent_asset_targets
        .iter()
        .map(|target| (target.path.as_str(), target))
        .collect();
    let mut issues = Vec::new();
    let mut links = Vec::new();
    let mut copies = Vec::new();
    let mut removals = Vec::new();
    let mut link_removals = Vec::new();
    let mut preserved = false;
    let mut preserved_names = Vec::new();

    for name in AGENT_ASSET_NAMES {
        let source = project.join(".claude").join(name);
        if let Err(detail) =
            reject_symlink_components(&project, Path::new(&format!(".claude/{name}")))
        {
            issues.push(issue(
                "install.agent-source.unsafe",
                detail,
                Some(&format!(".claude/{name}")),
                "install",
            ));
            continue;
        }
        let source_metadata = match fs::symlink_metadata(&source) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let prefix = format!(".agents/{name}/");
                for asset in previous_assets
                    .values()
                    .filter(|asset| asset.path.starts_with(&prefix))
                {
                    match preflight_agent_copy_removal(&project, asset, &journal) {
                        Ok(Some(candidate)) => removals.push(candidate),
                        Ok(None) => {}
                        Err(error) => issues.push(error),
                    }
                }
                if let Some(target) = previous_targets.get(format!(".agents/{name}").as_str()) {
                    match preflight_agent_link_removal(&project, target, &journal) {
                        Ok(Some(candidate)) => link_removals.push(candidate),
                        Ok(None) => {}
                        Err(error) => issues.push(error),
                    }
                }
                continue;
            }
            Err(error) => {
                issues.push(issue(
                    "install.agent-source.read",
                    format!("无法读取智能体资源：{error}"),
                    Some(&format!(".claude/{name}")),
                    "install",
                ));
                continue;
            }
        };
        if metadata_is_link_or_reparse(&source_metadata) || !source_metadata.is_dir() {
            issues.push(issue(
                "install.agent-source.unsafe",
                "智能体资源根必须是普通目录",
                Some(&format!(".claude/{name}")),
                "install",
            ));
            continue;
        }
        let mut source_files = Vec::new();
        if let Err(detail) = collect_agent_source_files(&source, Path::new(""), &mut source_files) {
            issues.push(issue(
                "install.agent-source.unsafe",
                detail,
                Some(&format!(".claude/{name}")),
                "install",
            ));
            continue;
        }
        let desired_paths: BTreeSet<String> = source_files
            .iter()
            .map(|(relative, _)| {
                format!(
                    ".agents/{name}/{}",
                    relative.to_string_lossy().replace('\\', "/")
                )
            })
            .collect();
        let prefix = format!(".agents/{name}/");
        for asset in previous_assets
            .values()
            .filter(|asset| asset.path.starts_with(&prefix) && !desired_paths.contains(&asset.path))
        {
            match preflight_agent_copy_removal(&project, asset, &journal) {
                Ok(Some(candidate)) => removals.push(candidate),
                Ok(None) => {}
                Err(error) => issues.push(error),
            }
        }
        if source_files.is_empty() {
            if let Some(target) = previous_targets.get(format!(".agents/{name}").as_str()) {
                match preflight_agent_link_removal(&project, target, &journal) {
                    Ok(Some(candidate)) => link_removals.push(candidate),
                    Ok(None) => {}
                    Err(error) => issues.push(error),
                }
            }
            continue;
        }

        let destination = project.join(".agents").join(name);
        if let Err(detail) = reject_symlink_components(&project, Path::new(".agents")) {
            issues.push(issue(
                "install.agent-target.unsafe",
                detail,
                Some(&format!(".agents/{name}")),
                "install",
            ));
            continue;
        }
        match fs::symlink_metadata(&destination) {
            Ok(metadata) if metadata_is_link_or_reparse(&metadata) => {
                match fs::read_link(&destination) {
                    Ok(actual) if actual == expected_agent_link(name) => {
                        links.push(name.to_string())
                    }
                    Ok(actual) => issues.push(issue(
                        "install.agent-link.wrong-target",
                        format!(
                            "现有链接指向 {}，期望 {}",
                            actual.display(),
                            expected_agent_link(name).display()
                        ),
                        Some(&format!(".agents/{name}")),
                        "install",
                    )),
                    Err(error) => issues.push(issue(
                        "install.agent-link.read",
                        format!("无法读取现有链接：{error}"),
                        Some(&format!(".agents/{name}")),
                        "install",
                    )),
                }
            }
            Ok(metadata) if metadata.is_dir() => {
                let owned_prefix = format!(".agents/{name}/");
                let owns_destination = previous_assets
                    .keys()
                    .any(|path| path.starts_with(&owned_prefix))
                    || journal.entries.iter().any(|(path, entry)| {
                        path.starts_with(&owned_prefix)
                            && entry.operation == JournalOperation::WriteAgentCopy
                    });
                if !owns_destination {
                    preserved = true;
                    preserved_names.push(name.to_string());
                    continue;
                }
                for (relative, bytes) in source_files {
                    let display = format!(
                        ".agents/{name}/{}",
                        relative.to_string_lossy().replace('\\', "/")
                    );
                    let target = project.join(&display);
                    if let Err(detail) = validate_target_ancestors(&project, Path::new(&display)) {
                        issues.push(issue(
                            "install.agent-copy.unsafe",
                            detail,
                            Some(&display),
                            "install",
                        ));
                        continue;
                    }
                    let expected_sha256 = content_sha256(&bytes);
                    let baseline_sha256 = match fs::read(&target) {
                        Ok(current) => {
                            let current_hash = content_sha256(&current);
                            let journal_owns = journal.entries.get(&display).is_some_and(|entry| {
                                entry.operation == JournalOperation::WriteAgentCopy
                                    && entry.expected_sha256.as_deref()
                                        == Some(expected_sha256.as_str())
                                    && (entry.expected_sha256.as_deref()
                                        == Some(current_hash.as_str())
                                        || entry.baseline_sha256.as_deref()
                                            == Some(current_hash.as_str()))
                            });
                            if !journal_owns {
                                match previous_assets.get(display.as_str()) {
                                    Some(owned) if owned.sha256 == current_hash => {}
                                    Some(_) => {
                                        issues.push(issue(
                                            "install.agent-copy.modified",
                                            "同步副本已被修改，拒绝覆盖",
                                            Some(&display),
                                            "install",
                                        ));
                                        continue;
                                    }
                                    None => {
                                        issues.push(issue(
                                            "install.agent-copy.unowned",
                                            "同步目标文件不属于当前 manifest，拒绝覆盖",
                                            Some(&display),
                                            "install",
                                        ));
                                        continue;
                                    }
                                }
                            }
                            Some(current_hash)
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                        Err(error) => {
                            issues.push(issue(
                                "install.agent-copy.read",
                                format!("无法读取同步目标：{error}"),
                                Some(&display),
                                "install",
                            ));
                            continue;
                        }
                    };
                    copies.push(CopyCandidate {
                        baseline_sha256,
                        path: display,
                        sha256: expected_sha256,
                        bytes,
                    });
                }
            }
            Ok(_) => issues.push(issue(
                "install.agent-target.unsafe",
                "智能体共享目标不是目录或正确的相对链接",
                Some(&format!(".agents/{name}")),
                "install",
            )),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if prefer_relative_symlinks {
                    links.push(name.to_string());
                } else {
                    for (relative, bytes) in source_files {
                        let display = format!(
                            ".agents/{name}/{}",
                            relative.to_string_lossy().replace('\\', "/")
                        );
                        copies.push(CopyCandidate {
                            baseline_sha256: None,
                            path: display,
                            sha256: content_sha256(&bytes),
                            bytes,
                        });
                    }
                }
            }
            Err(error) => issues.push(issue(
                "install.agent-target.read",
                format!("无法读取智能体共享目标：{error}"),
                Some(&format!(".agents/{name}")),
                "install",
            )),
        }
    }

    for candidate in &copies {
        if let Some(entry) = journal.entries.get(&candidate.path) {
            if entry.operation != JournalOperation::WriteAgentCopy
                || entry.expected_sha256.as_deref() != Some(candidate.sha256.as_str())
            {
                issues.push(issue(
                    "install.agent-copy.journal-diverged",
                    "同步副本恢复日志与本轮期望内容不一致",
                    Some(&candidate.path),
                    "install",
                ));
            }
        }
    }
    for name in &links {
        let path = format!(".agents/{name}");
        let expected = expected_agent_link(name).to_string_lossy().into_owned();
        if let Some(entry) = journal.entries.get(&path) {
            if entry.operation != JournalOperation::CreateAgentLink
                || entry.link_target.as_deref() != Some(expected.as_str())
            {
                issues.push(issue(
                    "install.agent-link.journal-diverged",
                    "智能体链接恢复日志与本轮期望目标不一致",
                    Some(&path),
                    "install",
                ));
            }
        }
    }
    if !issues.is_empty() {
        return Err(issues);
    }

    for candidate in &copies {
        journal
            .entries
            .entry(candidate.path.clone())
            .or_insert_with(|| InstallJournalEntry {
                operation: JournalOperation::WriteAgentCopy,
                baseline_sha256: candidate.baseline_sha256.clone(),
                expected_sha256: Some(candidate.sha256.clone()),
                state: JournalEntryState::Pending,
                link_target: None,
            });
    }
    for candidate in &removals {
        journal.entries.insert(
            candidate.path.clone(),
            InstallJournalEntry {
                operation: candidate.operation,
                baseline_sha256: Some(candidate.baseline_sha256.clone()),
                expected_sha256: None,
                state: JournalEntryState::Pending,
                link_target: None,
            },
        );
    }
    for candidate in &link_removals {
        journal.entries.insert(
            candidate.path.clone(),
            InstallJournalEntry {
                operation: JournalOperation::RemoveAgentLink,
                baseline_sha256: Some(content_sha256(candidate.expected_target.as_bytes())),
                expected_sha256: None,
                state: JournalEntryState::Pending,
                link_target: Some(candidate.expected_target.clone()),
            },
        );
    }
    for name in &links {
        let path = format!(".agents/{name}");
        let expected = expected_agent_link(name).to_string_lossy().into_owned();
        let baseline_sha256 = fs::read_link(project.join(&path))
            .ok()
            .map(|actual| content_sha256(actual.to_string_lossy().as_bytes()));
        journal
            .entries
            .entry(path)
            .or_insert_with(|| InstallJournalEntry {
                operation: JournalOperation::CreateAgentLink,
                baseline_sha256,
                expected_sha256: Some(content_sha256(expected.as_bytes())),
                state: JournalEntryState::Pending,
                link_target: Some(expected),
            });
    }
    save_install_journal(&project, &journal).map_err(|error| vec![error])?;

    let mut linked_count = 0;
    let mut copied_count = 0;
    let mut installed_link_names = Vec::new();
    for name in links {
        let destination = project.join(".agents").join(&name);
        match fs::symlink_metadata(&destination) {
            Ok(metadata)
                if metadata_is_link_or_reparse(&metadata)
                    && fs::read_link(&destination).ok().as_deref()
                        == Some(expected_agent_link(&name).as_path()) =>
            {
                linked_count += 1;
                installed_link_names.push(name.clone());
                if let Some(entry) = journal.entries.get_mut(&format!(".agents/{name}")) {
                    entry.state = JournalEntryState::Applied;
                }
                save_install_journal(&project, &journal).map_err(|error| vec![error])?;
                continue;
            }
            Ok(_) => {
                return Err(vec![issue(
                    "install.agent-link.changed-during-install",
                    "智能体链接在预检后发生变化",
                    Some(&format!(".agents/{name}")),
                    "install",
                )]);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(vec![issue(
                    "install.agent-link.changed-during-install",
                    format!("创建链接前无法检查目标：{error}"),
                    Some(&format!(".agents/{name}")),
                    "install",
                )]);
            }
        }
        fs::create_dir_all(project.join(".agents")).map_err(|error| {
            vec![issue(
                "install.agent-root.create",
                format!("无法创建 .agents：{error}"),
                Some(".agents"),
                "install",
            )]
        })?;
        match create_directory_symlink(&expected_agent_link(&name), &destination) {
            Ok(()) => {
                linked_count += 1;
                installed_link_names.push(name.clone());
                if let Some(entry) = journal.entries.get_mut(&format!(".agents/{name}")) {
                    entry.state = JournalEntryState::Applied;
                }
                save_install_journal(&project, &journal).map_err(|error| vec![error])?;
            }
            Err(_) => {
                journal.entries.remove(&format!(".agents/{name}"));
                let source = project.join(".claude").join(&name);
                let mut source_files = Vec::new();
                collect_agent_source_files(&source, Path::new(""), &mut source_files).map_err(
                    |detail| {
                        vec![issue(
                            "install.agent-source.unsafe",
                            detail,
                            Some(&format!(".claude/{name}")),
                            "install",
                        )]
                    },
                )?;
                for (relative, bytes) in source_files {
                    copies.push(CopyCandidate {
                        baseline_sha256: None,
                        path: format!(
                            ".agents/{name}/{}",
                            relative.to_string_lossy().replace('\\', "/")
                        ),
                        sha256: content_sha256(&bytes),
                        bytes,
                    });
                }
            }
        }
    }

    for candidate in &copies {
        journal
            .entries
            .entry(candidate.path.clone())
            .or_insert_with(|| InstallJournalEntry {
                operation: JournalOperation::WriteAgentCopy,
                baseline_sha256: candidate.baseline_sha256.clone(),
                expected_sha256: Some(candidate.sha256.clone()),
                state: JournalEntryState::Pending,
                link_target: None,
            });
    }
    save_install_journal(&project, &journal).map_err(|error| vec![error])?;

    let mut installed_assets = Vec::new();
    for candidate in copies {
        let target = project.join(&candidate.path);
        validate_target_ancestors(&project, Path::new(&candidate.path)).map_err(|detail| {
            vec![issue(
                "install.agent-copy.changed-during-install",
                detail,
                Some(&candidate.path),
                "install",
            )]
        })?;
        let current_sha256 = match fs::read(&target) {
            Ok(bytes) => Some(content_sha256(&bytes)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => {
                return Err(vec![issue(
                    "install.agent-copy.changed-during-install",
                    format!("复制前无法重新读取目标：{error}"),
                    Some(&candidate.path),
                    "install",
                )]);
            }
        };
        let already_applied = current_sha256.as_deref() == Some(candidate.sha256.as_str());
        if !already_applied && current_sha256 != candidate.baseline_sha256 {
            return Err(vec![issue(
                "install.agent-copy.changed-during-install",
                "同步副本在预检后发生变化，已保留恢复日志",
                Some(&candidate.path),
                "install",
            )]);
        }
        if !already_applied {
            atomic_write(&target, &candidate.bytes).map_err(|error| {
                vec![issue(
                    "install.agent-copy.write",
                    error,
                    Some(&candidate.path),
                    "install",
                )]
            })?;
        }
        copied_count += 1;
        if let Some(entry) = journal.entries.get_mut(&candidate.path) {
            entry.state = JournalEntryState::Applied;
        }
        save_install_journal(&project, &journal).map_err(|error| vec![error])?;
        installed_assets.push(ManagedAgentAsset {
            path: candidate.path,
            sha256: candidate.sha256,
        });
    }
    for candidate in removals {
        let relative = normalized_relative_path(&candidate.path)
            .expect("preflight accepted managed-copy removal path");
        validate_target_ancestors(&project, &relative).map_err(|detail| {
            vec![issue(
                "install.agent-copy.changed-during-remove",
                detail,
                Some(&candidate.path),
                "install",
            )]
        })?;
        let target = project.join(&relative);
        match fs::read(&target) {
            Ok(bytes) if content_sha256(&bytes) == candidate.baseline_sha256 => {
                fs::remove_file(&target).map_err(|error| {
                    vec![issue(
                        "install.agent-copy.remove",
                        format!("无法删除旧同步副本：{error}"),
                        Some(&candidate.path),
                        "install",
                    )]
                })?;
                remove_empty_agent_copy_parents(&project, &target);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(vec![issue(
                    "install.agent-copy.changed-during-remove",
                    "同步副本在预检后发生变化，已停止删除",
                    Some(&candidate.path),
                    "install",
                )]);
            }
            Err(error) => {
                return Err(vec![issue(
                    "install.agent-copy.changed-during-remove",
                    format!("删除前无法重新读取同步副本：{error}"),
                    Some(&candidate.path),
                    "install",
                )]);
            }
        }
        if let Some(entry) = journal.entries.get_mut(&candidate.path) {
            entry.state = JournalEntryState::Applied;
        }
        save_install_journal(&project, &journal).map_err(|error| vec![error])?;
    }
    for candidate in link_removals {
        let destination = project.join(&candidate.path);
        match fs::symlink_metadata(&destination) {
            Ok(metadata) if metadata_is_link_or_reparse(&metadata) => {
                let actual = fs::read_link(&destination).map_err(|error| {
                    vec![issue(
                        "install.agent-link.changed-during-remove",
                        format!("删除前无法读取智能体链接：{error}"),
                        Some(&candidate.path),
                        "install",
                    )]
                })?;
                if actual != Path::new(&candidate.expected_target) {
                    return Err(vec![issue(
                        "install.agent-link.remove-mismatch",
                        "智能体链接在预检后改变，已停止删除",
                        Some(&candidate.path),
                        "install",
                    )]);
                }
                fs::remove_file(&destination).map_err(|error| {
                    vec![issue(
                        "install.agent-link.remove",
                        format!("无法删除旧智能体链接：{error}"),
                        Some(&candidate.path),
                        "install",
                    )]
                })?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Ok(_) => {
                return Err(vec![issue(
                    "install.agent-link.remove-mismatch",
                    "智能体链接在预检后被替换为非链接目标，已停止删除",
                    Some(&candidate.path),
                    "install",
                )]);
            }
            Err(error) => {
                return Err(vec![issue(
                    "install.agent-link.changed-during-remove",
                    format!("删除前无法检查智能体链接：{error}"),
                    Some(&candidate.path),
                    "install",
                )]);
            }
        }
        if let Some(entry) = journal.entries.get_mut(&candidate.path) {
            entry.state = JournalEntryState::Applied;
        }
        save_install_journal(&project, &journal).map_err(|error| vec![error])?;
    }
    installed_assets.sort_by(|left, right| left.path.cmp(&right.path));
    let copied_names: BTreeSet<String> = installed_assets
        .iter()
        .filter_map(|asset| asset.path.split('/').nth(1).map(str::to_string))
        .collect();
    let mut targets = Vec::new();
    for name in installed_link_names {
        targets.push(AgentAssetTarget {
            path: format!(".agents/{name}"),
            source_path: format!(".claude/{name}"),
            mode: AgentAssetMode::RelativeSymlink,
            link_target: Some(expected_agent_link(&name).to_string_lossy().into_owned()),
        });
    }
    for name in copied_names {
        targets.push(AgentAssetTarget {
            path: format!(".agents/{name}"),
            source_path: format!(".claude/{name}"),
            mode: AgentAssetMode::ManagedCopy,
            link_target: None,
        });
    }
    for name in preserved_names {
        targets.push(AgentAssetTarget {
            path: format!(".agents/{name}"),
            source_path: format!(".claude/{name}"),
            mode: AgentAssetMode::Preserved,
            link_target: None,
        });
    }
    targets.sort_by(|left, right| left.path.cmp(&right.path));
    manifest.agent_assets = installed_assets;
    manifest.agent_asset_targets = targets;
    let mode = match (linked_count > 0, copied_count > 0, preserved) {
        (true, false, false) => AgentAssetMode::RelativeSymlink,
        (false, true, false) => AgentAssetMode::ManagedCopy,
        (false, false, true) => AgentAssetMode::Preserved,
        (false, false, false) => AgentAssetMode::Preserved,
        _ => AgentAssetMode::Mixed,
    };
    manifest.agent_asset_mode = Some(mode);
    Ok(mode)
}

fn remove_empty_agent_copy_parents(project: &Path, target: &Path) {
    let stop = project.join(".agents");
    let mut current = target.parent();
    while let Some(directory) = current {
        if directory == stop || !directory.starts_with(&stop) {
            break;
        }
        let parent = directory.parent();
        let is_empty = fs::read_dir(directory)
            .ok()
            .is_some_and(|mut entries| entries.next().is_none());
        if !is_empty || fs::remove_dir(directory).is_err() {
            break;
        }
        current = parent;
    }
}

#[cfg(unix)]
fn create_directory_symlink(target: &Path, destination: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, destination)
}

#[cfg(windows)]
fn create_directory_symlink(target: &Path, destination: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, destination)
}

#[cfg(not(any(unix, windows)))]
fn create_directory_symlink(_target: &Path, _destination: &Path) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "platform does not support directory symlinks",
    ))
}

pub fn save_ownership_manifest(project: &Path, manifest: &OwnershipManifest) -> Result<(), String> {
    if manifest.schema_version != INITIALIZATION_STATE_SCHEMA_VERSION {
        return Err(format!(
            "拒绝写入 schemaVersion={} 的所有权 manifest",
            manifest.schema_version
        ));
    }
    if manifest.state != InitializationRunState::Completed {
        return Err("只有 completed 状态可以写入便携所有权 manifest".to_string());
    }
    let project = canonical_directory(project, "项目")?;
    if let Some(journal) = load_install_journal(&project).map_err(|issue| issue.detail)? {
        if journal.plan_sha256 != manifest.plan_sha256 {
            return Err("存在另一份计划的未完成安装恢复日志，拒绝完成当前 manifest".to_string());
        }
        if journal
            .entries
            .values()
            .any(|entry| entry.state != JournalEntryState::Applied)
        {
            return Err("安装恢复日志仍有 pending 目标，拒绝写入 completed manifest".to_string());
        }
    }
    let verification = verify_ownership_manifest(&project, manifest);
    if !verification.is_empty() {
        return Err(format!(
            "所有权 manifest 验证失败：{}",
            verification
                .iter()
                .map(|item| format!("{}: {}", item.code, item.detail))
                .collect::<Vec<_>>()
                .join("；")
        ));
    }
    validate_target_ancestors(&project, Path::new(OWNERSHIP_MANIFEST_PATH))?;
    let target = project.join(OWNERSHIP_MANIFEST_PATH);
    match fs::symlink_metadata(&target) {
        Ok(_) => {
            let current = fs::read(&target)
                .map_err(|error| format!("无法读取现有所有权 manifest：{error}"))?;
            let current: OwnershipManifest = serde_json::from_slice(&current)
                .map_err(|error| format!("现有所有权 manifest 不可解析，拒绝覆盖：{error}"))?;
            if current.schema_version != INITIALIZATION_STATE_SCHEMA_VERSION
                || current.state != InitializationRunState::Completed
            {
                return Err("现有文件不是受支持的 completed v4 manifest，拒绝覆盖".to_string());
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("无法检查现有所有权 manifest：{error}")),
    }
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("无法序列化所有权 manifest：{error}"))?;
    atomic_write(&target, &bytes)?;
    if let Some(journal) = load_install_journal(&project).map_err(|issue| issue.detail)? {
        if journal.plan_sha256 == manifest.plan_sha256 {
            let path = journal_path(&project)?;
            fs::remove_file(&path).map_err(|error| {
                format!("completed manifest 已写入，但无法清理恢复日志：{error}")
            })?;
        }
    }
    Ok(())
}

pub fn load_ownership_manifest(project: &Path) -> Result<Option<OwnershipManifest>, String> {
    let project = canonical_directory(project, "项目")?;
    let path = project.join(OWNERSHIP_MANIFEST_PATH);
    reject_symlink_components(&project, Path::new(OWNERSHIP_MANIFEST_PATH))?;
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("无法读取所有权 manifest：{error}")),
    };
    let manifest: OwnershipManifest = serde_json::from_slice(&bytes)
        .map_err(|error| format!("所有权 manifest JSON 无法解析：{error}"))?;
    if manifest.schema_version != INITIALIZATION_STATE_SCHEMA_VERSION {
        return Err(format!(
            "不支持的所有权 manifest schemaVersion={}",
            manifest.schema_version
        ));
    }
    if manifest.state != InitializationRunState::Completed {
        return Err("便携所有权 manifest 不是 completed 状态".to_string());
    }
    let structural_issues = validate_manifest_structure(&manifest);
    if !structural_issues.is_empty() {
        return Err(format!(
            "所有权 manifest 结构无效：{}",
            structural_issues
                .iter()
                .map(|item| format!("{}: {}", item.code, item.detail))
                .collect::<Vec<_>>()
                .join("；")
        ));
    }
    Ok(Some(manifest))
}

pub fn verify_ownership_manifest(
    project: &Path,
    manifest: &OwnershipManifest,
) -> Vec<ValidationIssue> {
    let project = match canonical_directory(project, "项目") {
        Ok(project) => project,
        Err(error) => {
            return vec![issue("manifest.project.invalid", error, None, "verify")];
        }
    };
    let mut issues = validate_manifest_structure(manifest);
    if manifest.schema_version != INITIALIZATION_STATE_SCHEMA_VERSION {
        issues.push(issue(
            "manifest.schema.unsupported",
            "所有权 manifest schemaVersion 不受支持",
            None,
            "verify",
        ));
    }
    if manifest.state != InitializationRunState::Completed {
        issues.push(issue(
            "manifest.state.incomplete",
            "所有权 manifest 尚未 completed",
            None,
            "verify",
        ));
    }
    if manifest.run_id.trim().is_empty() {
        issues.push(issue(
            "manifest.run-id.missing",
            "completed manifest 缺少 runId",
            None,
            "verify",
        ));
    }
    if manifest.inventory_sha256.trim().is_empty() {
        issues.push(issue(
            "manifest.inventory-hash.missing",
            "completed manifest 缺少 inventorySha256",
            None,
            "verify",
        ));
    }
    if manifest.inventory_summary.is_none() {
        issues.push(issue(
            "manifest.inventory-summary.missing",
            "completed manifest 缺少紧凑项目清单摘要",
            None,
            "verify",
        ));
    }
    if manifest.plan_sha256.trim().is_empty() {
        issues.push(issue(
            "manifest.plan-hash.missing",
            "completed manifest 缺少 planSha256",
            None,
            "verify",
        ));
    }
    if manifest.started_at_unix_ms == 0
        || manifest.installed_at_unix_ms == 0
        || manifest.completed_at_unix_ms < manifest.installed_at_unix_ms
        || manifest.installed_at_unix_ms < manifest.started_at_unix_ms
    {
        issues.push(issue(
            "manifest.timestamps.invalid",
            "completed manifest 的 started/installed/completed 时间无效或顺序错误",
            None,
            "verify",
        ));
    }
    if !manifest.checkpoints.iter().any(|checkpoint| {
        checkpoint.state == InitializationRunState::Completed
            && checkpoint.artifact_totals == manifest.artifact_totals
            && checkpoint.completed_at_unix_ms == manifest.completed_at_unix_ms
    }) {
        issues.push(issue(
            "manifest.checkpoints.incomplete",
            "completed manifest 缺少与最终产物计数一致的 completed checkpoint",
            None,
            "verify",
        ));
    }
    let mut paths = BTreeSet::new();
    let mut documents = 0;
    let mut rules = 0;
    let mut skills = 0;
    for artifact in &manifest.artifacts {
        if !paths.insert(artifact.path.as_str()) {
            issues.push(issue(
                "manifest.path.duplicate",
                "所有权 manifest 包含重复路径",
                Some(&artifact.path),
                "verify",
            ));
        }
        match artifact.kind {
            ArtifactKind::Document => documents += 1,
            ArtifactKind::Rule => rules += 1,
            ArtifactKind::Skill => skills += 1,
        }
        verify_owned_file(
            &project,
            &artifact.path,
            &artifact.sha256,
            "manifest.hash.mismatch",
            &mut issues,
        );
    }
    if manifest.artifact_totals.documents != documents
        || manifest.artifact_totals.rules != rules
        || manifest.artifact_totals.skills != skills
        || manifest.artifact_totals.total != manifest.artifacts.len()
    {
        issues.push(issue(
            "manifest.totals.mismatch",
            "所有权 manifest 的产物计数与实际条目不一致",
            None,
            "verify",
        ));
    }
    for asset in &manifest.agent_assets {
        verify_owned_file(
            &project,
            &asset.path,
            &asset.sha256,
            "manifest.agent-asset.hash-mismatch",
            &mut issues,
        );
    }
    let mut agent_target_paths = BTreeSet::new();
    for target in &manifest.agent_asset_targets {
        if !agent_target_paths.insert(target.path.as_str()) {
            issues.push(issue(
                "manifest.agent-target.duplicate",
                "智能体目标在 manifest 中重复",
                Some(&target.path),
                "verify",
            ));
        }
        verify_agent_asset_target(&project, target, &manifest.agent_assets, &mut issues);
    }
    verify_agent_target_coverage_and_mode(&project, manifest, &mut issues);
    for asset in &manifest.agent_assets {
        let covered = manifest.agent_asset_targets.iter().any(|target| {
            target.mode == AgentAssetMode::ManagedCopy
                && asset
                    .path
                    .starts_with(&format!("{}/", target.path.trim_end_matches('/')))
        });
        if !covered {
            issues.push(issue(
                "manifest.agent-asset.target-missing",
                "managed-copy 文件没有对应的目标状态记录",
                Some(&asset.path),
                "verify",
            ));
        }
    }
    for entry in &manifest.managed_entries {
        let path = project.join(&entry.path);
        match fs::read_to_string(&path) {
            Ok(content) => match managed_block_range(&content) {
                Ok(Some((start, end)))
                    if content_sha256(&content.as_bytes()[start..end]) == entry.block_sha256 => {}
                _ => issues.push(issue(
                    "manifest.entry.hash-mismatch",
                    "入口文件托管块缺失或哈希不一致",
                    Some(&entry.path),
                    "verify",
                )),
            },
            Err(error) => issues.push(issue(
                "manifest.entry.missing",
                format!("无法读取入口文件：{error}"),
                Some(&entry.path),
                "verify",
            )),
        }
    }
    for required in ["CLAUDE.md", "AGENTS.md"] {
        let count = manifest
            .managed_entries
            .iter()
            .filter(|entry| entry.path == required)
            .count();
        if count == 0 {
            issues.push(issue(
                "manifest.entries.incomplete",
                "completed manifest 必须同时且仅记录一份 CLAUDE.md 与 AGENTS.md 托管块",
                Some(required),
                "verify",
            ));
        } else if count > 1 {
            issues.push(issue(
                "manifest.entry.duplicate",
                "completed manifest 的入口托管块记录重复",
                Some(required),
                "verify",
            ));
        }
    }
    issues
}

fn verify_agent_target_coverage_and_mode(
    project: &Path,
    manifest: &OwnershipManifest,
    issues: &mut Vec<ValidationIssue>,
) {
    let mut expected_target_count = 0usize;
    for name in AGENT_ASSET_NAMES {
        let source_relative = format!(".claude/{name}");
        let source = project.join(&source_relative);
        let source_metadata = match fs::symlink_metadata(&source) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                issues.push(issue(
                    "manifest.agent-source.invalid",
                    format!("无法读取智能体资源根：{error}"),
                    Some(&source_relative),
                    "verify",
                ));
                continue;
            }
        };
        if metadata_is_link_or_reparse(&source_metadata) || !source_metadata.is_dir() {
            issues.push(issue(
                "manifest.agent-source.invalid",
                "智能体资源根必须是普通目录",
                Some(&source_relative),
                "verify",
            ));
            continue;
        }
        let mut source_files = Vec::new();
        if let Err(detail) = collect_agent_source_files(&source, Path::new(""), &mut source_files) {
            issues.push(issue(
                "manifest.agent-source.invalid",
                detail,
                Some(&source_relative),
                "verify",
            ));
            continue;
        }
        if source_files.is_empty() {
            continue;
        }
        expected_target_count += 1;
        let target_path = format!(".agents/{name}");
        let count = manifest
            .agent_asset_targets
            .iter()
            .filter(|target| target.path == target_path && target.source_path == source_relative)
            .count();
        if count != 1 {
            issues.push(issue(
                "manifest.agent-target.missing",
                "每个非空 .claude 智能体资源根必须恰好对应一个 .agents 目标",
                Some(&target_path),
                "verify",
            ));
        }
    }

    if manifest.agent_asset_targets.len() != expected_target_count {
        issues.push(issue(
            "manifest.agent-target.coverage-mismatch",
            "智能体目标数量与非空 .claude 资源根数量不一致",
            None,
            "verify",
        ));
    }

    let first_mode = manifest
        .agent_asset_targets
        .first()
        .map(|target| target.mode);
    let expected_mode = first_mode.map(|first| {
        if manifest
            .agent_asset_targets
            .iter()
            .all(|target| target.mode == first)
        {
            first
        } else {
            AgentAssetMode::Mixed
        }
    });
    let aggregate_matches = match expected_mode {
        Some(expected) => manifest.agent_asset_mode == Some(expected),
        None => matches!(
            manifest.agent_asset_mode,
            None | Some(AgentAssetMode::Preserved)
        ),
    };
    if !aggregate_matches {
        issues.push(issue(
            "manifest.agent-mode.mismatch",
            "agentAssetMode 与逐目标共享模式汇总不一致",
            None,
            "verify",
        ));
    }
}

fn verify_agent_asset_target(
    project: &Path,
    target: &AgentAssetTarget,
    managed_assets: &[ManagedAgentAsset],
    issues: &mut Vec<ValidationIssue>,
) {
    let relative = match normalized_relative_path(&target.path) {
        Ok(relative) => relative,
        Err(detail) => {
            issues.push(issue(
                "manifest.agent-target.invalid",
                detail,
                Some(&target.path),
                "verify",
            ));
            return;
        }
    };
    let path = project.join(relative);
    match target.mode {
        AgentAssetMode::RelativeSymlink => {
            let expected = target.link_target.as_deref().unwrap_or_default();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(metadata) => metadata,
                Err(error) => {
                    issues.push(issue(
                        "manifest.agent-target.link-mismatch",
                        format!("无法读取相对链接：{error}"),
                        Some(&target.path),
                        "verify",
                    ));
                    return;
                }
            };
            if !metadata_is_link_or_reparse(&metadata)
                || fs::read_link(&path).ok().as_deref() != Some(Path::new(expected))
            {
                issues.push(issue(
                    "manifest.agent-target.link-mismatch",
                    "相对链接缺失或目标与 manifest 不一致",
                    Some(&target.path),
                    "verify",
                ));
            }
        }
        AgentAssetMode::ManagedCopy => {
            let metadata = fs::symlink_metadata(&path);
            if !matches!(metadata, Ok(ref metadata) if metadata.is_dir() && !metadata_is_link_or_reparse(metadata))
            {
                issues.push(issue(
                    "manifest.agent-target.copy-missing",
                    "managed-copy 目标目录缺失或不是普通目录",
                    Some(&target.path),
                    "verify",
                ));
            }
            let prefix = format!("{}/", target.path.trim_end_matches('/'));
            if !managed_assets
                .iter()
                .any(|asset| asset.path.starts_with(&prefix))
            {
                issues.push(issue(
                    "manifest.agent-target.copy-empty",
                    "managed-copy 目标没有任何受所有权哈希保护的文件",
                    Some(&target.path),
                    "verify",
                ));
            }
        }
        AgentAssetMode::Preserved => match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.is_dir() && !metadata_is_link_or_reparse(&metadata) => {}
            Ok(_) => issues.push(issue(
                "manifest.agent-target.preserved-mismatch",
                "preserved 目标不再是原有普通目录",
                Some(&target.path),
                "verify",
            )),
            Err(error) => issues.push(issue(
                "manifest.agent-target.preserved-mismatch",
                format!("preserved 目标不可访问：{error}"),
                Some(&target.path),
                "verify",
            )),
        },
        AgentAssetMode::Mixed => issues.push(issue(
            "manifest.agent-target.mode-invalid",
            "单个智能体目标不能使用 mixed 模式",
            Some(&target.path),
            "verify",
        )),
    }
    match normalized_relative_path(&target.source_path) {
        Ok(source) => {
            let source_path = project.join(&source);
            let source_metadata = fs::symlink_metadata(&source_path);
            if reject_symlink_components(project, &source).is_err()
                || !matches!(source_metadata, Ok(ref metadata) if metadata.is_dir() && !metadata_is_link_or_reparse(metadata))
            {
                issues.push(issue(
                    "manifest.agent-source.invalid",
                    "智能体目标的 sourcePath 缺失、不是普通目录或包含链接",
                    Some(&target.path),
                    "verify",
                ));
            }
        }
        Err(_) => issues.push(issue(
            "manifest.agent-source.invalid",
            "智能体目标记录了无效 sourcePath",
            Some(&target.path),
            "verify",
        )),
    }
}

fn verify_owned_file(
    project: &Path,
    display_path: &str,
    expected_sha256: &str,
    code: &str,
    issues: &mut Vec<ValidationIssue>,
) {
    let relative = match normalized_relative_path(display_path) {
        Ok(relative) => relative,
        Err(detail) => {
            issues.push(issue(code, detail, Some(display_path), "verify"));
            return;
        }
    };
    if let Err(detail) = reject_symlink_components(project, &relative) {
        issues.push(issue(code, detail, Some(display_path), "verify"));
        return;
    }
    match fs::read(project.join(relative)) {
        Ok(bytes) if content_sha256(&bytes) == expected_sha256 => {}
        Ok(_) => issues.push(issue(
            code,
            "文件内容哈希与 completed manifest 不一致",
            Some(display_path),
            "verify",
        )),
        Err(error) => issues.push(issue(
            code,
            format!("无法读取 manifest 文件：{error}"),
            Some(display_path),
            "verify",
        )),
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("目标路径缺少父目录：{}", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("无法创建目录 {}：{error}", parent.display()))?;
    let permissions = fs::metadata(path)
        .ok()
        .map(|metadata| metadata.permissions());
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("目标文件名不是有效 UTF-8：{}", path.display()))?;
    let (temporary, mut file) = loop {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = parent.join(format!(
            ".{file_name}.{}.{}.tmp",
            std::process::id(),
            sequence
        ));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => break (temporary, file),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(format!("无法创建临时文件 {}：{error}", temporary.display()));
            }
        }
    };
    let result = (|| {
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)
                .map_err(|error| format!("无法保留目标文件权限：{error}"))?;
        }
        file.write_all(bytes)
            .map_err(|error| format!("无法写入临时文件：{error}"))?;
        file.sync_all()
            .map_err(|error| format!("无法同步临时文件：{error}"))?;
        drop(file);
        replace_file(&temporary, path)
            .map_err(|error| format!("无法原子替换 {}：{error}", path.display()))?;
        sync_parent_directory(parent)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

#[cfg(not(windows))]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    fs::rename(source, target)
}

#[cfg(windows)]
fn replace_file(source: &Path, target: &Path) -> std::io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    #[link(name = "Kernel32")]
    extern "system" {
        fn MoveFileExW(existing: *const u16, replacement: *const u16, flags: u32) -> i32;
    }

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    let source: Vec<u16> = source.as_os_str().encode_wide().chain(Some(0)).collect();
    let target: Vec<u16> = target.as_os_str().encode_wide().chain(Some(0)).collect();
    // SAFETY: Both paths are NUL-terminated UTF-16 buffers that stay alive for the call.
    let result = unsafe {
        MoveFileExW(
            source.as_ptr(),
            target.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn sync_parent_directory(parent: &Path) -> Result<(), String> {
    fs::File::open(parent)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| format!("无法同步目录 {}：{error}", parent.display()))
}

#[cfg(not(unix))]
fn sync_parent_directory(_parent: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::*;
    use crate::project_factory::types::{
        ArtifactKind, ArtifactPlan, ArtifactPlanItem, ArtifactTotals, EvidenceReference,
        InitializationCheckpoint, InitializationRunState, InitializationState, InventorySummary,
    };

    struct TestDir(PathBuf);

    impl TestDir {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "vibe-task3-{label}-{}-{}",
                std::process::id(),
                uuid::Uuid::new_v4()
            ));
            fs::create_dir_all(&path).expect("create fixture");
            Self(path)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn write(path: impl AsRef<Path>, content: &str) {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, content).expect("write fixture");
    }

    fn read(path: impl AsRef<Path>) -> String {
        fs::read_to_string(path).expect("read fixture")
    }

    fn item(id: &str, kind: ArtifactKind, target_path: &str) -> ArtifactPlanItem {
        ArtifactPlanItem {
            id: id.to_string(),
            kind,
            layer: "common".to_string(),
            topic: id.to_string(),
            target_path: target_path.to_string(),
            rationale: "来自项目真实代码证据".to_string(),
            evidence: vec![EvidenceReference {
                path: "src/main.rs".to_string(),
                symbol: Some("main".to_string()),
            }],
            covers: vec!["src".to_string()],
            required_sections: vec!["项目证据".to_string()],
        }
    }

    fn plan(items: Vec<ArtifactPlanItem>) -> ArtifactPlan {
        ArtifactPlan {
            schema_version: 1,
            project_name: "fixture".to_string(),
            artifacts: items,
            exclusions: Vec::new(),
        }
    }

    fn state(run_id: &str) -> InitializationState {
        InitializationState {
            schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
            run_id: run_id.to_string(),
            state: InitializationRunState::PlanReady,
            workspace_path: "/tmp/workspace".to_string(),
            attempt: 2,
            ..InitializationState::default()
        }
    }

    fn mark_completed(manifest: &mut OwnershipManifest) {
        manifest.state = InitializationRunState::Completed;
        manifest.run_id = "run-completed".to_string();
        manifest.inventory_sha256 = "inventory-sha256".to_string();
        manifest.inventory_summary = Some(InventorySummary {
            modules: 2,
            source_roots: 3,
            files: 10,
            frontend: false,
            backend: true,
        });
        if manifest.plan_sha256.is_empty() {
            manifest.plan_sha256 = "plan-sha256".to_string();
        }
        if manifest.installed_at_unix_ms == 0 {
            manifest.installed_at_unix_ms = 20;
        }
        manifest.started_at_unix_ms = manifest.installed_at_unix_ms.saturating_sub(10).max(1);
        manifest.completed_at_unix_ms = manifest.installed_at_unix_ms + 10;
        manifest.checkpoints = vec![InitializationCheckpoint {
            state: InitializationRunState::Completed,
            artifact_totals: manifest.artifact_totals,
            completed_at_unix_ms: manifest.completed_at_unix_ms,
        }];
    }

    fn empty_manifest(plan_sha256: &str) -> OwnershipManifest {
        OwnershipManifest {
            plan_sha256: plan_sha256.to_string(),
            ..OwnershipManifest::default()
        }
    }

    fn save_completed_manifest(project: &Path, manifest: &mut OwnershipManifest) {
        install_managed_entries(project, manifest).expect("install managed entries");
        mark_completed(manifest);
        save_ownership_manifest(project, manifest).expect("save completed manifest");
    }

    #[test]
    fn state_round_trips_atomically_and_rejects_corrupt_or_future_schema() {
        let project = TestDir::new("state");
        let expected = state("run-state");

        save_initialization_state(project.path(), &expected).expect("save state");
        assert_eq!(
            load_initialization_state(project.path()).expect("load state"),
            Some(expected)
        );

        let directory = state_directory(project.path()).expect("state directory");
        assert!(fs::read_dir(&directory)
            .expect("read state directory")
            .all(|entry| !entry
                .expect("state entry")
                .file_name()
                .to_string_lossy()
                .contains(".tmp")));

        write(directory.join("state.json"), "{not-json");
        assert!(load_initialization_state(project.path())
            .expect_err("corrupt state must fail")
            .contains("JSON"));

        let mut future = state("future");
        future.schema_version += 1;
        write(
            directory.join("state.json"),
            &serde_json::to_string(&future).expect("serialize future state"),
        );
        assert!(load_initialization_state(project.path())
            .expect_err("future state must fail")
            .contains("schema"));

        let _ = fs::remove_dir_all(directory);
    }

    #[cfg(unix)]
    #[test]
    fn state_directory_is_stable_for_canonical_project_aliases() {
        use std::os::unix::fs::symlink;

        let fixture = TestDir::new("state-alias");
        let project = fixture.path().join("project");
        let alias = fixture.path().join("alias");
        fs::create_dir(&project).expect("create project");
        symlink(&project, &alias).expect("create alias");

        assert_eq!(
            state_directory(&project).expect("real state directory"),
            state_directory(&alias).expect("alias state directory")
        );
    }

    #[test]
    fn installer_writes_only_planned_files_and_is_idempotent() {
        let project = TestDir::new("planned-project");
        let workspace = TestDir::new("planned-workspace");
        let artifact_plan = plan(vec![
            item("map", ArtifactKind::Document, "docs/ai/project-map.md"),
            item(
                "router",
                ArtifactKind::Rule,
                ".claude/rules/project/README.md",
            ),
        ]);
        write(
            workspace.path().join("docs/ai/project-map.md"),
            "# 项目地图\n\n真实项目证据。",
        );
        write(
            workspace.path().join(".claude/rules/project/README.md"),
            "# 规则路由\n\n按任务读取规则。",
        );
        write(
            workspace.path().join("docs/ai/not-planned.md"),
            "# 不应安装",
        );

        let mut first =
            install_planned_artifacts(project.path(), workspace.path(), &artifact_plan, None)
                .expect("first install");
        mark_completed(&mut first);
        assert!(project.path().join("docs/ai/project-map.md").is_file());
        assert!(project
            .path()
            .join(".claude/rules/project/README.md")
            .is_file());
        assert!(!project.path().join("docs/ai/not-planned.md").exists());

        let second = install_planned_artifacts(
            project.path(),
            workspace.path(),
            &artifact_plan,
            Some(&first),
        )
        .expect("idempotent reinstall");
        assert_eq!(first.artifacts, second.artifacts);
    }

    #[test]
    fn installer_preflights_all_targets_before_writing_any_file() {
        let project = TestDir::new("conflict-project");
        let workspace = TestDir::new("conflict-workspace");
        let artifact_plan = plan(vec![
            item("owned", ArtifactKind::Document, "docs/ai/owned.md"),
            item("blocked", ArtifactKind::Document, "docs/ai/blocked.md"),
        ]);
        write(workspace.path().join("docs/ai/owned.md"), "# 待安装");
        write(workspace.path().join("docs/ai/blocked.md"), "# 新内容");
        write(project.path().join("docs/ai/blocked.md"), "# 用户内容");

        let issues =
            install_planned_artifacts(project.path(), workspace.path(), &artifact_plan, None)
                .expect_err("unowned target must conflict");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "install.target.unowned"));
        assert!(!project.path().join("docs/ai/owned.md").exists());
        assert_eq!(
            read(project.path().join("docs/ai/blocked.md")),
            "# 用户内容"
        );
    }

    #[test]
    fn installer_rejects_plan_entries_outside_the_generated_roots() {
        let project = TestDir::new("allowlist-project");
        let workspace = TestDir::new("allowlist-workspace");
        let artifact_plan = plan(vec![item("unsafe", ArtifactKind::Document, "README.md")]);
        write(workspace.path().join("README.md"), "# 不应覆盖项目入口");

        let issues =
            install_planned_artifacts(project.path(), workspace.path(), &artifact_plan, None)
                .expect_err("outside allowlist must fail");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "install.target.outside-allowlist"));
        assert!(!project.path().join("README.md").exists());
    }

    #[test]
    fn installer_refuses_to_overwrite_a_modified_owned_target() {
        let project = TestDir::new("modified-project");
        let workspace = TestDir::new("modified-workspace");
        let artifact_plan = plan(vec![item(
            "map",
            ArtifactKind::Document,
            "docs/ai/project-map.md",
        )]);
        write(
            workspace.path().join("docs/ai/project-map.md"),
            "# 原始生成",
        );
        let mut previous =
            install_planned_artifacts(project.path(), workspace.path(), &artifact_plan, None)
                .expect("initial install");
        mark_completed(&mut previous);
        write(
            project.path().join("docs/ai/project-map.md"),
            "# 用户已修改",
        );

        let issues = install_planned_artifacts(
            project.path(),
            workspace.path(),
            &artifact_plan,
            Some(&previous),
        )
        .expect_err("modified owned target must conflict");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "install.target.modified"));
        assert_eq!(
            read(project.path().join("docs/ai/project-map.md")),
            "# 用户已修改"
        );
    }

    #[test]
    fn installer_resumes_only_files_marked_started_by_the_matching_journal() {
        let project = TestDir::new("journal-project");
        let workspace = TestDir::new("journal-workspace");
        let artifact_plan = plan(vec![item(
            "map",
            ArtifactKind::Document,
            "docs/ai/project-map.md",
        )]);
        let content = "# 已完成原子替换但尚未完成日志收尾";
        write(workspace.path().join("docs/ai/project-map.md"), content);
        write(project.path().join("docs/ai/project-map.md"), content);
        let hash = content_sha256(content.as_bytes());
        let mut entries = BTreeMap::new();
        entries.insert(
            "docs/ai/project-map.md".to_string(),
            InstallJournalEntry {
                operation: JournalOperation::WriteArtifact,
                baseline_sha256: None,
                expected_sha256: Some(hash),
                state: JournalEntryState::Applied,
                link_target: None,
            },
        );
        let journal = InstallJournal {
            schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
            plan_sha256: plan_sha256(&artifact_plan).expect("plan hash"),
            entries,
        };
        save_install_journal(project.path(), &journal).expect("save interrupted journal");

        let manifest =
            install_planned_artifacts(project.path(), workspace.path(), &artifact_plan, None)
                .expect("resume journal-owned target");
        assert_eq!(manifest.artifacts.len(), 1);
        assert_eq!(read(project.path().join("docs/ai/project-map.md")), content);
        assert!(journal_path(project.path()).expect("journal path").exists());
    }

    #[test]
    fn matching_journal_takes_precedence_over_the_previous_completed_manifest() {
        let project = TestDir::new("journal-priority-project");
        let workspace = TestDir::new("journal-priority-workspace");
        let artifact_plan = plan(vec![item(
            "map",
            ArtifactKind::Document,
            "docs/ai/project-map.md",
        )]);
        write(workspace.path().join("docs/ai/project-map.md"), "# 旧内容");
        let mut previous =
            install_planned_artifacts(project.path(), workspace.path(), &artifact_plan, None)
                .expect("initial install");
        save_completed_manifest(project.path(), &mut previous);

        let new_content = "# 新一轮已原子写入的内容";
        write(workspace.path().join("docs/ai/project-map.md"), new_content);
        write(project.path().join("docs/ai/project-map.md"), new_content);
        let path = "docs/ai/project-map.md".to_string();
        let mut entries = BTreeMap::new();
        entries.insert(
            path.clone(),
            InstallJournalEntry {
                operation: JournalOperation::WriteArtifact,
                baseline_sha256: previous
                    .artifacts
                    .iter()
                    .find(|artifact| artifact.path == path)
                    .map(|artifact| artifact.sha256.clone()),
                expected_sha256: Some(content_sha256(new_content.as_bytes())),
                state: JournalEntryState::Applied,
                link_target: None,
            },
        );
        save_install_journal(
            project.path(),
            &InstallJournal {
                schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
                plan_sha256: plan_sha256(&artifact_plan).expect("plan hash"),
                entries,
            },
        )
        .expect("save matching journal");

        install_planned_artifacts(
            project.path(),
            workspace.path(),
            &artifact_plan,
            Some(&previous),
        )
        .expect("matching journal must own its applied write before old manifest");
    }

    #[test]
    fn a_new_plan_deletes_unchanged_old_owned_artifacts() {
        let project = TestDir::new("remove-old-project");
        let workspace = TestDir::new("remove-old-workspace");
        let old_plan = plan(vec![item("old", ArtifactKind::Document, "docs/ai/old.md")]);
        write(workspace.path().join("docs/ai/old.md"), "# 旧产物");
        let mut previous =
            install_planned_artifacts(project.path(), workspace.path(), &old_plan, None)
                .expect("install old plan");
        save_completed_manifest(project.path(), &mut previous);

        let new_plan = plan(vec![item("new", ArtifactKind::Document, "docs/ai/new.md")]);
        write(workspace.path().join("docs/ai/new.md"), "# 新产物");
        let next =
            install_planned_artifacts(project.path(), workspace.path(), &new_plan, Some(&previous))
                .expect("replace plan");

        assert!(!project.path().join("docs/ai/old.md").exists());
        assert!(project.path().join("docs/ai/new.md").is_file());
        assert!(next
            .artifacts
            .iter()
            .all(|artifact| artifact.path != "docs/ai/old.md"));
    }

    #[test]
    fn a_new_plan_conflicts_on_modified_old_artifacts_and_preserves_prior_ownership() {
        let project = TestDir::new("keep-modified-old-project");
        let workspace = TestDir::new("keep-modified-old-workspace");
        let old_plan = plan(vec![item("old", ArtifactKind::Document, "docs/ai/old.md")]);
        write(workspace.path().join("docs/ai/old.md"), "# 旧产物");
        let mut previous =
            install_planned_artifacts(project.path(), workspace.path(), &old_plan, None)
                .expect("install old plan");
        save_completed_manifest(project.path(), &mut previous);
        write(project.path().join("docs/ai/old.md"), "# 用户已修改旧产物");

        let new_plan = plan(vec![item("new", ArtifactKind::Document, "docs/ai/new.md")]);
        write(workspace.path().join("docs/ai/new.md"), "# 新产物");
        let issues =
            install_planned_artifacts(project.path(), workspace.path(), &new_plan, Some(&previous))
                .expect_err("modified retired artifact must conflict");

        assert!(issues
            .iter()
            .any(|issue| issue.code == "install.removed-owned.modified"));
        assert_eq!(
            read(project.path().join("docs/ai/old.md")),
            "# 用户已修改旧产物"
        );
        assert!(!project.path().join("docs/ai/new.md").exists());
        let still_owned = load_ownership_manifest(project.path())
            .expect("read prior manifest")
            .expect("prior manifest remains");
        assert!(still_owned
            .artifacts
            .iter()
            .any(|artifact| artifact.path == "docs/ai/old.md"));
    }

    #[test]
    fn missing_target_with_pending_remove_is_marked_applied_on_resume() {
        let project = TestDir::new("remove-crash-project");
        let workspace = TestDir::new("remove-crash-workspace");
        let old_plan = plan(vec![item("old", ArtifactKind::Document, "docs/ai/old.md")]);
        write(workspace.path().join("docs/ai/old.md"), "# 旧产物");
        let mut previous =
            install_planned_artifacts(project.path(), workspace.path(), &old_plan, None)
                .expect("install old plan");
        save_completed_manifest(project.path(), &mut previous);

        let new_plan = plan(vec![item("new", ArtifactKind::Document, "docs/ai/new.md")]);
        write(workspace.path().join("docs/ai/new.md"), "# 新产物");
        let mut entries = BTreeMap::new();
        entries.insert(
            "docs/ai/old.md".to_string(),
            InstallJournalEntry {
                operation: JournalOperation::RemoveOwnedArtifact,
                baseline_sha256: previous.artifacts.first().map(|item| item.sha256.clone()),
                expected_sha256: None,
                state: JournalEntryState::Pending,
                link_target: None,
            },
        );
        save_install_journal(
            project.path(),
            &InstallJournal {
                schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
                plan_sha256: plan_sha256(&new_plan).expect("new plan hash"),
                entries,
            },
        )
        .expect("save pending removal");
        fs::remove_file(project.path().join("docs/ai/old.md"))
            .expect("simulate crash after delete");

        install_planned_artifacts(project.path(), workspace.path(), &new_plan, Some(&previous))
            .expect("resume removal");
        let journal = load_install_journal(project.path())
            .expect("load journal")
            .expect("journal remains");
        assert_eq!(
            journal
                .entries
                .get("docs/ai/old.md")
                .expect("removal entry")
                .state,
            JournalEntryState::Applied
        );
    }

    #[test]
    fn tampered_manifest_paths_never_delete_project_source() {
        let project = TestDir::new("tampered-manifest-project");
        let workspace = TestDir::new("tampered-manifest-workspace");
        let source_path = "src/main.rs";
        let source_content = "fn main() {}";
        write(project.path().join(source_path), source_content);
        let mut previous = empty_manifest("old-plan");
        mark_completed(&mut previous);
        previous.artifact_totals = ArtifactTotals {
            documents: 1,
            rules: 0,
            skills: 0,
            total: 1,
        };
        previous.artifacts = vec![OwnedArtifact {
            path: source_path.to_string(),
            sha256: content_sha256(source_content.as_bytes()),
            kind: ArtifactKind::Document,
        }];
        let new_plan = plan(vec![item(
            "map",
            ArtifactKind::Document,
            "docs/ai/project-map.md",
        )]);
        write(
            workspace.path().join("docs/ai/project-map.md"),
            "# 项目地图",
        );

        let issues =
            install_planned_artifacts(project.path(), workspace.path(), &new_plan, Some(&previous))
                .expect_err("forged ownership path must fail");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "manifest.artifact.path-invalid"));
        assert_eq!(read(project.path().join(source_path)), source_content);

        write(
            project.path().join("docs/ai/.initialization-manifest.json"),
            &serde_json::to_string_pretty(&previous).expect("serialize forged manifest"),
        );
        assert!(load_ownership_manifest(project.path())
            .expect_err("loader must reject forged paths")
            .contains("manifest.artifact.path-invalid"));
    }

    #[test]
    fn tampered_agent_asset_paths_are_rejected_without_touching_source() {
        let project = TestDir::new("tampered-agent-asset");
        let source_path = "src/secret.md";
        let source_content = "project-owned source";
        write(project.path().join(source_path), source_content);
        let mut manifest = empty_manifest("tampered-agent-plan");
        manifest.agent_assets = vec![ManagedAgentAsset {
            path: source_path.to_string(),
            sha256: content_sha256(source_content.as_bytes()),
        }];
        mark_completed(&mut manifest);

        assert!(verify_ownership_manifest(project.path(), &manifest)
            .iter()
            .any(|issue| issue.code == "manifest.agent-asset.path-invalid"));
        let issues = share_agent_assets_for_test(project.path(), &mut manifest, false)
            .expect_err("forged agent asset path must block sharing");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "manifest.agent-asset.path-invalid"));
        assert_eq!(read(project.path().join(source_path)), source_content);

        write(
            project.path().join("docs/ai/.initialization-manifest.json"),
            &serde_json::to_string_pretty(&manifest).expect("serialize forged manifest"),
        );
        assert!(load_ownership_manifest(project.path())
            .expect_err("loader must reject forged agent asset paths")
            .contains("manifest.agent-asset.path-invalid"));
    }

    #[test]
    fn completed_manifest_requires_exact_entry_set_and_inventory_summary() {
        let project = TestDir::new("manifest-entry-set");
        let mut manifest = empty_manifest("entry-set-plan");
        install_managed_entries(project.path(), &mut manifest).expect("install entries");
        mark_completed(&mut manifest);
        assert!(verify_ownership_manifest(project.path(), &manifest).is_empty());

        let mut missing = manifest.clone();
        missing
            .managed_entries
            .retain(|entry| entry.path != "AGENTS.md");
        assert!(verify_ownership_manifest(project.path(), &missing)
            .iter()
            .any(|issue| issue.code == "manifest.entries.incomplete"));

        let mut duplicate = manifest.clone();
        duplicate
            .managed_entries
            .push(duplicate.managed_entries[0].clone());
        assert!(verify_ownership_manifest(project.path(), &duplicate)
            .iter()
            .any(|issue| issue.code == "manifest.entry.duplicate"));

        let mut no_summary = manifest;
        no_summary.inventory_summary = None;
        assert!(verify_ownership_manifest(project.path(), &no_summary)
            .iter()
            .any(|issue| issue.code == "manifest.inventory-summary.missing"));
    }

    #[test]
    fn completed_manifest_requires_agent_target_coverage_and_consistent_mode() {
        let project = TestDir::new("manifest-agent-coverage");
        write(
            project.path().join(".claude/rules/project/README.md"),
            "# Rules",
        );
        write(
            project.path().join(".claude/skills/debug/SKILL.md"),
            "# Debug",
        );
        let mut manifest = empty_manifest("agent-coverage-plan");
        share_agent_assets_for_test(project.path(), &mut manifest, false)
            .expect("install managed copies");
        install_managed_entries(project.path(), &mut manifest).expect("install entries");
        mark_completed(&mut manifest);
        assert!(verify_ownership_manifest(project.path(), &manifest).is_empty());

        let mut missing = manifest.clone();
        missing
            .agent_asset_targets
            .retain(|target| target.path != ".agents/skills");
        assert!(verify_ownership_manifest(project.path(), &missing)
            .iter()
            .any(|issue| issue.code == "manifest.agent-target.missing"));

        let mut wrong_mode = manifest;
        wrong_mode.agent_asset_mode = Some(AgentAssetMode::RelativeSymlink);
        assert!(verify_ownership_manifest(project.path(), &wrong_mode)
            .iter()
            .any(|issue| issue.code == "manifest.agent-mode.mismatch"));
    }

    #[cfg(unix)]
    #[test]
    fn installer_rejects_a_staged_source_symlink_escape() {
        use std::os::unix::fs::symlink;

        let project = TestDir::new("source-link-project");
        let workspace = TestDir::new("source-link-workspace");
        let outside = TestDir::new("source-link-outside");
        let artifact_plan = plan(vec![item(
            "map",
            ArtifactKind::Document,
            "docs/ai/project-map.md",
        )]);
        write(outside.path().join("secret.md"), "secret");
        fs::create_dir_all(workspace.path().join("docs/ai")).expect("source parent");
        symlink(
            outside.path().join("secret.md"),
            workspace.path().join("docs/ai/project-map.md"),
        )
        .expect("source symlink");

        let issues =
            install_planned_artifacts(project.path(), workspace.path(), &artifact_plan, None)
                .expect_err("source symlink must fail");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "install.source.symlink"));
        assert!(!project.path().join("docs/ai/project-map.md").exists());
    }

    #[test]
    fn managed_entries_update_only_owned_blocks_and_preserve_other_bytes() {
        let project = TestDir::new("entries");
        let workspace = TestDir::new("entries-workspace");
        let first_plan = plan(vec![item(
            "map",
            ArtifactKind::Document,
            "docs/ai/project-map.md",
        )]);
        write(
            workspace.path().join("docs/ai/project-map.md"),
            "# 项目地图",
        );
        write(
            project.path().join("CLAUDE.md"),
            "prefix\n\nuser instructions\n",
        );
        let mut first =
            install_planned_artifacts(project.path(), workspace.path(), &first_plan, None)
                .expect("install first plan");
        install_managed_entries(project.path(), &mut first).expect("insert managed entries");
        mark_completed(&mut first);
        save_ownership_manifest(project.path(), &first).expect("complete first plan");
        let first_claude = read(project.path().join("CLAUDE.md"));
        assert!(first_claude.starts_with("prefix\n\nuser instructions\n"));
        assert!(first_claude.contains(MANAGED_BLOCK_START));
        assert!(first_claude.contains("docs/ai/project-map.md"));
        write(
            project.path().join("CLAUDE.md"),
            &format!("{first_claude}\nuser suffix without normalization"),
        );

        let second_plan = plan(vec![
            item("map", ArtifactKind::Document, "docs/ai/project-map.md"),
            item(
                "router",
                ArtifactKind::Rule,
                ".claude/rules/project/README.md",
            ),
        ]);
        write(
            workspace.path().join(".claude/rules/project/README.md"),
            "# 规则路由",
        );
        let mut second =
            install_planned_artifacts(project.path(), workspace.path(), &second_plan, Some(&first))
                .expect("install second plan");
        install_managed_entries(project.path(), &mut second).expect("update managed entries");
        let updated = read(project.path().join("CLAUDE.md"));
        assert!(updated.starts_with("prefix\n\nuser instructions\n"));
        assert!(updated.ends_with("\nuser suffix without normalization"));
        assert_eq!(updated.matches(MANAGED_BLOCK_START).count(), 1);
        assert!(updated.contains(".claude/rules/project/README.md"));
        assert!(project.path().join("AGENTS.md").is_file());

        let block_start = updated.find(MANAGED_BLOCK_START).expect("block start");
        let mut tampered = updated.clone();
        tampered.insert_str(block_start + MANAGED_BLOCK_START.len(), "\ntampered");
        write(project.path().join("CLAUDE.md"), &tampered);
        let issues = install_managed_entries(project.path(), &mut second)
            .expect_err("modified managed block must conflict");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "install.entry.modified"));
        assert_eq!(read(project.path().join("CLAUDE.md")), tampered);
    }

    #[test]
    fn managed_entries_resume_an_applied_target_from_the_matching_journal() {
        let project = TestDir::new("entry-resume");
        let mut manifest = empty_manifest("entry-resume-plan");
        let block = managed_block(&manifest);
        let applied = format!("{block}\n");
        write(project.path().join("CLAUDE.md"), &applied);
        let mut entries = BTreeMap::new();
        entries.insert(
            "CLAUDE.md".to_string(),
            InstallJournalEntry {
                operation: JournalOperation::WriteManagedEntry,
                baseline_sha256: None,
                expected_sha256: Some(content_sha256(applied.as_bytes())),
                state: JournalEntryState::Applied,
                link_target: None,
            },
        );
        save_install_journal(
            project.path(),
            &InstallJournal {
                schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
                plan_sha256: manifest.plan_sha256.clone(),
                entries,
            },
        )
        .expect("save entry journal");

        install_managed_entries(project.path(), &mut manifest)
            .expect("resume remaining managed entry");
        assert_eq!(manifest.managed_entries.len(), 2);
        assert!(project.path().join("AGENTS.md").is_file());
        let journal = load_install_journal(project.path())
            .expect("load entry journal")
            .expect("journal retained");
        assert!(journal
            .entries
            .values()
            .filter(|entry| entry.operation == JournalOperation::WriteManagedEntry)
            .all(|entry| entry.state == JournalEntryState::Applied));
    }

    #[test]
    fn completed_manifest_is_atomic_and_verifies_owned_hashes() {
        let project = TestDir::new("manifest-project");
        let workspace = TestDir::new("manifest-workspace");
        let artifact_plan = plan(vec![item(
            "map",
            ArtifactKind::Document,
            "docs/ai/project-map.md",
        )]);
        write(
            workspace.path().join("docs/ai/project-map.md"),
            "# 项目地图",
        );
        let mut manifest =
            install_planned_artifacts(project.path(), workspace.path(), &artifact_plan, None)
                .expect("install artifact");
        install_managed_entries(project.path(), &mut manifest).expect("install entries");
        mark_completed(&mut manifest);
        assert!(journal_path(project.path())
            .expect("journal retained before completed manifest")
            .exists());
        save_ownership_manifest(project.path(), &manifest).expect("save manifest");
        assert!(!journal_path(project.path())
            .expect("journal removed after completed manifest")
            .exists());

        let loaded = load_ownership_manifest(project.path())
            .expect("load manifest")
            .expect("manifest exists");
        assert_eq!(loaded, manifest);
        assert!(verify_ownership_manifest(project.path(), &loaded).is_empty());

        write(project.path().join("docs/ai/project-map.md"), "# 被修改");
        assert!(verify_ownership_manifest(project.path(), &loaded)
            .iter()
            .any(|issue| issue.code == "manifest.hash.mismatch"));
    }

    #[test]
    fn completed_manifest_refuses_pending_journal_entries_and_missing_run_metadata() {
        let project = TestDir::new("manifest-pending");
        let workspace = TestDir::new("manifest-pending-workspace");
        let artifact_plan = plan(vec![item(
            "map",
            ArtifactKind::Document,
            "docs/ai/project-map.md",
        )]);
        write(
            workspace.path().join("docs/ai/project-map.md"),
            "# 项目地图",
        );
        let mut manifest =
            install_planned_artifacts(project.path(), workspace.path(), &artifact_plan, None)
                .expect("install artifact");
        manifest.state = InitializationRunState::Completed;
        let verification = verify_ownership_manifest(project.path(), &manifest);
        assert!(verification
            .iter()
            .any(|issue| issue.code == "manifest.inventory-hash.missing"));

        install_managed_entries(project.path(), &mut manifest).expect("install entries");
        mark_completed(&mut manifest);
        let mut journal = load_install_journal(project.path())
            .expect("load journal")
            .expect("journal exists");
        journal
            .entries
            .values_mut()
            .next()
            .expect("journal entry")
            .state = JournalEntryState::Pending;
        save_install_journal(project.path(), &journal).expect("save pending journal");
        assert!(save_ownership_manifest(project.path(), &manifest)
            .expect_err("pending journal must block completion")
            .contains("pending"));
        assert!(!project
            .path()
            .join("docs/ai/.initialization-manifest.json")
            .exists());
        assert!(journal_path(project.path()).expect("journal path").exists());
    }

    #[cfg(unix)]
    #[test]
    fn unreadable_existing_targets_are_conflicts_not_missing_files() {
        use std::os::unix::fs::PermissionsExt;

        let project = TestDir::new("permission-project");
        let workspace = TestDir::new("permission-workspace");
        let artifact_plan = plan(vec![item(
            "map",
            ArtifactKind::Document,
            "docs/ai/project-map.md",
        )]);
        write(workspace.path().join("docs/ai/project-map.md"), "# 新内容");
        let target = project.path().join("docs/ai/project-map.md");
        write(&target, "# 用户内容");
        let mut permissions = fs::metadata(&target)
            .expect("target metadata")
            .permissions();
        permissions.set_mode(0o000);
        fs::set_permissions(&target, permissions).expect("make target unreadable");

        let issues =
            install_planned_artifacts(project.path(), workspace.path(), &artifact_plan, None)
                .expect_err("unreadable target must conflict");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "install.target.read"));
    }

    #[test]
    fn copy_fallback_synchronizes_non_empty_agent_assets() {
        let project = TestDir::new("copy-assets");
        write(
            project.path().join(".claude/rules/project/README.md"),
            "# 规则路由",
        );
        write(
            project
                .path()
                .join(".claude/skills/release-verification/SKILL.md"),
            "# 发布验证",
        );
        let mut manifest = empty_manifest("agent-copy-plan");

        let mode = share_agent_assets_for_test(project.path(), &mut manifest, false)
            .expect("copy fallback");
        assert_eq!(mode, AgentAssetMode::ManagedCopy);
        assert_eq!(
            read(project.path().join(".agents/rules/project/README.md")),
            read(project.path().join(".claude/rules/project/README.md"))
        );
        assert_eq!(
            read(
                project
                    .path()
                    .join(".agents/skills/release-verification/SKILL.md")
            ),
            read(
                project
                    .path()
                    .join(".claude/skills/release-verification/SKILL.md")
            )
        );
        assert!(!project.path().join(".agents/scripts").exists());
        assert_eq!(manifest.agent_assets.len(), 2);
        assert!(manifest.agent_asset_targets.iter().any(|target| {
            target.path == ".agents/rules" && target.mode == AgentAssetMode::ManagedCopy
        }));
        install_managed_entries(project.path(), &mut manifest).expect("install entries");
        mark_completed(&mut manifest);
        assert!(verify_ownership_manifest(project.path(), &manifest).is_empty());

        write(
            project.path().join(".agents/rules/project/README.md"),
            "# 用户修改过副本",
        );
        let issues = share_agent_assets_for_test(project.path(), &mut manifest, false)
            .expect_err("modified managed copy must conflict");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "install.agent-copy.modified"));
    }

    #[test]
    fn copy_fallback_resumes_applied_targets_and_removes_only_unchanged_stale_copies() {
        let project = TestDir::new("copy-resume");
        let source_path = ".claude/rules/project/README.md";
        let target_path = ".agents/rules/project/README.md";
        let content = "# 规则路由";
        write(project.path().join(source_path), content);
        write(project.path().join(target_path), content);
        let mut manifest = empty_manifest("copy-resume-plan");
        let mut entries = BTreeMap::new();
        entries.insert(
            target_path.to_string(),
            InstallJournalEntry {
                operation: JournalOperation::WriteAgentCopy,
                baseline_sha256: None,
                expected_sha256: Some(content_sha256(content.as_bytes())),
                state: JournalEntryState::Applied,
                link_target: None,
            },
        );
        save_install_journal(
            project.path(),
            &InstallJournal {
                schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
                plan_sha256: manifest.plan_sha256.clone(),
                entries,
            },
        )
        .expect("save copy journal");

        share_agent_assets_for_test(project.path(), &mut manifest, false)
            .expect("resume copy target");
        assert_eq!(manifest.agent_assets.len(), 1);

        fs::remove_file(project.path().join(source_path)).expect("remove obsolete source");
        share_agent_assets_for_test(project.path(), &mut manifest, false)
            .expect("remove unchanged obsolete managed copy");
        assert!(!project.path().join(target_path).exists());
        assert!(!project.path().join(".agents/rules").exists());
        assert!(manifest.agent_assets.is_empty());
    }

    #[test]
    fn missing_managed_copy_with_pending_remove_is_marked_applied() {
        let project = TestDir::new("copy-remove-crash");
        let source_path = ".claude/rules/project/README.md";
        let target_path = ".agents/rules/project/README.md";
        let content = "# Rules";
        write(project.path().join(source_path), content);
        let mut manifest = empty_manifest("copy-remove-crash-plan");
        share_agent_assets_for_test(project.path(), &mut manifest, false)
            .expect("initial managed copy");
        let owned_hash = manifest
            .agent_assets
            .iter()
            .find(|asset| asset.path == target_path)
            .expect("owned target")
            .sha256
            .clone();
        let mut entries = BTreeMap::new();
        entries.insert(
            target_path.to_string(),
            InstallJournalEntry {
                operation: JournalOperation::RemoveAgentCopy,
                baseline_sha256: Some(owned_hash),
                expected_sha256: None,
                state: JournalEntryState::Pending,
                link_target: None,
            },
        );
        save_install_journal(
            project.path(),
            &InstallJournal {
                schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
                plan_sha256: manifest.plan_sha256.clone(),
                entries,
            },
        )
        .expect("save pending copy removal");
        fs::remove_file(project.path().join(source_path)).expect("remove obsolete source");
        fs::remove_file(project.path().join(target_path))
            .expect("simulate crash after deleting copy");

        share_agent_assets_for_test(project.path(), &mut manifest, false)
            .expect("resume managed-copy removal");
        let journal = load_install_journal(project.path())
            .expect("load journal")
            .expect("journal retained");
        assert_eq!(
            journal.entries[target_path].state,
            JournalEntryState::Applied
        );
        assert!(manifest.agent_assets.is_empty());
    }

    #[test]
    fn stale_copy_modified_by_the_user_remains_owned_and_conflicts() {
        let project = TestDir::new("copy-stale-modified");
        let source_path = ".claude/rules/project/README.md";
        let target_path = ".agents/rules/project/README.md";
        write(project.path().join(source_path), "# 规则路由");
        let mut manifest = empty_manifest("copy-stale-modified-plan");
        share_agent_assets_for_test(project.path(), &mut manifest, false)
            .expect("initial managed copy");
        fs::remove_file(project.path().join(source_path)).expect("remove obsolete source");
        write(project.path().join(target_path), "# 用户修改");

        let issues = share_agent_assets_for_test(project.path(), &mut manifest, false)
            .expect_err("modified stale copy must conflict");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "install.agent-copy.modified"));
        assert_eq!(read(project.path().join(target_path)), "# 用户修改");
        assert!(manifest
            .agent_assets
            .iter()
            .any(|asset| asset.path == target_path));
    }

    #[cfg(unix)]
    #[test]
    fn unix_sharing_uses_safe_relative_links_and_preserves_real_agents_directories() {
        use std::os::unix::fs::symlink;

        let project = TestDir::new("linked-assets");
        write(
            project.path().join(".claude/rules/project/README.md"),
            "# 规则路由",
        );
        write(
            project.path().join(".claude/skills/task/SKILL.md"),
            "# Task",
        );
        let mut manifest = empty_manifest("agent-link-plan");
        let mode = share_agent_assets_for_test(project.path(), &mut manifest, true)
            .expect("link agent assets");
        assert_eq!(mode, AgentAssetMode::RelativeSymlink);
        assert_eq!(
            fs::read_link(project.path().join(".agents/rules")).expect("rules link"),
            PathBuf::from("../.claude/rules")
        );
        assert_eq!(
            fs::read_link(project.path().join(".agents/skills")).expect("skills link"),
            PathBuf::from("../.claude/skills")
        );
        assert!(manifest.agent_asset_targets.iter().any(|target| {
            target.path == ".agents/rules"
                && target.mode == AgentAssetMode::RelativeSymlink
                && target.link_target.as_deref() == Some("../.claude/rules")
        }));
        install_managed_entries(project.path(), &mut manifest).expect("install managed entries");
        mark_completed(&mut manifest);
        assert!(verify_ownership_manifest(project.path(), &manifest).is_empty());
        fs::remove_file(project.path().join(".agents/rules")).expect("remove owned rules link");
        symlink("../.claude/skills", project.path().join(".agents/rules"))
            .expect("replace with wrong link");
        assert!(verify_ownership_manifest(project.path(), &manifest)
            .iter()
            .any(|issue| issue.code == "manifest.agent-target.link-mismatch"));

        let preserved = TestDir::new("preserved-assets");
        write(
            preserved.path().join(".claude/rules/project/README.md"),
            "# 新规则",
        );
        write(
            preserved.path().join(".agents/rules/user-owned.md"),
            "# 用户规则",
        );
        let mut preserved_manifest = empty_manifest("agent-preserved-plan");
        let preserved_mode =
            share_agent_assets_for_test(preserved.path(), &mut preserved_manifest, true)
                .expect("preserve real agents directory");
        assert_eq!(preserved_mode, AgentAssetMode::Preserved);
        assert!(preserved_manifest.agent_asset_targets.iter().any(|target| {
            target.path == ".agents/rules" && target.mode == AgentAssetMode::Preserved
        }));
        assert_eq!(
            read(preserved.path().join(".agents/rules/user-owned.md")),
            "# 用户规则"
        );
        assert!(!preserved
            .path()
            .join(".agents/rules/project/README.md")
            .exists());

        let escaped = TestDir::new("escaped-assets");
        let outside = TestDir::new("escaped-assets-outside");
        write(
            escaped.path().join(".claude/rules/project/README.md"),
            "# 新规则",
        );
        symlink(outside.path(), escaped.path().join(".agents")).expect("agents root escape");
        let mut escaped_manifest = empty_manifest("agent-escaped-plan");
        let issues = share_agent_assets_for_test(escaped.path(), &mut escaped_manifest, true)
            .expect_err("agents root symlink must fail");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "install.agent-target.unsafe"));
        assert!(!outside.path().join("rules").exists());
    }

    #[cfg(unix)]
    #[test]
    fn stale_platform_links_are_journaled_and_wrong_targets_are_preserved() {
        use std::os::unix::fs::symlink;

        let project = TestDir::new("stale-agent-link");
        let source_path = ".claude/rules/project/README.md";
        write(project.path().join(source_path), "# Rules");
        let mut manifest = empty_manifest("stale-agent-link-plan");
        share_agent_assets_for_test(project.path(), &mut manifest, true)
            .expect("create relative link");
        fs::remove_file(project.path().join(source_path)).expect("empty source root");

        share_agent_assets_for_test(project.path(), &mut manifest, true)
            .expect("remove stale platform link");
        assert!(!project.path().join(".agents/rules").exists());
        let journal = load_install_journal(project.path())
            .expect("load journal")
            .expect("journal retained");
        let removal = journal.entries.get(".agents/rules").expect("link removal");
        assert_eq!(removal.operation, JournalOperation::RemoveAgentLink);
        assert_eq!(removal.state, JournalEntryState::Applied);

        let wrong = TestDir::new("wrong-stale-agent-link");
        write(wrong.path().join(source_path), "# Rules");
        let mut wrong_manifest = empty_manifest("wrong-stale-link-plan");
        share_agent_assets_for_test(wrong.path(), &mut wrong_manifest, true)
            .expect("create owned relative link");
        fs::remove_file(wrong.path().join(source_path)).expect("empty source root");
        fs::remove_file(wrong.path().join(".agents/rules")).expect("remove owned link");
        symlink("../.claude/skills", wrong.path().join(".agents/rules"))
            .expect("install user replacement link");

        let issues = share_agent_assets_for_test(wrong.path(), &mut wrong_manifest, true)
            .expect_err("wrong replacement target must conflict");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "install.agent-link.remove-mismatch"));
        assert_eq!(
            fs::read_link(wrong.path().join(".agents/rules")).expect("wrong link preserved"),
            PathBuf::from("../.claude/skills")
        );
    }

    #[cfg(unix)]
    #[test]
    fn missing_platform_link_with_pending_remove_is_marked_applied() {
        let project = TestDir::new("agent-link-remove-crash");
        let source_path = ".claude/rules/project/README.md";
        write(project.path().join(source_path), "# Rules");
        let mut manifest = empty_manifest("agent-link-remove-crash-plan");
        share_agent_assets_for_test(project.path(), &mut manifest, true)
            .expect("create relative link");
        let expected = "../.claude/rules";
        let mut entries = BTreeMap::new();
        entries.insert(
            ".agents/rules".to_string(),
            InstallJournalEntry {
                operation: JournalOperation::RemoveAgentLink,
                baseline_sha256: Some(content_sha256(expected.as_bytes())),
                expected_sha256: None,
                state: JournalEntryState::Pending,
                link_target: Some(expected.to_string()),
            },
        );
        save_install_journal(
            project.path(),
            &InstallJournal {
                schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
                plan_sha256: manifest.plan_sha256.clone(),
                entries,
            },
        )
        .expect("save pending link removal");
        fs::remove_file(project.path().join(source_path)).expect("empty source root");
        fs::remove_file(project.path().join(".agents/rules")).expect("simulate crash after unlink");

        share_agent_assets_for_test(project.path(), &mut manifest, true)
            .expect("resume link removal");
        let journal = load_install_journal(project.path())
            .expect("load journal")
            .expect("journal retained");
        assert_eq!(
            journal.entries[".agents/rules"].state,
            JournalEntryState::Applied
        );
        assert!(manifest.agent_asset_targets.is_empty());
    }
}
