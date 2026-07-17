// 回收站 —— agent 无关。
// 所有 agent 共用一个 trash 目录：~/.claude/.session-viewer-trash/。
// 每个被删的 JSONL 旁边有一个 `<file>.meta` 旁车文件，记录原路径 / agent / 删除时间，
// restore 时凭它恢复到原位。删除/恢复操作本身不依赖 agent，只有"展示标题"需要
// 通过 `agents::source(agent)` 取得对应的解析逻辑。

use std::fs;
use std::path::PathBuf;

use serde_json::Value;

use crate::agents;
use crate::types::TrashItem;
use crate::util::{home, is_jsonl, now_millis};

pub fn trash_dir() -> PathBuf {
    let d = home().join(".claude").join(".session-viewer-trash");
    let _ = fs::create_dir_all(&d);
    d
}

pub fn soft_delete(agent: &str, path: &str, project_label: &str) -> Result<(), String> {
    let src = PathBuf::from(path);
    if !src.exists() {
        return Err("Session file does not exist".to_string());
    }
    let base = src
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "session.jsonl".to_string());
    let now = now_millis();
    let trash_name = format!("{now}-{base}");
    let td = trash_dir();
    let dest = td.join(&trash_name);
    fs::rename(&src, &dest)
        .or_else(|_| {
            fs::copy(&src, &dest)
                .and_then(|_| fs::remove_file(&src))
                .map(|_| ())
        })
        .map_err(|e| format!("Failed to move to trash: {e}"))?;
    let meta = serde_json::json!({
        "agent": agent,
        "originalPath": path,
        "projectLabel": project_label,
        "deletedAt": now,
    });
    fs::write(td.join(format!("{trash_name}.meta")), meta.to_string())
        .map_err(|e| format!("Failed to write trash metadata: {e}"))?;
    Ok(())
}

pub fn list() -> Result<Vec<TrashItem>, String> {
    let td = trash_dir();
    let mut out = Vec::new();
    let entries = fs::read_dir(&td).map_err(|e| format!("Failed to read trash: {e}"))?;
    for f in entries.flatten() {
        let fp = f.path();
        if !is_jsonl(&fp) {
            continue;
        }
        let trash_file = f.file_name().to_string_lossy().to_string();
        let meta_path = td.join(format!("{trash_file}.meta"));
        let meta: Value = fs::read_to_string(&meta_path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or(Value::Null);
        let agent = meta
            .get("agent")
            .and_then(|x| x.as_str())
            .unwrap_or("claude")
            .to_string();
        let original_path = meta
            .get("originalPath")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let project_label = meta
            .get("projectLabel")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let deleted_at = meta.get("deletedAt").and_then(|x| x.as_u64()).unwrap_or(0);
        let title = agents::source(&agent)
            .map(|s| s.trash_title(&fp))
            .unwrap_or_default();
        out.push(TrashItem {
            trash_file,
            agent,
            project_label,
            original_path,
            trash_path: fp.to_string_lossy().to_string(),
            deleted_at,
            title,
            size: fs::metadata(&fp).map(|m| m.len()).unwrap_or(0),
        });
    }
    out.sort_by_key(|t| std::cmp::Reverse(t.deleted_at));
    Ok(out)
}

pub fn restore(trash_file: &str) -> Result<(), String> {
    let td = trash_dir();
    let src = td.join(trash_file);
    let meta_path = td.join(format!("{trash_file}.meta"));
    let s = fs::read_to_string(&meta_path)
        .map_err(|_| "Missing metadata — cannot determine restore location".to_string())?;
    let v: Value = serde_json::from_str(&s).map_err(|e| format!("Corrupted metadata: {e}"))?;
    let original_path = v
        .get("originalPath")
        .and_then(|x| x.as_str())
        .ok_or("Metadata missing original path")?;
    let dest = PathBuf::from(original_path);
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {e}"))?;
    }
    fs::rename(&src, &dest).map_err(|e| format!("Failed to restore: {e}"))?;
    let _ = fs::remove_file(&meta_path);
    Ok(())
}

pub fn permanent_delete(trash_file: &str) -> Result<(), String> {
    let td = trash_dir();
    fs::remove_file(td.join(trash_file))
        .map_err(|e| format!("Failed to delete permanently: {e}"))?;
    let _ = fs::remove_file(td.join(format!("{trash_file}.meta")));
    Ok(())
}

pub fn empty() -> Result<(), String> {
    let td = trash_dir();
    let entries = fs::read_dir(&td).map_err(|e| format!("Failed to read trash: {e}"))?;
    for f in entries.flatten() {
        let _ = fs::remove_file(f.path());
    }
    Ok(())
}
