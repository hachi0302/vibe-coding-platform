// 从 Bash 工具调用的 input 里抽出"主命令名"，给 By Shell 统计用。
//
// Claude / Codex 在 tool_use 的 input 里写形如：
//   { "command": "git status --short", "description": "show changes" }
// 主命令名 = 命令首词；去掉 `sudo` / `time` / 环境变量赋值前缀；过滤 shell 关键字。

use serde_json::Value;

const PREFIXES_TO_SKIP: &[&str] = &["sudo", "time", "env", "command", "exec", "nohup"];

/// 解析一条 Bash tool_use 的 input 字符串（可能是 JSON 也可能是裸字符串），
/// 返回首词。失败 / 无意义返回 None。
pub fn extract_first_command(raw_input: &str) -> Option<String> {
    let trimmed = raw_input.trim();
    if trimmed.is_empty() {
        return None;
    }
    // 1) 尝试 JSON 形式：{"command":"...","description":"..."}
    let cmd_str = match serde_json::from_str::<Value>(trimmed) {
        Ok(v) => v
            .get("command")
            .and_then(|c| c.as_str())
            .map(|s| s.to_string())
            // 也兼容 OpenAI 风格的 {"input":"..."} / {"cmd":"..."}
            .or_else(|| {
                v.get("input")
                    .and_then(|c| c.as_str())
                    .map(|s| s.to_string())
            })
            .or_else(|| v.get("cmd").and_then(|c| c.as_str()).map(|s| s.to_string()))
            .unwrap_or_else(|| trimmed.to_string()),
        Err(_) => trimmed.to_string(),
    };
    first_token_of(&cmd_str)
}

/// 命令字符串里取第一个有意义的 token。逻辑：
///   1. 去掉前面 `KEY=VAL` 形式的环境变量赋值；
///   2. 跳过 sudo / time / env 等 wrapper 前缀；
///   3. 取剩余的第一个 token，并把 `/path/to/foo` 截到 basename。
fn first_token_of(cmd: &str) -> Option<String> {
    let trimmed = cmd.trim_start_matches(|c: char| c.is_whitespace());
    if trimmed.is_empty() {
        return None;
    }
    // 用 shell 风格的简单 splitting —— 第一个空白前的子串是 token；
    // 复杂引号 / 管道的语义不做（统计用，noise 可以接受）。
    let mut rest = trimmed;
    loop {
        let token = rest.split_whitespace().next().map(|s| s.to_string())?;
        // 环境变量赋值（FOO=BAR cmd）：跳过这个 token，继续看下一个。
        if token.contains('=') && !token.starts_with('=') && !is_known_command(&token) {
            let after = rest[token.len()..].trim_start();
            if after.is_empty() {
                return None;
            }
            rest = after;
            continue;
        }
        // wrapper 前缀（sudo / time / env / nohup）：跳过它。
        if PREFIXES_TO_SKIP.contains(&token.as_str()) {
            let after = rest[token.len()..].trim_start();
            if after.is_empty() {
                return None;
            }
            rest = after;
            continue;
        }
        // 拿到主命令：把 `/usr/local/bin/git` → `git`。
        let basename = token
            .rsplit('/')
            .next()
            .unwrap_or(&token)
            .trim_matches(|c: char| !c.is_alphanumeric() && c != '_' && c != '-' && c != '.')
            .to_string();
        if basename.is_empty() {
            return None;
        }
        return Some(basename);
    }
}

/// 用来辨别 `=`-token 是不是误判（比如有些 alias 命令字面里就含 `=`）。
/// 现在只有一种例外：完全等于 `==` 这种字符串。一般用不到。
fn is_known_command(token: &str) -> bool {
    matches!(token, "==" | "!=")
}

/// 从工具名里抽 MCP server。`mcp__github__list_repos` → `github`。
/// 非 mcp__ 开头返回 None；分段不足也 None。
pub fn extract_mcp_server(tool_name: &str) -> Option<String> {
    let rest = tool_name.strip_prefix("mcp__")?;
    let server = rest.split("__").next()?;
    if server.is_empty() {
        None
    } else {
        Some(server.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_from_json_command_string() {
        let s = r#"{"command":"git status --short","description":"changes"}"#;
        assert_eq!(extract_first_command(s), Some("git".to_string()));
    }

    #[test]
    fn extract_strips_sudo_wrapper() {
        let s = r#"{"command":"sudo apt install foo"}"#;
        assert_eq!(extract_first_command(s), Some("apt".to_string()));
    }

    #[test]
    fn extract_strips_env_prefix() {
        let s = r#"{"command":"FOO=bar BAZ=qux pnpm run dev"}"#;
        assert_eq!(extract_first_command(s), Some("pnpm".to_string()));
    }

    #[test]
    fn extract_strips_absolute_path_to_basename() {
        let s = r#"{"command":"/usr/local/bin/rg --threads 2 needle"}"#;
        assert_eq!(extract_first_command(s), Some("rg".to_string()));
    }

    #[test]
    fn extract_from_bare_string_not_json() {
        let s = "git diff --staged";
        assert_eq!(extract_first_command(s), Some("git".to_string()));
    }

    #[test]
    fn extract_handles_empty_and_garbage() {
        assert_eq!(extract_first_command(""), None);
        assert_eq!(extract_first_command("   "), None);
    }

    #[test]
    fn extract_handles_oai_cmd_field_alias() {
        let s = r#"{"cmd":"npm test"}"#;
        assert_eq!(extract_first_command(s), Some("npm".to_string()));
    }

    #[test]
    fn mcp_server_basic() {
        assert_eq!(
            extract_mcp_server("mcp__github__list_repos"),
            Some("github".to_string())
        );
        assert_eq!(
            extract_mcp_server("mcp__chrome-devtools__click"),
            Some("chrome-devtools".to_string())
        );
    }

    #[test]
    fn mcp_server_non_mcp_returns_none() {
        assert_eq!(extract_mcp_server("Bash"), None);
        assert_eq!(extract_mcp_server("mcp__"), None);
    }
}
