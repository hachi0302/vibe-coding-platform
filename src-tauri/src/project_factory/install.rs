use std::process::Command;

use tauri::{AppHandle, Emitter};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallCommand {
    pub program: String,
    pub args: Vec<String>,
}

fn command(program: &str, args: &[&str]) -> InstallCommand {
    InstallCommand {
        program: program.to_string(),
        args: args.iter().map(|arg| (*arg).to_string()).collect(),
    }
}

pub fn install_command_for(tool_id: &str, os: &str) -> Result<InstallCommand, String> {
    let command = match os {
        "macos" => match tool_id {
            "node" => command("brew", &["install", "node"]),
            "pnpm" => command("brew", &["install", "pnpm"]),
            "jdk" => command("brew", &["install", "openjdk@21"]),
            "maven" => command("brew", &["install", "maven"]),
            "mysql" => command("brew", &["install", "mysql"]),
            "redis" => command("brew", &["install", "redis"]),
            "rust" => command("brew", &["install", "rust"]),
            "tauri" => command("cargo", &["install", "tauri-cli", "--version", "^2"]),
            "python" => command("brew", &["install", "python"]),
            "go" => command("brew", &["install", "go"]),
            "dotnet" => command("brew", &["install", "dotnet"]),
            _ => return Err(format!("不支持安装工具：{tool_id}")),
        },
        "windows" => match tool_id {
            "node" => command(
                "winget",
                &["install", "--id", "OpenJS.NodeJS.LTS", "--exact"],
            ),
            "pnpm" => command("winget", &["install", "--id", "pnpm.pnpm", "--exact"]),
            "jdk" => command(
                "winget",
                &["install", "--id", "Microsoft.OpenJDK.21", "--exact"],
            ),
            "maven" => command("winget", &["install", "--id", "Apache.Maven", "--exact"]),
            "mysql" => command("winget", &["install", "--id", "Oracle.MySQL", "--exact"]),
            "redis" => command(
                "winget",
                &["install", "--id", "Memurai.MemuraiDeveloper", "--exact"],
            ),
            "rust" => command("winget", &["install", "--id", "Rustlang.Rustup", "--exact"]),
            "tauri" => command("npm", &["install", "--global", "@tauri-apps/cli"]),
            "python" => command(
                "winget",
                &["install", "--id", "Python.Python.3.12", "--exact"],
            ),
            "go" => command("winget", &["install", "--id", "GoLang.Go", "--exact"]),
            "dotnet" => command(
                "winget",
                &["install", "--id", "Microsoft.DotNet.SDK.8", "--exact"],
            ),
            _ => return Err(format!("不支持安装工具：{tool_id}")),
        },
        _ => return Err("当前系统暂不支持一键安装，仅支持 macOS 和 Windows".to_string()),
    };
    Ok(command)
}

pub fn install_tool(app: &AppHandle, tool_id: &str) -> Result<(), String> {
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "windows") {
        "windows"
    } else {
        "unsupported"
    };
    let command = install_command_for(tool_id, os)?;
    app.emit("env-install://log", format!("开始安装 {tool_id}"))
        .map_err(|error| error.to_string())?;
    let output = Command::new(&command.program)
        .args(&command.args)
        .output()
        .map_err(|error| format!("无法启动安装命令：{error}"))?;
    if output.status.success() {
        app.emit("env-install://done", tool_id)
            .map_err(|error| error.to_string())?;
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let message = if detail.is_empty() {
            "安装命令执行失败".to_string()
        } else {
            detail
        };
        let _ = app.emit("env-install://error", &message);
        Err(message)
    }
}
