use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::types::{RequirementMaterialBundle, RequirementMaterialFile};

const MAX_FILES: usize = 200;
const MAX_FILE_TEXT_BYTES: u64 = 160 * 1024;
const MAX_TOTAL_TEXT_BYTES: usize = 900 * 1024;

const SKIPPED_DIRECTORIES: &[&str] = &[
    ".git",
    ".idea",
    ".vscode",
    "node_modules",
    "target",
    "dist",
    "build",
    "coverage",
    ".next",
    ".nuxt",
    ".venv",
    "venv",
    "__pycache__",
];

const TEXT_EXTENSIONS: &[&str] = &[
    "md",
    "markdown",
    "txt",
    "json",
    "jsonl",
    "yaml",
    "yml",
    "csv",
    "tsv",
    "toml",
    "xml",
    "html",
    "htm",
    "css",
    "scss",
    "sass",
    "less",
    "js",
    "mjs",
    "cjs",
    "ts",
    "tsx",
    "jsx",
    "vue",
    "svelte",
    "java",
    "kt",
    "kts",
    "groovy",
    "gradle",
    "py",
    "go",
    "rs",
    "c",
    "h",
    "cc",
    "cpp",
    "hpp",
    "cs",
    "php",
    "rb",
    "swift",
    "dart",
    "sql",
    "graphql",
    "gql",
    "proto",
    "sh",
    "bash",
    "zsh",
    "fish",
    "ps1",
    "properties",
    "conf",
    "ini",
    "env",
    "mmd",
];

const TEXT_FILE_NAMES: &[&str] = &[
    "dockerfile",
    "makefile",
    "readme",
    "license",
    "agents.md",
    "claude.md",
];

const WORD_EXTENSIONS: &[&str] = &["doc", "docx", "rtf"];
const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp", "heic", "svg"];

pub fn read_requirement_materials(path: &str) -> Result<RequirementMaterialBundle, String> {
    let selected = canonicalize_selected_path(path)?;
    let is_directory = selected.is_dir();
    let root = if is_directory {
        selected.clone()
    } else {
        selected
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(|| "无法确定所选文件的父目录".to_string())?
    };

    let mut paths = if is_directory {
        collect_files(&selected)?
    } else {
        vec![selected.clone()]
    };
    paths.sort_by_key(|path| relative_display(path, &root));

    let mut warnings = Vec::new();
    if paths.len() > MAX_FILES {
        warnings.push(format!(
            "资料共发现 {} 个文件，为避免超出分析上下文，仅处理前 {} 个（按相对路径排序）。",
            paths.len(),
            MAX_FILES
        ));
        paths.truncate(MAX_FILES);
    }

    let mut text = String::new();
    let mut files = Vec::with_capacity(paths.len());
    let mut total_text_bytes = 0usize;
    let mut reached_total_limit = false;

    for file_path in paths {
        let relative_path = relative_display(&file_path, &root);
        let absolute_path = file_path.to_string_lossy().to_string();
        let extension = extension_lowercase(&file_path);

        if reached_total_limit {
            let detail = "未读取：已达到本次资料正文总量上限".to_string();
            warnings.push(format!("{relative_path}：{detail}"));
            files.push(material_file(
                relative_path,
                absolute_path,
                classify_kind(&extension),
                false,
                detail,
            ));
            continue;
        }

        let extraction = extract_material_text(&file_path, &extension);
        match extraction {
            Extraction::Text {
                kind,
                text: extracted,
                truncated,
            } => {
                let header = format!("\n\n--- 资料：{relative_path} ---\n");
                let remaining =
                    MAX_TOTAL_TEXT_BYTES.saturating_sub(total_text_bytes + header.len());
                let (content, total_truncated) = truncate_utf8(&extracted, remaining);
                if content.is_empty() && !extracted.is_empty() {
                    reached_total_limit = true;
                    let detail = "未读取：已达到本次资料正文总量上限".to_string();
                    warnings.push(format!("{relative_path}：{detail}"));
                    files.push(material_file(
                        relative_path,
                        absolute_path,
                        kind,
                        false,
                        detail,
                    ));
                    continue;
                }

                text.push_str(&header);
                text.push_str(content);
                total_text_bytes = text.len();
                let was_truncated = truncated || total_truncated;
                if was_truncated {
                    warnings.push(format!("{relative_path}：正文较长，已截断后提供给 Agent。"));
                }
                let detail = if was_truncated {
                    "已提取正文（已截断）"
                } else {
                    "已提取正文"
                };
                files.push(material_file(
                    relative_path,
                    absolute_path,
                    kind,
                    true,
                    detail.to_string(),
                ));
                if total_truncated || total_text_bytes >= MAX_TOTAL_TEXT_BYTES {
                    reached_total_limit = true;
                    warnings.push(format!(
                        "资料正文已达到 {} KB 上限，后续文件只保留清单。",
                        MAX_TOTAL_TEXT_BYTES / 1024
                    ));
                }
            }
            Extraction::Attachment { kind, detail } => {
                text.push_str(&format!(
                    "\n\n--- 本机附件（Agent 必须按需尝试读取）：{relative_path} ---\n绝对路径：{absolute_path}\n读取状态：{detail}\n"
                ));
                warnings.push(format!("{relative_path}：{detail}"));
                files.push(material_file(
                    relative_path,
                    absolute_path,
                    kind,
                    false,
                    detail,
                ));
            }
        }
    }

    if files.is_empty() {
        warnings.push("所选目录中没有可用于需求分析的文件。".to_string());
    }

    let selected_name = selected
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| selected.to_string_lossy().to_string());
    let source_label = if is_directory {
        format!("文件夹 · {selected_name}（{} 个文件）", files.len())
    } else {
        format!("文件 · {selected_name}")
    };

    Ok(RequirementMaterialBundle {
        root_path: selected.to_string_lossy().to_string(),
        source_label,
        text: text.trim().to_string(),
        files,
        warnings,
    })
}

fn canonicalize_selected_path(path: &str) -> Result<PathBuf, String> {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return Err("请选择要分析的本机文件或文件夹".to_string());
    }
    let selected = PathBuf::from(trimmed);
    if !selected.exists() {
        return Err(format!("所选资料不存在：{}", selected.display()));
    }
    if !selected.is_file() && !selected.is_dir() {
        return Err(format!("仅支持普通文件或文件夹：{}", selected.display()));
    }
    selected
        .canonicalize()
        .map_err(|error| format!("无法读取所选资料 {}：{error}", selected.display()))
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    collect_files_recursive(root, &mut files)?;
    Ok(files)
}

fn collect_files_recursive(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    let mut entries = fs::read_dir(directory)
        .map_err(|error| format!("无法读取目录 {}：{error}", directory.display()))?
        .filter_map(Result::ok)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            if SKIPPED_DIRECTORIES.contains(&name.as_str()) || name.starts_with('.') {
                continue;
            }
            collect_files_recursive(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

enum Extraction {
    Text {
        kind: String,
        text: String,
        truncated: bool,
    },
    Attachment {
        kind: String,
        detail: String,
    },
}

fn extract_material_text(path: &Path, extension: &str) -> Extraction {
    if is_text_file(path, extension) {
        return match read_text_limited(path) {
            Ok((text, truncated)) => Extraction::Text {
                kind: "text".to_string(),
                text,
                truncated,
            },
            Err(detail) => Extraction::Attachment {
                kind: "text".to_string(),
                detail,
            },
        };
    }

    if WORD_EXTENSIONS.contains(&extension) {
        return extract_with_command(
            path,
            "word",
            Command::new("textutil")
                .args(["-convert", "txt", "-stdout"])
                .arg(path),
            "无法通过 macOS textutil 提取 Word/RTF 正文，已保留绝对路径供 Agent 尝试读取",
        );
    }

    if extension == "pdf" {
        if let Some(text) = successful_command_text(Command::new("pdftotext").arg(path).arg("-")) {
            return extracted_command_text("pdf", text);
        }
        if let Some(text) = successful_command_text(
            Command::new("mdls")
                .args(["-raw", "-name", "kMDItemTextContent"])
                .arg(path),
        ) {
            if text.trim() != "(null)" {
                return extracted_command_text("pdf", text);
            }
        }
        return Extraction::Attachment {
            kind: "pdf".to_string(),
            detail: "未能提取 PDF 正文，已保留绝对路径供 Agent 尝试读取".to_string(),
        };
    }

    if IMAGE_EXTENSIONS.contains(&extension) {
        return Extraction::Attachment {
            kind: "image".to_string(),
            detail: "图片未在本地转写，已保留绝对路径供 Agent 视觉读取".to_string(),
        };
    }

    Extraction::Attachment {
        kind: classify_kind(extension),
        detail: "当前不支持直接提取该文件正文，已保留绝对路径供 Agent 尝试读取".to_string(),
    }
}

fn extract_with_command(
    path: &Path,
    kind: &str,
    command: &mut Command,
    failure_detail: &str,
) -> Extraction {
    match successful_command_text(command) {
        Some(text) if !text.trim().is_empty() => extracted_command_text(kind, text),
        _ => Extraction::Attachment {
            kind: kind.to_string(),
            detail: format!("{failure_detail}：{}", path.display()),
        },
    }
}

fn extracted_command_text(kind: &str, text: String) -> Extraction {
    let (text, truncated) = truncate_utf8(&text, MAX_FILE_TEXT_BYTES as usize);
    Extraction::Text {
        kind: kind.to_string(),
        text: text.to_string(),
        truncated,
    }
}

fn successful_command_text(command: &mut Command) -> Option<String> {
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

fn read_text_limited(path: &Path) -> Result<(String, bool), String> {
    let metadata = fs::metadata(path)
        .map_err(|error| format!("无法读取文件元信息，已保留绝对路径供 Agent 尝试读取：{error}"))?;
    let truncated = metadata.len() > MAX_FILE_TEXT_BYTES;
    let mut bytes = Vec::new();
    File::open(path)
        .map_err(|error| format!("无法打开文件，已保留绝对路径供 Agent 尝试读取：{error}"))?
        .take(MAX_FILE_TEXT_BYTES)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("无法读取文件，已保留绝对路径供 Agent 尝试读取：{error}"))?;
    String::from_utf8(bytes)
        .map(|text| (text, truncated))
        .map_err(|_| "文件不是有效 UTF-8 文本，已保留绝对路径供 Agent 尝试读取".to_string())
}

fn truncate_utf8(value: &str, max_bytes: usize) -> (&str, bool) {
    if value.len() <= max_bytes {
        return (value, false);
    }
    let mut boundary = max_bytes.min(value.len());
    while boundary > 0 && !value.is_char_boundary(boundary) {
        boundary -= 1;
    }
    (&value[..boundary], true)
}

fn is_text_file(path: &Path, extension: &str) -> bool {
    if TEXT_EXTENSIONS.contains(&extension) {
        return true;
    }
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    TEXT_FILE_NAMES.contains(&name.as_str()) || name.starts_with(".env") || name.ends_with("rc")
}

fn extension_lowercase(path: &Path) -> String {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default()
}

fn classify_kind(extension: &str) -> String {
    if extension.is_empty() {
        "unknown".to_string()
    } else {
        extension.to_string()
    }
}

fn relative_display(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn material_file(
    relative_path: String,
    absolute_path: String,
    kind: String,
    included: bool,
    detail: String,
) -> RequirementMaterialFile {
    RequirementMaterialFile {
        relative_path,
        absolute_path,
        kind,
        included,
        detail,
    }
}
