//! 账号额度（5 小时 / 周 / 各模型分项）—— 走 Claude 官方未公开的 OAuth 用量接口
//! `GET https://api.anthropic.com/api/oauth/usage`。这是 claude-hud / claude-code-statusline
//! 等社区状态栏拿到「精确百分比 + 重置时间」的同一个数据源：headless `--print` 流里
//! 只有越过 ~75% 阈值才发的 `rate_limit_event`，而这个接口把每个窗口的 utilization 与
//! resets_at 全量返回、不受阈值限制，所以才能「随时精确显示」。
//!
//! OAuth token 从 macOS 钥匙串条目 "Claude Code-credentials" 里取（Claude Code 在活跃
//! 会话期间自动维护其新鲜度）。接口对频繁调用很敏感（社区报告会持续 429），故这里加了
//! 一个进程内短 TTL 缓存兜住前端轮询。

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

const USAGE_URL: &str = "https://api.anthropic.com/api/oauth/usage";
/// 进程内缓存有效期：前端按 ~60s 轮询，这里 20s 兜住偶发的密集调用，避免触发 429。
const CACHE_TTL: Duration = Duration::from_secs(20);

/// 单个额度窗口：利用率百分比（0–100）+ ISO8601 重置时间（前端用 `new Date()` 解析）。
#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct UsageWindow {
    pub utilization: f64,
    pub resets_at: Option<String>,
}

/// 账号额度快照。`five_hour` = 会话窗口，`seven_day` = 周总额，另带 Opus / Sonnet 周分项。
#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AccountUsage {
    pub five_hour: Option<UsageWindow>,
    pub seven_day: Option<UsageWindow>,
    pub seven_day_opus: Option<UsageWindow>,
    pub seven_day_sonnet: Option<UsageWindow>,
}

// ---- 接口响应解析（只取需要的窗口，未知字段忽略）----

#[derive(Deserialize)]
struct ApiWindow {
    utilization: Option<f64>,
    resets_at: Option<String>,
}

#[derive(Deserialize)]
struct ApiUsage {
    five_hour: Option<ApiWindow>,
    seven_day: Option<ApiWindow>,
    seven_day_opus: Option<ApiWindow>,
    seven_day_sonnet: Option<ApiWindow>,
}

impl From<ApiWindow> for UsageWindow {
    fn from(w: ApiWindow) -> Self {
        UsageWindow {
            utilization: w.utilization.unwrap_or(0.0),
            resets_at: w.resets_at,
        }
    }
}

impl From<ApiUsage> for AccountUsage {
    fn from(u: ApiUsage) -> Self {
        AccountUsage {
            five_hour: u.five_hour.map(Into::into),
            seven_day: u.seven_day.map(Into::into),
            seven_day_opus: u.seven_day_opus.map(Into::into),
            seven_day_sonnet: u.seven_day_sonnet.map(Into::into),
        }
    }
}

/// 从 macOS 钥匙串读取 Claude Code 的 OAuth access token。
fn oauth_access_token() -> Result<String, String> {
    let out = std::process::Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output()
        .map_err(|e| format!("read keychain: {e}"))?;
    if !out.status.success() {
        return Err("钥匙串里没有 Claude Code 凭证（未用订阅账号登录 Claude Code？）".into());
    }
    let raw = String::from_utf8_lossy(&out.stdout);
    let v: serde_json::Value =
        serde_json::from_str(raw.trim()).map_err(|e| format!("parse credentials: {e}"))?;
    v.get("claudeAiOauth")
        .and_then(|o| o.get("accessToken"))
        .and_then(|t| t.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "凭证里缺少 claudeAiOauth.accessToken".into())
}

/// 给 curl 配置补一行 `proxy = "..."`（缺省返回空串，curl 行为不变）。
///
/// `tauri dev` 从终端继承了 `HTTPS_PROXY`，curl 自己会读，这里返回空、路径不变。
/// 打包后的 .app 由 Finder/launchd 启动，环境里没有任何 `*_proxy`，curl 只能直连 ——
/// 而本接口对直连 IP 会按地区/风控直接 403。故进程环境无代理时，回落到 macOS「系统代理」
/// （scutil --proxy），把它显式喂给 curl，让打包版也能走代理拿到 200。
fn proxy_config_line() -> String {
    // curl 原生就读环境里的 *_proxy；进程环境已有就别重复注入，保持 dev 路径完全不变。
    for k in ["https_proxy", "HTTPS_PROXY", "ALL_PROXY", "all_proxy"] {
        if std::env::var(k).is_ok_and(|v| !v.trim().is_empty()) {
            return String::new();
        }
    }
    match system_https_proxy() {
        Some(p) => format!("proxy = \"{p}\"\n"),
        None => String::new(),
    }
}

/// 从 macOS 系统网络设置读「HTTPS 代理」（scutil --proxy 输出的 HTTPSEnable/HTTPSProxy/HTTPSPort）。
/// 未启用 / 解析不出主机端口 → None。非 macOS 一律 None（curl 仍只认环境变量，行为不变）。
#[cfg(target_os = "macos")]
fn system_https_proxy() -> Option<String> {
    let out = std::process::Command::new("scutil")
        .arg("--proxy")
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let (mut enabled, mut host, mut port) = (false, None, None);
    for line in text.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("HTTPSEnable :") {
            enabled = v.trim() == "1";
        } else if let Some(v) = line.strip_prefix("HTTPSProxy :") {
            host = Some(v.trim().to_string());
        } else if let Some(v) = line.strip_prefix("HTTPSPort :") {
            port = Some(v.trim().to_string());
        }
    }
    if !enabled {
        return None;
    }
    let host = host.filter(|h| !h.is_empty())?;
    let port = port.filter(|p| !p.is_empty())?;
    Some(format!("http://{host}:{port}"))
}

#[cfg(not(target_os = "macos"))]
fn system_https_proxy() -> Option<String> {
    None
}

/// 真正打接口：带 OAuth header GET 一次 → 解析 → 转成 AccountUsage。
///
/// 用系统 `curl`（而非 ureq）发请求：这个接口挡在 Cloudflare 机器人识别后面，会按 TLS
/// 指纹放行/拦截 —— 系统 TLS 客户端（curl / python / Go）放行，ureq 的 rustls 指纹会被
/// 直接 403。社区状态栏（claude-hud / claudeline）也都走 curl/系统栈。app 本就 shell 调
/// `security`，再调 curl 一致。token 经 stdin 配置传入，不进 argv（避免 `ps` 泄露）。
fn fetch_blocking() -> Result<AccountUsage, String> {
    let token = oauth_access_token()?;
    // curl 配置走 stdin（--config -）：silent + 出错可见 + 15s 超时 + 末尾追加 HTTP 状态码。
    // 代理：见 proxy_config_line —— 打包后的 .app 拿不到 shell 的 HTTPS_PROXY，得显式补上系统代理，
    // 否则这个接口对「直连 IP」会按地区/风控返回 403（系统 curl 也照样 403），徽标在打包版整块消失。
    let config = format!(
        "silent\nshow-error\nmax-time = 15\n\
         {proxy}\
         url = \"{USAGE_URL}\"\n\
         header = \"Authorization: Bearer {token}\"\n\
         header = \"anthropic-beta: oauth-2025-04-20\"\n\
         header = \"anthropic-version: 2023-06-01\"\n\
         header = \"Accept: application/json\"\n\
         header = \"User-Agent: claude-cli/2.1.0 (external)\"\n\
         write-out = \"\\n%{{http_code}}\"\n",
        proxy = proxy_config_line(),
    );
    let mut cmd = Command::new("curl");
    cmd.arg("--config")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    let mut child = cmd.spawn().map_err(|e| format!("spawn curl: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("curl stdin unavailable")?
        .write_all(config.as_bytes())
        .map_err(|e| format!("write curl config: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("curl wait: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "curl: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    // 末行是 write-out 追加的 HTTP 状态码，前面是响应体。
    let (json, code) = stdout.rsplit_once('\n').ok_or("empty curl output")?;
    let code = code.trim();
    if code != "200" {
        return Err(format!("usage api status {code}"));
    }
    let parsed: ApiUsage =
        serde_json::from_str(json.trim()).map_err(|e| format!("parse usage: {e}"))?;
    Ok(parsed.into())
}

fn cache() -> &'static Mutex<Option<(Instant, AccountUsage)>> {
    static CACHE: OnceLock<Mutex<Option<(Instant, AccountUsage)>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

/// 取额度快照：命中 20s TTL 缓存直接返回，否则打接口并刷新缓存。
/// `force=true` 跳过缓存读取（强制拉新），但仍会回写缓存供后续慢轮询复用。
/// 拉新失败（多为 429）时回退到「上一次成功值」（即便已过 TTL）—— 缓存里的快照只在成功时被覆盖，
/// 永不因失败清空，所以徽标宁可陈旧也不整块消失（前端再叠一层 localStorage 即时回种）。
pub fn account_usage_blocking(force: bool) -> Result<AccountUsage, String> {
    if !force {
        if let Ok(guard) = cache().lock() {
            if let Some((at, ref usage)) = *guard {
                if at.elapsed() < CACHE_TTL {
                    return Ok(usage.clone());
                }
            }
        }
    }
    match fetch_blocking() {
        Ok(usage) => {
            if let Ok(mut guard) = cache().lock() {
                *guard = Some((Instant::now(), usage.clone()));
            }
            Ok(usage)
        }
        Err(e) => {
            // 失败回退：有旧值就返回旧值（陈旧好过空白），否则才把错误抛给前端。
            if let Ok(guard) = cache().lock() {
                if let Some((_, ref usage)) = *guard {
                    return Ok(usage.clone());
                }
            }
            Err(e)
        }
    }
}
