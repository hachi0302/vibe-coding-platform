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
    AgentAssetMode, ArtifactKind, ArtifactPlan, InitializationRunState, InitializationState,
    ManagedAgentAsset, ManagedEntryOwnership, OwnedArtifact, OwnershipManifest, ValidationIssue,
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

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallJournal {
    schema_version: u32,
    plan_sha256: String,
    #[serde(default)]
    started: BTreeMap<String, String>,
}

#[derive(Debug)]
struct EntryCandidate {
    path: String,
    bytes: Vec<u8>,
    block_sha256: String,
}

#[derive(Debug)]
struct CopyCandidate {
    path: String,
    bytes: Vec<u8>,
    sha256: String,
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
                if let Some(owned) = previous_artifacts.get(item.target_path.as_str()) {
                    if current_hash != owned.sha256 {
                        issues.push(issue(
                            "install.target.modified",
                            "平台此前生成的文件已被修改，拒绝覆盖用户改动",
                            Some(&item.target_path),
                            "install",
                        ));
                    }
                } else {
                    let journal_owns = journal.as_ref().is_some_and(|journal| {
                        journal.plan_sha256 == plan_sha256
                            && journal
                                .started
                                .get(&item.target_path)
                                .is_some_and(|expected| expected == &current_hash)
                    });
                    if !journal_owns {
                        issues.push(issue(
                            "install.target.unowned",
                            "目标文件没有 completed v4 manifest 所有权，拒绝覆盖",
                            Some(&item.target_path),
                            "install",
                        ));
                    }
                }
                Some(current_hash)
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
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

    if !issues.is_empty() {
        return Err(issues);
    }

    let mut journal = journal.unwrap_or(InstallJournal {
        schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
        plan_sha256: plan_sha256.clone(),
        started: BTreeMap::new(),
    });
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
        if current_sha256 != candidate.baseline_sha256 {
            return Err(vec![issue(
                "install.target.changed-during-install",
                "目标文件在预检后发生变化，已停止安装并保留恢复日志",
                Some(&candidate.path),
                "install",
            )]);
        }
        journal
            .started
            .insert(candidate.path.clone(), candidate.sha256.clone());
        save_install_journal(&project, &journal).map_err(|error| vec![error])?;
        atomic_write(&target, &candidate.bytes).map_err(|error| {
            vec![issue(
                "install.target.write",
                error,
                Some(&candidate.path),
                "install",
            )]
        })?;
    }

    if let Ok(path) = journal_path(&project) {
        let _ = fs::remove_file(path);
        if let Ok(directory) = state_directory(&project) {
            let _ = fs::remove_dir(directory);
        }
    }

    Ok(OwnershipManifest {
        schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
        platform_version: env!("CARGO_PKG_VERSION").to_string(),
        run_id: previous
            .map(|manifest| manifest.run_id.clone())
            .filter(|run_id| !run_id.is_empty())
            .or_else(|| {
                load_initialization_state(&project)
                    .ok()
                    .flatten()
                    .map(|state| state.run_id)
            })
            .unwrap_or_default(),
        state: InitializationRunState::Installing,
        plan_sha256,
        artifact_totals: artifact_totals(plan),
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
        agent_asset_mode: previous.and_then(|manifest| manifest.agent_asset_mode),
        conflicts: Vec::new(),
        diagnostics: Vec::new(),
        installed_at_unix_ms: unix_time_ms(),
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
) -> Result<EntryCandidate, ValidationIssue> {
    let path = project.join(relative);
    validate_target_ancestors(project, Path::new(relative))
        .map_err(|detail| issue("install.entry.unsafe", detail, Some(relative), "install"))?;
    let existing = match fs::read(&path) {
        Ok(bytes) => String::from_utf8(bytes).map_err(|_| {
            issue(
                "install.entry.encoding",
                "入口文件不是 UTF-8，无法安全插入托管块",
                Some(relative),
                "install",
            )
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(issue(
                "install.entry.read",
                format!("无法读取入口文件：{error}"),
                Some(relative),
                "install",
            ));
        }
    };
    let output = match managed_block_range(&existing)
        .map_err(|detail| issue("install.entry.malformed", detail, Some(relative), "install"))?
    {
        Some((start, end)) => {
            let current_block = &existing[start..end];
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
    Ok(EntryCandidate {
        path: relative.to_string(),
        bytes: output.into_bytes(),
        block_sha256: content_sha256(block.as_bytes()),
    })
}

pub fn install_managed_entries(
    project: &Path,
    manifest: &mut OwnershipManifest,
) -> Result<(), Vec<ValidationIssue>> {
    let project = canonical_directory(project, "项目")
        .map_err(|error| vec![issue("install.project.invalid", error, None, "install")])?;
    let block = managed_block(manifest);
    let previous: BTreeMap<&str, &ManagedEntryOwnership> = manifest
        .managed_entries
        .iter()
        .map(|entry| (entry.path.as_str(), entry))
        .collect();
    let mut issues = Vec::new();
    let mut candidates = Vec::new();
    for path in ["CLAUDE.md", "AGENTS.md"] {
        match preflight_entry(&project, path, &block, &previous) {
            Ok(candidate) => candidates.push(candidate),
            Err(error) => issues.push(error),
        }
    }
    if !issues.is_empty() {
        return Err(issues);
    }
    for candidate in &candidates {
        atomic_write(&project.join(&candidate.path), &candidate.bytes).map_err(|error| {
            vec![issue(
                "install.entry.write",
                error,
                Some(&candidate.path),
                "install",
            )]
        })?;
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
    let previous_assets: BTreeMap<&str, &ManagedAgentAsset> = manifest
        .agent_assets
        .iter()
        .map(|asset| (asset.path.as_str(), asset))
        .collect();
    let mut issues = Vec::new();
    let mut links = Vec::new();
    let mut copies = Vec::new();
    let mut preserved = false;

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
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
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
        if source_files.is_empty() {
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
                    .any(|path| path.starts_with(&owned_prefix));
                if !owns_destination {
                    preserved = true;
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
                    if let Ok(current) = fs::read(&target) {
                        let current_hash = content_sha256(&current);
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
                    copies.push(CopyCandidate {
                        path: display,
                        sha256: content_sha256(&bytes),
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

    if !issues.is_empty() {
        return Err(issues);
    }

    let mut linked_count = 0;
    let mut copied_count = 0;
    for name in links {
        let destination = project.join(".agents").join(&name);
        if fs::symlink_metadata(&destination).is_ok() {
            linked_count += 1;
            continue;
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
            Ok(()) => linked_count += 1,
            Err(_) => {
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

    let mut installed_assets = Vec::new();
    for candidate in copies {
        atomic_write(&project.join(&candidate.path), &candidate.bytes).map_err(|error| {
            vec![issue(
                "install.agent-copy.write",
                error,
                Some(&candidate.path),
                "install",
            )]
        })?;
        copied_count += 1;
        installed_assets.push(ManagedAgentAsset {
            path: candidate.path,
            sha256: candidate.sha256,
        });
    }
    installed_assets.sort_by(|left, right| left.path.cmp(&right.path));
    manifest.agent_assets = installed_assets;
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
    if target.exists() {
        let current =
            fs::read(&target).map_err(|error| format!("无法读取现有所有权 manifest：{error}"))?;
        let current: OwnershipManifest = serde_json::from_slice(&current)
            .map_err(|error| format!("现有所有权 manifest 不可解析，拒绝覆盖：{error}"))?;
        if current.schema_version != INITIALIZATION_STATE_SCHEMA_VERSION
            || current.state != InitializationRunState::Completed
        {
            return Err("现有文件不是受支持的 completed v4 manifest，拒绝覆盖".to_string());
        }
    }
    let bytes = serde_json::to_vec_pretty(manifest)
        .map_err(|error| format!("无法序列化所有权 manifest：{error}"))?;
    atomic_write(&target, &bytes)
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
    let mut issues = Vec::new();
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
    issues
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
        ArtifactKind, ArtifactPlan, ArtifactPlanItem, EvidenceReference, InitializationRunState,
        InitializationState,
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
        first.state = InitializationRunState::Completed;
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
        previous.state = InitializationRunState::Completed;
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
        let mut started = BTreeMap::new();
        started.insert("docs/ai/project-map.md".to_string(), hash);
        let journal = InstallJournal {
            schema_version: INITIALIZATION_STATE_SCHEMA_VERSION,
            plan_sha256: plan_sha256(&artifact_plan).expect("plan hash"),
            started,
        };
        save_install_journal(project.path(), &journal).expect("save interrupted journal");

        let manifest =
            install_planned_artifacts(project.path(), workspace.path(), &artifact_plan, None)
                .expect("resume journal-owned target");
        assert_eq!(manifest.artifacts.len(), 1);
        assert_eq!(read(project.path().join("docs/ai/project-map.md")), content);
        assert!(!journal_path(project.path()).expect("journal path").exists());
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
        first.state = InitializationRunState::Completed;
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
        manifest.state = InitializationRunState::Completed;
        save_ownership_manifest(project.path(), &manifest).expect("save manifest");

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
        let mut manifest = OwnershipManifest::default();

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
        let mut manifest = OwnershipManifest::default();
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

        let preserved = TestDir::new("preserved-assets");
        write(
            preserved.path().join(".claude/rules/project/README.md"),
            "# 新规则",
        );
        write(
            preserved.path().join(".agents/rules/user-owned.md"),
            "# 用户规则",
        );
        let mut preserved_manifest = OwnershipManifest::default();
        let preserved_mode =
            share_agent_assets_for_test(preserved.path(), &mut preserved_manifest, true)
                .expect("preserve real agents directory");
        assert_eq!(preserved_mode, AgentAssetMode::Preserved);
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
        let mut escaped_manifest = OwnershipManifest::default();
        let issues = share_agent_assets_for_test(escaped.path(), &mut escaped_manifest, true)
            .expect_err("agents root symlink must fail");
        assert!(issues
            .iter()
            .any(|issue| issue.code == "install.agent-target.unsafe"));
        assert!(!outside.path().join("rules").exists());
    }
}
