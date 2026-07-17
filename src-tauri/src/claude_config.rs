use std::fs;
use std::path::Path;

use serde_json::{json, Value};

use crate::types::{ClaudeAliasTargets, ClaudeRuntimeInfo};
use crate::util::home;

fn settings_path() -> std::path::PathBuf {
    home().join(".claude").join("settings.json")
}

fn read_json_object(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let raw = fs::read_to_string(path).map_err(|e| format!("Failed to read Claude config: {e}"))?;
    if raw.trim().is_empty() {
        return Ok(json!({}));
    }
    let parsed: Value = serde_json::from_str(&raw)
        .map_err(|e| format!("Claude settings.json is not valid JSON: {e}"))?;
    if parsed.is_object() {
        Ok(parsed)
    } else {
        Err("Claude settings.json top level must be an object".to_string())
    }
}

pub fn runtime_info() -> Result<ClaudeRuntimeInfo, String> {
    let settings = read_json_object(&settings_path())?;
    let env = settings.get("env").and_then(|v| v.as_object());
    let base_url = env
        .and_then(|env| env.get("ANTHROPIC_BASE_URL"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .unwrap_or("");

    Ok(ClaudeRuntimeInfo {
        has_custom_base_url: !base_url.is_empty(),
        alias_targets: ClaudeAliasTargets {
            opus: alias_target(env, "OPUS"),
            sonnet: alias_target(env, "SONNET"),
            haiku: alias_target(env, "HAIKU"),
            fable: alias_target(env, "FABLE"),
        },
        api_key_source: guess_api_key_source(&settings, env),
        effort_level: settings
            .get("effortLevel")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    })
}

/// 在 Claude CLI 发出 init 事件、给出权威 `apiKeySource` 之前，先尽力预判鉴权方式，
/// 让官方订阅用户一进新会话就能看到 effort + 5h/周限额（而不是等首轮 init 才显形）。
///
/// 优先级对齐 CLI：显式 API key（env / helper）优先于 OAuth。判不出时返回 None，
/// 让前端保持「未知即不显示」的保守态，等 init 校正。
fn guess_api_key_source(
    settings: &Value,
    env: Option<&serde_json::Map<String, Value>>,
) -> Option<String> {
    // 1) settings.env 或进程环境里有非空 ANTHROPIC_API_KEY → API key 计费。
    //    （GUI 启动常拿不到 shell 里的 env，故 settings.json 是更可靠的来源。）
    let settings_key = env
        .and_then(|env| env.get("ANTHROPIC_API_KEY"))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty());
    let process_key = std::env::var("ANTHROPIC_API_KEY")
        .ok()
        .filter(|s| !s.trim().is_empty());
    if settings_key.is_some() || process_key.is_some() {
        return Some("ANTHROPIC_API_KEY".to_string());
    }
    // 2) settings.apiKeyHelper（外部脚本吐 key）→ 同样是 API key 计费。
    let has_helper = settings
        .get("apiKeyHelper")
        .and_then(|v| v.as_str())
        .map(str::trim)
        .is_some_and(|s| !s.is_empty());
    if has_helper {
        return Some("apiKeyHelper".to_string());
    }
    // 3) 钥匙串里有订阅凭证 → 官方 OAuth 登录（init 会回 "none"）。只看条目存在与否，
    //    不解析 token（避免无谓暴露），与 usage_api 读的是同一条目。
    if has_oauth_credentials() {
        return Some("none".to_string());
    }
    // 4) 都判不出 → 交给 init。
    None
}

/// 默认走 macOS 钥匙串条目 "Claude Code-credentials"；关了钥匙串的设置则回落到
/// `~/.claude/.credentials.json`（Claude Code 在非 keychain 模式 / 非 macOS 上写这里）。
#[cfg(target_os = "macos")]
fn has_oauth_credentials() -> bool {
    let in_keychain = std::process::Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    in_keychain || home().join(".claude").join(".credentials.json").is_file()
}

#[cfg(not(target_os = "macos"))]
fn has_oauth_credentials() -> bool {
    home().join(".claude").join(".credentials.json").is_file()
}

fn alias_target(env: Option<&serde_json::Map<String, Value>>, family: &str) -> Option<String> {
    let name_key = format!("ANTHROPIC_DEFAULT_{family}_MODEL_NAME");
    let model_key = format!("ANTHROPIC_DEFAULT_{family}_MODEL");
    let name = env
        .and_then(|env| env.get(&name_key))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    if name.is_some() {
        return name;
    }
    env.and_then(|env| env.get(&model_key))
        .and_then(|v| v.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.replace("[1M]", "").replace("[1m]", ""))
}
