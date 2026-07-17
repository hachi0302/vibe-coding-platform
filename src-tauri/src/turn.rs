use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{AppHandle, Emitter};

#[derive(Serialize, Deserialize, Clone)]
pub struct TerminalTurnPayload {
    pub agent: String,
    pub path: String,
    pub state: String,
}

struct SignalState {
    _watcher: RecommendedWatcher,
    path: PathBuf,
    offset: u64,
}

struct SessionTurnWatch {
    _watcher: RecommendedWatcher,
    agent: String,
    path: PathBuf,
    offset: u64,
}

static SIGNAL_STATE: OnceLock<Mutex<Option<SignalState>>> = OnceLock::new();
static SESSION_TURN_WATCHES: OnceLock<Mutex<HashMap<String, SessionTurnWatch>>> = OnceLock::new();

fn signal_state() -> &'static Mutex<Option<SignalState>> {
    SIGNAL_STATE.get_or_init(|| Mutex::new(None))
}

fn session_turn_watches() -> &'static Mutex<HashMap<String, SessionTurnWatch>> {
    SESSION_TURN_WATCHES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn data_dir() -> Result<PathBuf, String> {
    let base = dirs::data_local_dir()
        .or_else(dirs::data_dir)
        .ok_or_else(|| "Cannot locate local data directory".to_string())?;
    Ok(base.join("vibe-coding-platform"))
}

pub fn signal_file_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("turn-signals.jsonl"))
}

fn hook_script_path() -> Result<PathBuf, String> {
    Ok(data_dir()?.join("claude-turn-signal-hook.cjs"))
}

const SESSION_TURN_POLL_MS: u64 = 1500;

pub fn emit_turn_signal(app: &AppHandle, payload: TerminalTurnPayload) -> Result<(), String> {
    if payload.agent != "claude" && payload.agent != "codex" && payload.agent != "agy" {
        return Err("Unknown agent".to_string());
    }
    if payload.path.trim().is_empty() {
        return Err("Missing session path".to_string());
    }
    if !matches!(
        payload.state.as_str(),
        "started" | "completed" | "blocked" | "failed"
    ) {
        return Err("Unknown session state".to_string());
    }
    app.emit("terminal-turn://state", payload)
        .map_err(|e| e.to_string())
}

pub fn start_signal_watcher(app: AppHandle) -> Result<(), String> {
    let signal_path = signal_file_path()?;
    if let Some(parent) = signal_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create state directory: {e}"))?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&signal_path)
        .map_err(|e| format!("Failed to initialize state file: {e}"))?;

    let offset = fs::metadata(&signal_path).map(|m| m.len()).unwrap_or(0);
    let app_for_cb = app.clone();
    let path_for_cb = signal_path.clone();
    let mut watcher: RecommendedWatcher =
        notify::recommended_watcher(move |res: notify::Result<Event>| {
            let Ok(ev) = res else { return };
            if !matches!(ev.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                return;
            }
            process_signal_file(&app_for_cb, &path_for_cb);
        })
        .map_err(|e| format!("Failed to initialize turn signal watcher: {e}"))?;

    watcher
        .watch(&signal_path, RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch state file: {e}"))?;

    let mut slot = signal_state().lock().map_err(|e| e.to_string())?;
    *slot = Some(SignalState {
        _watcher: watcher,
        path: signal_path,
        offset,
    });
    Ok(())
}

pub fn watch_session_turn(
    app: AppHandle,
    agent: String,
    path: String,
    catch_up: bool,
) -> Result<(), String> {
    if agent != "claude" && agent != "codex" && agent != "agy" {
        return Ok(());
    }
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err(format!("File does not exist: {path}"));
    }
    let offset = if catch_up {
        0
    } else {
        let target_fp = if agent == "agy" {
            preferred_transcript(&p)
        } else {
            p.clone()
        };
        fs::metadata(&target_fp).map(|m| m.len()).unwrap_or(0)
    };
    let watch_root = p
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| format!("Cannot determine parent directory: {path}"))?;
    let app_for_cb = app.clone();
    let agent_for_cb = agent.clone();
    let agent_for_catchup = agent.clone();
    let path_for_cb = path.clone();
    let path_buf_for_cb = p.clone();
    let mut watcher: RecommendedWatcher =
        notify::recommended_watcher(move |res: notify::Result<Event>| {
            let Ok(ev) = res else { return };
            if !matches!(ev.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                return;
            }
            process_session_turn_file(&app_for_cb, &agent_for_cb, &path_for_cb, &path_buf_for_cb);
        })
        .map_err(|e| format!("Failed to initialize turn session watcher: {e}"))?;

    watcher
        .watch(&watch_root, RecursiveMode::NonRecursive)
        .map_err(|e| format!("Failed to watch session state: {e}"))?;

    let mut watches = session_turn_watches().lock().map_err(|e| e.to_string())?;
    watches.insert(
        path.clone(),
        SessionTurnWatch {
            _watcher: watcher,
            agent,
            path: p.clone(),
            offset,
        },
    );
    drop(watches);
    if catch_up {
        process_session_turn_file(&app, &agent_for_catchup, &path, &p);
    }
    start_session_turn_poll(app, agent_for_catchup, path, p);
    Ok(())
}

pub fn unwatch_session_turn(path: String) -> Result<(), String> {
    let mut watches = session_turn_watches().lock().map_err(|e| e.to_string())?;
    watches.remove(&path);
    Ok(())
}

pub fn check_session_turns(app: AppHandle) -> Result<(), String> {
    let watches_info = {
        let guard = session_turn_watches().lock().map_err(|e| e.to_string())?;
        guard
            .values()
            .map(|w| (w.agent.clone(), w.path.to_string_lossy().to_string()))
            .collect::<Vec<_>>()
    };
    for (agent, path) in watches_info {
        process_session_turn_file(&app, &agent, &path, Path::new(&path));
    }
    Ok(())
}

fn start_session_turn_poll(app: AppHandle, agent: String, path: String, fp: PathBuf) {
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(SESSION_TURN_POLL_MS));
        let should_continue = {
            let guard = match session_turn_watches().lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            matches!(
                guard.get(&path),
                Some(state) if state.agent == agent && state.path == fp
            )
        };
        if !should_continue {
            return;
        }
        process_session_turn_file(&app, &agent, &path, &fp);
    });
}

fn process_session_turn_file(app: &AppHandle, agent: &str, path: &str, fp: &Path) {
    let target_fp = if agent == "agy" {
        preferred_transcript(fp)
    } else {
        fp.to_path_buf()
    };
    let mut file = match File::open(&target_fp) {
        Ok(f) => f,
        Err(_) => return,
    };
    let file_len = match file.metadata() {
        Ok(m) => m.len(),
        Err(_) => return,
    };
    let offset = {
        let mut guard = match session_turn_watches().lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let Some(state) = guard.get_mut(path) else {
            return;
        };
        if state.agent != agent || state.path != fp {
            return;
        }
        if file_len < state.offset {
            state.offset = 0;
        }
        state.offset
    };

    if file.seek(SeekFrom::Start(offset)).is_err() {
        return;
    }
    let mut buf = String::new();
    if file.read_to_string(&mut buf).is_err() {
        return;
    }
    let consumed = complete_jsonl_prefix_len(&buf);
    if consumed == 0 {
        return;
    }
    if let Ok(mut guard) = session_turn_watches().lock() {
        if let Some(state) = guard.get_mut(path) {
            state.offset = offset.saturating_add(consumed as u64);
        }
    }

    for line in buf[..consumed].lines() {
        let Some(state) = infer_turn_state(agent, line) else {
            continue;
        };
        let _ = emit_turn_signal(
            app,
            TerminalTurnPayload {
                agent: agent.to_string(),
                path: path.to_string(),
                state: state.to_string(),
            },
        );
    }
}

fn complete_jsonl_prefix_len(buf: &str) -> usize {
    let newline_prefix_len = buf.rfind('\n').map(|idx| idx + 1).unwrap_or(0);
    let tail = &buf[newline_prefix_len..];
    if tail.trim().is_empty() {
        return newline_prefix_len;
    }
    if serde_json::from_str::<Value>(tail.trim()).is_ok() {
        buf.len()
    } else {
        newline_prefix_len
    }
}

fn infer_turn_state(agent: &str, line: &str) -> Option<&'static str> {
    let value: Value = serde_json::from_str(line.trim()).ok()?;
    match agent {
        "claude" => infer_claude_turn_state(&value),
        "codex" => infer_codex_turn_state(&value),
        "agy" => crate::agents::agy::classify_turn_state(&value),
        _ => None,
    }
}

fn infer_claude_turn_state(value: &Value) -> Option<&'static str> {
    match value.get("type").and_then(Value::as_str)? {
        "user" => {
            if value
                .get("isMeta")
                .and_then(Value::as_bool)
                .unwrap_or(false)
            {
                None
            } else if claude_user_message_has_content(value) {
                Some("started")
            } else {
                None
            }
        }
        "attachment" => {
            if claude_queued_command_has_content(value) {
                Some("started")
            } else {
                None
            }
        }
        "assistant" => {
            if value.get("message").is_some() {
                Some("completed")
            } else {
                None
            }
        }
        _ => None,
    }
}

fn claude_user_message_has_content(value: &Value) -> bool {
    let Some(content) = value
        .get("message")
        .and_then(|message| message.get("content"))
    else {
        return false;
    };
    match content {
        Value::String(text) => !text.trim().is_empty(),
        Value::Array(items) => {
            items
                .iter()
                .any(|item| match item.get("type").and_then(Value::as_str) {
                    Some("text") => item
                        .get("text")
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.trim().is_empty()),
                    Some("image") => true,
                    _ => false,
                })
        }
        _ => false,
    }
}

fn claude_queued_command_has_content(value: &Value) -> bool {
    let Some(attachment) = value.get("attachment") else {
        return false;
    };
    if attachment.get("type").and_then(Value::as_str) != Some("queued_command") {
        return false;
    }
    match attachment.get("prompt") {
        Some(Value::String(text)) => !text.trim().is_empty(),
        Some(Value::Array(items)) => {
            items
                .iter()
                .any(|item| match item.get("type").and_then(Value::as_str) {
                    Some("text") => item
                        .get("text")
                        .and_then(Value::as_str)
                        .is_some_and(|text| !text.trim().is_empty()),
                    Some("image") => true,
                    _ => false,
                })
        }
        _ => false,
    }
}

fn infer_codex_turn_state(value: &Value) -> Option<&'static str> {
    crate::agents::codex::classify_turn_state(value)
}

fn process_signal_file(app: &AppHandle, path: &Path) {
    let mut file = match File::open(path) {
        Ok(f) => f,
        Err(_) => return,
    };
    let file_len = match file.metadata() {
        Ok(m) => m.len(),
        Err(_) => return,
    };
    let offset = {
        let mut guard = match signal_state().lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        let Some(state) = guard.as_mut() else { return };
        if state.path != path {
            return;
        }
        if file_len < state.offset {
            state.offset = 0;
        }
        state.offset
    };

    if file.seek(SeekFrom::Start(offset)).is_err() {
        return;
    }
    let mut buf = String::new();
    if file.read_to_string(&mut buf).is_err() {
        return;
    }
    let consumed = complete_jsonl_prefix_len(&buf);
    if consumed == 0 {
        return;
    }
    if let Ok(mut guard) = signal_state().lock() {
        if let Some(state) = guard.as_mut() {
            if state.path == path {
                state.offset = offset.saturating_add(consumed as u64);
            }
        }
    }

    for line in buf[..consumed].lines() {
        let Ok(payload) = serde_json::from_str::<TerminalTurnPayload>(line) else {
            continue;
        };
        let _ = emit_turn_signal(app, payload);
    }
}

pub fn install_claude_hooks() -> Result<String, String> {
    let signal_path = signal_file_path()?;
    if let Some(parent) = signal_path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Failed to create state directory: {e}"))?;
    }
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(&signal_path)
        .map_err(|e| format!("Failed to initialize state file: {e}"))?;

    let script_path = hook_script_path()?;
    write_hook_script(&script_path)?;

    let home = dirs::home_dir().ok_or_else(|| "Cannot locate home directory".to_string())?;
    let claude_dir = home.join(".claude");
    fs::create_dir_all(&claude_dir)
        .map_err(|e| format!("Failed to create Claude config directory: {e}"))?;
    let settings_path = claude_dir.join("settings.json");

    let mut settings = read_json_object(&settings_path)?;
    merge_claude_hook(
        &mut settings,
        "UserPromptSubmit",
        "started",
        &script_path,
        &signal_path,
    );
    merge_claude_hook(
        &mut settings,
        "Stop",
        "completed",
        &script_path,
        &signal_path,
    );
    merge_claude_hook(
        &mut settings,
        "StopFailure",
        "failed",
        &script_path,
        &signal_path,
    );
    merge_claude_hook(
        &mut settings,
        "Notification",
        "blocked",
        &script_path,
        &signal_path,
    );
    merge_claude_hook(
        &mut settings,
        "PermissionRequest",
        "blocked",
        &script_path,
        &signal_path,
    );

    let formatted = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    fs::write(&settings_path, format!("{formatted}\n"))
        .map_err(|e| format!("Failed to write Claude config: {e}"))?;

    Ok(settings_path.to_string_lossy().to_string())
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

fn merge_claude_hook(
    settings: &mut Value,
    event: &str,
    state: &str,
    script_path: &Path,
    signal_path: &Path,
) {
    if !settings.get("hooks").is_some_and(Value::is_object) {
        settings["hooks"] = json!({});
    }
    let Some(hooks) = settings.get_mut("hooks").and_then(Value::as_object_mut) else {
        return;
    };
    let entry = hooks.entry(event.to_string()).or_insert_with(|| json!([]));
    if !entry.is_array() {
        *entry = json!([]);
    }
    let Some(groups) = entry.as_array_mut() else {
        return;
    };

    for group in groups.iter_mut() {
        let Some(items) = group.get_mut("hooks").and_then(Value::as_array_mut) else {
            continue;
        };
        items.retain(|item| !is_our_hook(item, script_path));
    }
    groups.retain(|group| {
        group
            .get("hooks")
            .and_then(Value::as_array)
            .is_some_and(|items| !items.is_empty())
    });

    groups.push(json!({
        "hooks": [{
            "type": "command",
            "command": format!(
                "node {} {} {}",
                shell_path_arg(script_path),
                shell_string_arg(state),
                shell_path_arg(signal_path)
            ),
            "timeout": 5
        }]
    }));
}

fn is_our_hook(item: &Value, script_path: &Path) -> bool {
    item.get("command")
        .and_then(Value::as_str)
        .is_some_and(|command| command.contains(script_path.to_string_lossy().as_ref()))
}

fn shell_path_arg(value: impl AsRef<Path>) -> String {
    let raw = value.as_ref().to_string_lossy();
    shell_string_arg(&raw)
}

fn shell_string_arg(raw: &str) -> String {
    format!("\"{}\"", raw.replace('\\', "\\\\").replace('"', "\\\""))
}

fn write_hook_script(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create hook script directory: {e}"))?;
    }
    fs::write(path, HOOK_SCRIPT).map_err(|e| format!("Failed to write hook script: {e}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(path)
            .map_err(|e| format!("Failed to read hook script permissions: {e}"))?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(path, perms)
            .map_err(|e| format!("Failed to set hook script permissions: {e}"))?;
    }
    Ok(())
}

const HOOK_SCRIPT: &str = r#"#!/usr/bin/env node
const fs = require('fs');
const path = require('path');

function hasPromptContent(value) {
  if (typeof value === 'string') return value.trim().length > 0;
  if (Array.isArray(value)) return value.some((item) => {
    if (typeof item === 'string') return item.trim().length > 0;
    if (!item || typeof item !== 'object') return false;
    if (item.type === 'text') return hasPromptContent(item.text || item.content || '');
    if (item.type === 'image') return true;
    return hasPromptContent(item.text || item.content || item.prompt || '');
  });
  if (value && typeof value === 'object') {
    if (value.type === 'image') return true;
    return hasPromptContent(value.text || value.content || value.prompt || '');
  }
  return false;
}

function shouldSkipStarted(data) {
  const candidates = [data.prompt, data.message, data.user_prompt, data.userPrompt];
  return candidates.some((value) => value !== undefined) && !candidates.some(hasPromptContent);
}

const state = process.argv[2];
const signalPath = process.argv[3];
let input = '';
process.stdin.setEncoding('utf8');
process.stdin.on('data', chunk => { input += chunk; });
process.stdin.on('end', () => {
  try {
    if (!signalPath || !state) process.exit(0);
    const data = input.trim() ? JSON.parse(input) : {};
    const transcriptPath = data.transcript_path || data.transcriptPath || '';
    if (!transcriptPath) process.exit(0);
    if (state === 'started' && shouldSkipStarted(data)) process.exit(0);
    const payload = {
      agent: 'claude',
      path: transcriptPath,
      state,
    };
    fs.mkdirSync(path.dirname(signalPath), { recursive: true });
    fs.appendFileSync(signalPath, JSON.stringify(payload) + '\n', 'utf8');
  } catch (_) {
    // Observability hook: never block Claude Code.
  }
});
"#;

fn preferred_transcript(p: &Path) -> PathBuf {
    let full = p.with_file_name("transcript_full.jsonl");
    if full.exists() {
        full
    } else {
        p.to_path_buf()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    use crate::agents::codex::classify_turn_state as codex_classify;

    #[test]
    fn claude_infers_turn_lifecycle_only_from_real_user_input() {
        assert_eq!(
            infer_claude_turn_state(&json!({"type":"user","message":{"content":"hi"}})),
            Some("started")
        );
        assert_eq!(
            infer_claude_turn_state(
                &json!({"type":"user","message":{"content":[{"type":"image","source":{"type":"base64","data":"AAAA"}}]}})
            ),
            Some("started")
        );
        assert_eq!(
            infer_claude_turn_state(&json!({"type":"user","message":{"content":"   "}})),
            None
        );
        assert_eq!(
            infer_claude_turn_state(
                &json!({"type":"user","message":{"content":[{"type":"tool_result","content":"ok"}]}})
            ),
            None
        );
        assert_eq!(
            infer_claude_turn_state(
                &json!({"type":"user","isMeta":true,"message":{"content":"hi"}})
            ),
            None
        );
        assert_eq!(
            infer_claude_turn_state(
                &json!({"type":"attachment","attachment":{"type":"queued_command","prompt":"run tests"}})
            ),
            Some("started")
        );
        assert_eq!(
            infer_claude_turn_state(
                &json!({"type":"attachment","attachment":{"type":"queued_command","prompt":"   "}})
            ),
            None
        );
        assert_eq!(
            infer_claude_turn_state(&json!({"type":"assistant","message":{"content":"done"}})),
            Some("completed")
        );
    }

    #[test]
    fn codex_infers_turn_lifecycle_from_event_messages() {
        assert_eq!(
            codex_classify(
                &json!({"type":"event_msg","payload":{"type":"user_message","message":"hi"}})
            ),
            Some("started")
        );
        assert_eq!(
            codex_classify(
                &json!({"type":"event_msg","payload":{"type":"user_message","message":" ","images":["data:image/png;base64,abc"]}})
            ),
            Some("started")
        );
        assert_eq!(
            codex_classify(&json!({"type":"event_msg","payload":{"type":"user_message"}})),
            None
        );
        assert_eq!(
            codex_classify(
                &json!({"type":"event_msg","payload":{"type":"user_message","message":"   ","images":[],"local_images":[],"text_elements":[]}})
            ),
            None
        );
        assert_eq!(
            codex_classify(&json!({"type":"event_msg","payload":{"type":"task_started"}})),
            None
        );
        assert_eq!(
            codex_classify(
                &json!({"type":"event_msg","payload":{"type":"agent_message","message":"done"}})
            ),
            Some("completed")
        );
        assert_eq!(
            codex_classify(&json!({"type":"event_msg","payload":{"type":"task_complete"}})),
            Some("completed")
        );
        assert_eq!(
            codex_classify(
                &json!({"type":"event_msg","payload":{"type":"agent_message","phase":"final_answer","message":"done"}})
            ),
            Some("completed")
        );
        assert_eq!(
            codex_classify(
                &json!({"type":"event_msg","payload":{"type":"agent_message","phase":"commentary","message":"checking"}})
            ),
            None
        );
        assert_eq!(
            codex_classify(
                &json!({"type":"event_msg","payload":{"type":"error","message":"boom"}})
            ),
            Some("failed")
        );
        assert_eq!(
            codex_classify(
                &json!({"type":"event_msg","payload":{"type":"task_failed","message":"boom"}})
            ),
            Some("failed")
        );
        assert_eq!(
            codex_classify(&json!({"type":"event_msg","payload":{"type":"token_count"}})),
            None
        );
    }

    #[test]
    fn jsonl_consumption_keeps_partial_line_for_next_event() {
        assert_eq!(complete_jsonl_prefix_len(""), 0);
        assert_eq!(complete_jsonl_prefix_len("{\"a\":1}"), 7);
        assert_eq!(complete_jsonl_prefix_len("{\"a\":"), 0);
        assert_eq!(complete_jsonl_prefix_len("{\"a\":1}\n{\"b\":"), 8);
        assert_eq!(complete_jsonl_prefix_len("{\"a\":1}\n{\"b\":2}"), 15);
        assert_eq!(
            complete_jsonl_prefix_len("{\"a\":\"中\"}\n"),
            "{\"a\":\"中\"}\n".len()
        );
    }
}
