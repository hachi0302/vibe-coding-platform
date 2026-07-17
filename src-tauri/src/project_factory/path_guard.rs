use std::fs;
use std::path::PathBuf;

fn valid_project_name(project_name: &str) -> bool {
    !project_name.is_empty()
        && project_name.len() <= 80
        && project_name
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

pub fn preview_target_path(parent_path: &str, project_name: &str) -> Result<PathBuf, String> {
    if !valid_project_name(project_name) {
        return Err("项目名称只能包含英文、数字、- 或 _，且长度不超过 80".to_string());
    }
    let parent = PathBuf::from(parent_path);
    if !parent.is_absolute() {
        return Err("项目路径必须是绝对路径".to_string());
    }
    Ok(parent.join(project_name))
}

pub fn validate_target_dir(parent_path: &str, project_name: &str) -> Result<PathBuf, String> {
    let parent = PathBuf::from(parent_path);
    if !parent.is_dir() {
        return Err("项目父路径不存在或不是目录".to_string());
    }
    let target = preview_target_path(parent_path, project_name)?;
    if target.exists() {
        let mut entries =
            fs::read_dir(&target).map_err(|error| format!("无法读取目标目录：{error}"))?;
        if entries.next().is_some() {
            return Err("目标项目目录已存在且非空，不会覆盖已有文件".to_string());
        }
    }
    Ok(target)
}
