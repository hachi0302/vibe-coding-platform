use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::initialization::InitializationStage;
use super::types::{ArtifactPlan, ProjectInventory, ValidationIssue};

pub(super) const CONTEXT_MEMORY_DIR: &str = ".vibe-coding-platform/context-memory";

fn safe_component(value: &str) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    let normalized = normalized
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-");
    if normalized.is_empty() {
        "root".to_string()
    } else {
        normalized
    }
}

fn ensure_real_directory(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            Err(format!("临时上下文路径不是安全目录：{}", path.display()))
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => fs::create_dir(path)
            .map_err(|error| format!("无法创建临时上下文目录 {}：{error}", path.display())),
        Err(error) => Err(format!(
            "无法检查临时上下文目录 {}：{error}",
            path.display()
        )),
    }
}

fn write_json(path: &Path, value: &serde_json::Value) -> Result<(), String> {
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(format!("临时上下文文件不安全：{}", path.display()));
    }
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| format!("无法序列化临时上下文：{error}"))?;
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    if temporary.exists() {
        fs::remove_file(&temporary).map_err(|error| format!("无法清理临时上下文文件：{error}"))?;
    }
    fs::write(&temporary, bytes)
        .map_err(|error| format!("无法写入临时上下文 {}：{error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("无法安装临时上下文 {}：{error}", path.display()))
}

fn write_text(path: &Path, content: &str) -> Result<(), String> {
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink() || !metadata.is_file())
    {
        return Err(format!("临时上下文文件不安全：{}", path.display()));
    }
    let temporary = path.with_extension(format!("tmp-{}", std::process::id()));
    if temporary.exists() {
        fs::remove_file(&temporary).map_err(|error| format!("无法清理临时上下文文件：{error}"))?;
    }
    fs::write(&temporary, content)
        .map_err(|error| format!("无法写入临时上下文 {}：{error}", temporary.display()))?;
    fs::rename(&temporary, path)
        .map_err(|error| format!("无法安装临时上下文 {}：{error}", path.display()))
}

fn memory_root(workspace: &Path) -> PathBuf {
    workspace.join(CONTEXT_MEMORY_DIR)
}

pub(super) fn prepare_context_memory(
    workspace: &Path,
    inventory: &ProjectInventory,
) -> Result<(), String> {
    let control = workspace.join(".vibe-coding-platform");
    ensure_real_directory(&control)?;
    let root = memory_root(workspace);
    ensure_real_directory(&root)?;
    let shards = root.join("shards");
    ensure_real_directory(&shards)?;
    let notes = root.join("notes");
    ensure_real_directory(&notes)?;
    let diagnostics = root.join("diagnostics");
    ensure_real_directory(&diagnostics)?;

    let mut module_index = Vec::new();
    for (index, module) in inventory.modules.iter().enumerate() {
        let shard_name = format!(
            "shards/{index:03}-{}.json",
            safe_component(if module.path == "." {
                &module.name
            } else {
                &module.path
            })
        );
        let files = inventory
            .files
            .iter()
            .filter(|file| {
                file.module
                    .as_deref()
                    .is_some_and(|owner| owner == module.name || owner == module.path)
            })
            .map(|file| {
                json!({
                    "path": file.path,
                    "kind": file.kind,
                    "size": file.size,
                    "sha256": file.sha256,
                })
            })
            .collect::<Vec<_>>();
        write_json(
            &root.join(&shard_name),
            &json!({
                "schemaVersion": 1,
                "module": module,
                "files": files,
            }),
        )?;
        module_index.push(json!({
            "name": module.name,
            "path": module.path,
            "kind": module.kind,
            "sourceRoots": module.source_roots,
            "manifests": module.manifests,
            "shard": shard_name,
        }));
    }

    let unowned = inventory
        .files
        .iter()
        .filter(|file| file.module.is_none())
        .map(|file| {
            json!({
                "path": file.path,
                "kind": file.kind,
                "size": file.size,
                "sha256": file.sha256,
            })
        })
        .collect::<Vec<_>>();
    write_json(
        &root.join("shards/unowned.json"),
        &json!({ "schemaVersion": 1, "scope": "unowned", "files": unowned }),
    )?;
    write_json(
        &root.join("index.json"),
        &json!({
            "schemaVersion": 1,
            "projectName": inventory.project_name,
            "layers": inventory.layers,
            "moduleCount": inventory.modules.len(),
            "sourceRootCount": inventory.source_roots.len(),
            "fileCount": inventory.files.len(),
            "sourceRoots": inventory.source_roots,
            "commands": inventory.commands,
            "riskKeyCount": inventory.risk_keys.len(),
            "modules": module_index,
            "unownedShard": "shards/unowned.json",
        }),
    )?;
    write_json(
        &root.join("document-templates.json"),
        &super::document_templates::catalog_json(),
    )?;
    write_text(
        &root.join("document-template-library.md"),
        super::document_templates::template_library_markdown(),
    )?;

    let note_path = notes.join("project-memory.md");
    match fs::symlink_metadata(&note_path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            return Err(format!("临时项目记忆文件不安全：{}", note_path.display()));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::write(
        &note_path,
            "# 临时项目记忆\n\n## 已确认架构与边界\n\n## 已确认可复用资产\n\n## 模块与源码根覆盖\n\n## 风险、历史陷阱与验证边界\n",
        )
        .map_err(|error| format!("无法创建临时项目记忆 {}：{error}", note_path.display()))?;
        }
        Err(error) => {
            return Err(format!(
                "无法检查临时项目记忆 {}：{error}",
                note_path.display()
            ));
        }
    }
    Ok(())
}

pub(super) fn update_stage_context(
    workspace: &Path,
    stage: InitializationStage,
    issues: &[ValidationIssue],
    plan: Option<&ArtifactPlan>,
) -> Result<(), String> {
    let root = memory_root(workspace);
    write_json(
        &root.join("validation-issues.json"),
        &serde_json::to_value(issues).map_err(|error| format!("无法序列化校验问题：{error}"))?,
    )?;
    let artifacts = plan
        .map(|plan| {
            plan.artifacts
                .iter()
                .filter(|artifact| stage.kind().is_none_or(|kind| artifact.kind == kind))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    write_json(
        &root.join("stage-artifacts.json"),
        &serde_json::to_value(artifacts).map_err(|error| format!("无法序列化阶段产物：{error}"))?,
    )
}

pub(super) fn save_stage_diagnostic(
    workspace: &Path,
    stage: InitializationStage,
    attempt: u32,
    exit_code: Option<i32>,
    diagnostic_tail: &str,
    issues: &[ValidationIssue],
) -> Result<(), String> {
    let path = memory_root(workspace)
        .join("diagnostics")
        .join(format!("{}-{attempt}.json", stage.name()));
    write_json(
        &path,
        &json!({
            "schemaVersion": 1,
            "stage": stage.name(),
            "attempt": attempt,
            "exitCode": exit_code,
            "diagnosticTail": diagnostic_tail,
            "issues": issues,
        }),
    )
}

pub(super) fn prompt_contract() -> &'static str {
    r#"临时上下文记忆协议：
- 先读取 `.vibe-coding-platform/context-memory/index.json`，再按需读取对应 `shards/*.json`；不要反复全仓搜索。
- 每确认一个模块、源码根、入口、复用资产或历史陷阱，立即把简洁结论与真实 path + symbol 追加到 `notes/project-memory.md`。
- 修复轮次和后续 documents/rules/skills 阶段必须先复用已有 project-memory，只重读当前问题涉及的 shard 和源码。
- `validation-issues.json` 是本轮机器校验问题，`stage-artifacts.json` 是本阶段唯一目标清单。
- `document-templates.json` 与 `document-template-library.md` 是平台文档模板；它们只提供 IPS 风格的目录、中文章节与审查口径，不能推导或复制任何项目事实。命中触发条件时，必须按目标项目真实证据创建对应中文文档；证据不足的字段写入“待补信息”。
- context-memory 只是隔离工作区临时记忆，不得链接到产物、不得复制到原项目；初始化完成后平台会整体删除。"#
}
