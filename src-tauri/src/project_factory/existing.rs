use std::fs;
use std::path::{Path, PathBuf};

use super::docs::{project_file_contents, project_files_named, project_layers, ProjectLayers};
use super::initialization_state::{
    load_initialization_state, load_ownership_manifest, verify_ownership_manifest,
    MANAGED_BLOCK_START,
};
use super::types::{
    ExistingProjectInitPreparation, ExistingProjectInitResult, ExistingProjectInitStatus,
    InitializationRunState, InitializationState, ValidationIssue,
};

const PLATFORM_INIT_V3_MARKER: &str = "<!-- vibe-coding-platform:init:v3 -->";
const MAX_DISCOVERY_DEPTH: usize = 6;

fn detected_stack(root: &Path, layers: ProjectLayers) -> Vec<String> {
    let package = project_file_contents(root, "package.json");
    let cargo = project_file_contents(root, "Cargo.toml");
    let maven = project_file_contents(root, "pom.xml");
    let gradle = format!(
        "{}\n{}",
        project_file_contents(root, "build.gradle"),
        project_file_contents(root, "build.gradle.kts")
    );
    let python = format!(
        "{}\n{}",
        project_file_contents(root, "pyproject.toml"),
        project_file_contents(root, "requirements.txt")
    );
    let mut stack = Vec::new();
    for (name, found) in [
        ("Vue", package.contains("\"vue\"")),
        ("React", package.contains("\"react\"")),
        ("Angular", package.contains("@angular/core")),
        ("Svelte", package.contains("\"svelte\"")),
        ("Node.js", !package.is_empty()),
        ("Rust", !cargo.is_empty()),
        (
            "Spring Boot",
            maven.contains("spring-boot") || gradle.contains("spring-boot"),
        ),
        ("Java/Kotlin", !maven.is_empty() || !gradle.is_empty()),
        ("Python", !python.is_empty()),
        ("Go", !project_files_named(root, "go.mod").is_empty()),
        (".NET", !project_files_named(root, "Program.cs").is_empty()),
    ] {
        if found && !stack.iter().any(|item| item == name) {
            stack.push(name.to_string());
        }
    }
    if stack.is_empty() {
        if layers.frontend {
            stack.push("Frontend".to_string());
        }
        if layers.backend {
            stack.push("Backend".to_string());
        }
    }
    stack
}

fn should_skip(name: &str) -> bool {
    matches!(
        name,
        ".git" | "node_modules" | "target" | "dist" | "build" | "vendor"
    )
}

fn collect_existing(root: &Path, current: &Path, depth: usize, output: &mut Vec<String>) {
    if depth > MAX_DISCOVERY_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    let mut entries = entries.flatten().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if let Ok(relative) = path.strip_prefix(root) {
            output.push(relative.to_string_lossy().replace('\\', "/"));
        }
        if metadata.is_dir() && !should_skip(&entry.file_name().to_string_lossy()) {
            collect_existing(root, &path, depth + 1, output);
        }
    }
}

fn list_existing(root: &Path, relative: &str) -> Vec<String> {
    let path = root.join(relative);
    if !path.exists() {
        return Vec::new();
    }
    let mut output = vec![relative.to_string()];
    if path.is_dir() {
        collect_existing(root, &path, 0, &mut output);
    }
    output.sort();
    output.dedup();
    output
}

/// Discovery only. This function deliberately performs no writes and installs no templates,
/// hooks, entries, rules, skills, or `.agents` assets.
pub fn prepare_existing_project_initialization(
    project_path: &str,
) -> Result<ExistingProjectInitPreparation, String> {
    let root = Path::new(project_path);
    if !root.is_dir() {
        return Err("项目路径不存在或不是目录".to_string());
    }
    let layers = project_layers(root);
    if !layers.frontend && !layers.backend {
        return Err("未识别到前端或后端代码层；请确认项目根目录后再初始化".to_string());
    }
    let existing_docs = list_existing(root, "docs");
    let mut existing_agent_material = Vec::new();
    for relative in ["CLAUDE.md", "AGENTS.md", ".claude", ".agents"] {
        existing_agent_material.extend(list_existing(root, relative));
    }
    existing_agent_material.sort();
    existing_agent_material.dedup();
    Ok(ExistingProjectInitPreparation {
        project_path: root.to_string_lossy().to_string(),
        layers,
        detected_stack: detected_stack(root, layers),
        existing_docs,
        existing_agent_material,
    })
}

fn issue(code: &str, detail: impl Into<String>, stage: &str) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
        detail: detail.into(),
        path: None,
        stage: Some(stage.to_string()),
    }
}

fn phase_for_state(state: InitializationRunState) -> &'static str {
    match state {
        InitializationRunState::Preflight => "scan",
        InitializationRunState::SnapshotReady => "plan",
        InitializationRunState::PlanReady => "documents",
        InitializationRunState::DocumentsReady => "rules",
        InitializationRunState::RulesReady => "skills",
        InitializationRunState::SkillsReady | InitializationRunState::Installing => "install",
        InitializationRunState::Verifying => "verify",
        InitializationRunState::Completed => "complete",
        InitializationRunState::Failed => "failed",
        InitializationRunState::Interrupted => "interrupted",
        InitializationRunState::Conflict => "conflict",
    }
}

fn percent_for_phase(phase: &str) -> u8 {
    match phase {
        "scan" => 5,
        "plan" => 18,
        "documents" => 34,
        "rules" => 50,
        "skills" => 64,
        "install" => 78,
        "verify" => 90,
        "complete" => 100,
        _ => 0,
    }
}

fn warning_details(state: &InitializationState) -> Vec<String> {
    state
        .warnings
        .iter()
        .map(|warning| warning.detail.clone())
        .collect()
}

fn status_from_state(state: InitializationState) -> ExistingProjectInitStatus {
    let phase = phase_for_state(state.state).to_string();
    let warnings = warning_details(&state);
    let needs_attention = matches!(
        state.state,
        InitializationRunState::Failed
            | InitializationRunState::Conflict
            | InitializationRunState::Completed
    );
    let status = if needs_attention {
        "needs-attention"
    } else {
        "incomplete"
    };
    ExistingProjectInitStatus {
        initialized: false,
        status: status.to_string(),
        phase: phase.clone(),
        marker_version: None,
        run_id: Some(state.run_id.clone()),
        percent: percent_for_phase(&phase),
        detail: state
            .conflicts
            .first()
            .or_else(|| state.issues.first())
            .map(|item| item.detail.clone())
            .unwrap_or_else(|| "已有未完成初始化，可从最后有效节点继续".to_string()),
        attempt: state.attempt,
        sequence: state.updated_at_unix_ms,
        recoverable: !matches!(
            state.state,
            InitializationRunState::Conflict | InitializationRunState::Completed
        ),
        issues: state.issues,
        conflicts: state.conflicts,
        warnings,
        artifact_totals: (state.artifact_totals.total > 0).then_some(state.artifact_totals),
    }
}

fn attention_status(detail: impl Into<String>, code: &str) -> ExistingProjectInitStatus {
    let detail = detail.into();
    ExistingProjectInitStatus {
        initialized: false,
        status: "needs-attention".to_string(),
        phase: "failed".to_string(),
        marker_version: None,
        run_id: None,
        percent: 0,
        detail: detail.clone(),
        attempt: 0,
        sequence: 0,
        recoverable: false,
        issues: vec![issue(code, detail, "verify")],
        conflicts: Vec::new(),
        warnings: Vec::new(),
        artifact_totals: None,
    }
}

pub fn existing_project_init_status(
    project_path: &str,
) -> Result<ExistingProjectInitStatus, String> {
    let root = Path::new(project_path);
    if !root.is_dir() {
        return Err("项目路径不存在或不是目录".to_string());
    }
    let manifest_path = root.join("docs/ai/.initialization-manifest.json");
    if manifest_path.exists() {
        let manifest = match load_ownership_manifest(root) {
            Ok(Some(manifest)) => manifest,
            Ok(None) => {
                return Ok(attention_status(
                    "v4 所有权 manifest 消失，无法确认初始化结果",
                    "manifest.missing",
                ));
            }
            Err(error) => {
                return Ok(attention_status(error, "manifest.invalid"));
            }
        };
        let issues = verify_ownership_manifest(root, &manifest);
        if !issues.is_empty() {
            let detail = issues
                .iter()
                .map(|item| format!("{}: {}", item.code, item.detail))
                .collect::<Vec<_>>()
                .join("；");
            let mut status = attention_status(detail, "manifest.verification-failed");
            status.issues = issues;
            status.run_id = Some(manifest.run_id);
            status.artifact_totals = Some(manifest.artifact_totals);
            return Ok(status);
        }
        return Ok(ExistingProjectInitStatus {
            initialized: true,
            status: "current-v4".to_string(),
            phase: "complete".to_string(),
            marker_version: Some("v4".to_string()),
            run_id: Some(manifest.run_id),
            percent: 100,
            detail: "初始化已完成并通过所有权校验".to_string(),
            attempt: 0,
            sequence: manifest.completed_at_unix_ms,
            recoverable: false,
            issues: Vec::new(),
            conflicts: manifest.conflicts,
            warnings: manifest
                .diagnostics
                .iter()
                .map(|warning| warning.detail.clone())
                .collect(),
            artifact_totals: Some(manifest.artifact_totals),
        });
    }

    match load_initialization_state(root) {
        Ok(Some(state)) => return Ok(status_from_state(state)),
        Ok(None) => {}
        Err(error) => return Ok(attention_status(error, "state.invalid")),
    }

    let has_v3_marker = fs::read_to_string(root.join("CLAUDE.md"))
        .map(|content| content.contains(PLATFORM_INIT_V3_MARKER))
        .unwrap_or(false);
    if has_v3_marker {
        return Ok(ExistingProjectInitStatus {
            initialized: true,
            status: "legacy-v3".to_string(),
            phase: "scan".to_string(),
            marker_version: Some("v3".to_string()),
            run_id: None,
            percent: 0,
            detail: "检测到旧版 v3 标记；文件保持不变，可显式启动 v4 初始化".to_string(),
            attempt: 0,
            sequence: 0,
            recoverable: true,
            issues: Vec::new(),
            conflicts: Vec::new(),
            warnings: Vec::new(),
            artifact_totals: None,
        });
    }
    let has_unowned_v4_block = ["CLAUDE.md", "AGENTS.md"].iter().any(|relative| {
        fs::read_to_string(root.join(relative))
            .map(|content| content.contains(MANAGED_BLOCK_START))
            .unwrap_or(false)
    });
    if has_unowned_v4_block {
        return Ok(attention_status(
            "发现 v4 托管入口但缺少 completed manifest，无法证明文件所有权",
            "manifest.entry-without-ownership",
        ));
    }
    Ok(ExistingProjectInitStatus {
        initialized: false,
        status: "not-initialized".to_string(),
        phase: "scan".to_string(),
        marker_version: None,
        run_id: None,
        percent: 0,
        detail: "尚未初始化".to_string(),
        attempt: 0,
        sequence: 0,
        recoverable: true,
        issues: Vec::new(),
        conflicts: Vec::new(),
        warnings: Vec::new(),
        artifact_totals: None,
    })
}

/// Finalize is now a read-only compatibility command. Installation and verification happen
/// atomically in the v4 orchestrator; this function never calls prepare or recreates v3 assets.
pub fn finalize_existing_project_initialization(
    project_path: &str,
) -> Result<ExistingProjectInitResult, String> {
    let status = existing_project_init_status(project_path)?;
    if status.status != "current-v4" {
        return Err(format!(
            "项目不是已验证的 current-v4 状态：{}",
            status.detail
        ));
    }
    let root = PathBuf::from(project_path);
    let manifest = load_ownership_manifest(&root)?
        .ok_or_else(|| "current-v4 状态缺少所有权 manifest".to_string())?;
    let layers = project_layers(&root);
    Ok(ExistingProjectInitResult {
        project_path: root.to_string_lossy().to_string(),
        status: status.status,
        phase: status.phase,
        run_id: manifest.run_id,
        percent: 100,
        detail: status.detail,
        attempt: status.attempt,
        sequence: status.sequence,
        recoverable: false,
        issues: status.issues,
        conflicts: status.conflicts,
        warnings: status.warnings,
        artifact_totals: manifest.artifact_totals,
        layers: Some(layers),
        detected_stack: detected_stack(&root, layers),
        generated: manifest
            .artifacts
            .into_iter()
            .map(|artifact| artifact.path)
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::{existing_project_init_status, prepare_existing_project_initialization};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture(name: &str) -> std::path::PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("vibe-existing-{name}-{suffix}"));
        fs::create_dir_all(&root).expect("fixture root");
        root
    }

    #[test]
    fn prepare_is_discovery_only() {
        let root = fixture("pure-prepare");
        fs::write(root.join("package.json"), r#"{"dependencies":{"vue":"3"}}"#).expect("package");
        let before = fs::read_dir(&root).expect("before").count();
        let preparation =
            prepare_existing_project_initialization(&root.to_string_lossy()).expect("prepare");
        let after = fs::read_dir(&root).expect("after").count();

        assert!(preparation.layers.frontend);
        assert_eq!(before, after);
        assert!(!root.join(".claude").exists());
        assert!(!root.join(".agents").exists());
        assert!(!root.join("docs").exists());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn v3_marker_is_read_only_legacy_classification() {
        let root = fixture("legacy");
        fs::write(
            root.join("CLAUDE.md"),
            "<!-- vibe-coding-platform:init:v3 -->\n",
        )
        .expect("legacy entry");
        let status = existing_project_init_status(&root.to_string_lossy()).expect("status");

        assert_eq!(status.status, "legacy-v3");
        assert_eq!(status.marker_version.as_deref(), Some("v3"));
        assert_eq!(
            fs::read_to_string(root.join("CLAUDE.md")).expect("unchanged"),
            "<!-- vibe-coding-platform:init:v3 -->\n"
        );
        assert!(!root.join("docs").exists());
        let _ = fs::remove_dir_all(root);
    }
}
