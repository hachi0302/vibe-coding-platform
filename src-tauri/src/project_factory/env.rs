use std::path::PathBuf;
use std::process::Command;

use super::types::EnvCheckItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub program: &'static str,
    pub args: &'static [&'static str],
    pub minimum_version: Option<(u32, u32)>,
}

const TOOLS: &[ToolDefinition] = &[
    ToolDefinition {
        id: "node",
        label: "Node.js",
        program: "node",
        args: &["--version"],
        minimum_version: Some((20, 0)),
    },
    ToolDefinition {
        id: "pnpm",
        label: "pnpm",
        program: "pnpm",
        args: &["--version"],
        minimum_version: None,
    },
    ToolDefinition {
        id: "jdk",
        label: "JDK 17+",
        program: "java",
        args: &["-version"],
        minimum_version: Some((17, 0)),
    },
    ToolDefinition {
        id: "maven",
        label: "Maven",
        program: "mvn",
        args: &["--version"],
        minimum_version: None,
    },
    ToolDefinition {
        id: "mysql",
        label: "MySQL",
        program: "mysql",
        args: &["--version"],
        minimum_version: None,
    },
    ToolDefinition {
        id: "redis",
        label: "Redis",
        program: "redis-server",
        args: &["--version"],
        minimum_version: None,
    },
    ToolDefinition {
        id: "rust",
        label: "Rust",
        program: "rustc",
        args: &["--version"],
        minimum_version: Some((1, 75)),
    },
    ToolDefinition {
        id: "tauri",
        label: "Tauri CLI",
        program: "tauri",
        args: &["--version"],
        minimum_version: None,
    },
    ToolDefinition {
        id: "python",
        label: "Python 3.9+",
        program: "python3",
        args: &["--version"],
        minimum_version: Some((3, 9)),
    },
    ToolDefinition {
        id: "go",
        label: "Go",
        program: "go",
        args: &["version"],
        minimum_version: Some((1, 22)),
    },
    ToolDefinition {
        id: "dotnet",
        label: ".NET SDK",
        program: "dotnet",
        args: &["--version"],
        minimum_version: Some((8, 0)),
    },
];

fn numeric_version(text: &str) -> Option<(u32, u32)> {
    let digits: String = text
        .chars()
        .map(|character| {
            if character.is_ascii_digit() || character == '.' {
                character
            } else {
                ' '
            }
        })
        .collect();
    digits.split_whitespace().find_map(|candidate| {
        let mut parts = candidate.split('.');
        let major = parts.next()?.parse().ok()?;
        let minor = parts.next().unwrap_or("0").parse().ok()?;
        Some((major, minor))
    })
}

pub fn tool_definition(tool_id: &str) -> Option<&'static ToolDefinition> {
    TOOLS.iter().find(|tool| tool.id == tool_id)
}

pub fn program_path(tool: &ToolDefinition) -> PathBuf {
    if tool.id == "dotnet" {
        let local_dotnet = dirs::home_dir().map(|home| home.join(".dotnet/dotnet"));
        if local_dotnet.as_ref().is_some_and(|path| path.is_file()) {
            return local_dotnet.expect("checked above");
        }
    }
    PathBuf::from(tool.program)
}

pub fn check_environment(tool_ids: &[String]) -> Result<Vec<EnvCheckItem>, String> {
    tool_ids
        .iter()
        .map(|tool_id| {
            let tool =
                tool_definition(tool_id).ok_or_else(|| format!("不支持的环境工具：{tool_id}"))?;
            let result = Command::new(program_path(tool)).args(tool.args).output();
            let (installed, compatible, version, detail) = match result {
                Ok(output) if output.status.success() => {
                    let text = String::from_utf8_lossy(&output.stdout);
                    let fallback = String::from_utf8_lossy(&output.stderr);
                    let value = text
                        .lines()
                        .chain(fallback.lines())
                        .next()
                        .unwrap_or("已安装")
                        .trim();
                    let compatible = tool
                        .minimum_version
                        .map(|minimum| {
                            numeric_version(value)
                                .map(|version| version >= minimum)
                                .unwrap_or(false)
                        })
                        .unwrap_or(true);
                    let detail = if compatible {
                        None
                    } else {
                        let minimum = tool
                            .minimum_version
                            .map(|(major, minor)| format!("{major}.{minor}"))
                            .unwrap_or_default();
                        Some(format!("已安装，但当前项目骨架至少需要 {minimum}"))
                    };
                    (true, compatible, Some(value.to_string()), detail)
                }
                Ok(output) => {
                    let text = String::from_utf8_lossy(&output.stderr);
                    let detail = text
                        .lines()
                        .next()
                        .unwrap_or("命令执行失败")
                        .trim()
                        .to_string();
                    (false, false, None, Some(detail))
                }
                Err(_) => (false, false, None, Some("未安装或未加入 PATH".to_string())),
            };
            Ok(EnvCheckItem {
                tool_id: tool.id.to_string(),
                label: tool.label.to_string(),
                required: true,
                installed,
                compatible,
                version,
                detail,
            })
        })
        .collect()
}
