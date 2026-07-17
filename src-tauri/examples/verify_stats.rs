// 跑全量 stats，模拟前端 start_agent_stats 路径，但不走 Tauri：
//   cargo run --example verify_stats --release -- <agent> <range>
// agent: claude | codex | all
// range: today | days7 | days30 | month | months6 | all (alias of months6 + warn)
//
// 输出一行 JSON：{ cost, calls, sessions, projects, in, out, cr, cw, models, projects_list }
// 用于和 codeburn export 的对账。

use chrono::{Datelike, Duration as CDuration, Local, Months, TimeZone};
use claude_session_viewer_lib::agents;
use claude_session_viewer_lib::stats::aggregate::{Aggregator, SessionFeed};
use claude_session_viewer_lib::stats::pricing;

fn parse_range(range: &str) -> (Option<u64>, Option<u64>) {
    let now = Local::now();
    let today_midnight = Local
        .with_ymd_and_hms(now.year(), now.month(), now.day(), 0, 0, 0)
        .single()
        .unwrap();
    let to_ms = |dt: chrono::DateTime<Local>| dt.timestamp_millis() as u64;
    match range {
        "all" => (None, None), // CLI 兜底：核对 codeburn 时仍可强行无下限
        "today" => (Some(to_ms(today_midnight)), None),
        "days7" => (Some(to_ms(today_midnight - CDuration::days(6))), None),
        "days30" => (Some(to_ms(today_midnight - CDuration::days(29))), None),
        "month" => {
            let m = Local
                .with_ymd_and_hms(now.year(), now.month(), 1, 0, 0, 0)
                .single()
                .unwrap();
            (Some(to_ms(m)), None)
        }
        "months6" => (
            Some(to_ms(
                today_midnight.checked_sub_months(Months::new(6)).unwrap(),
            )),
            None,
        ),
        _ => panic!("unknown range {range}"),
    }
}

fn in_window(mtime: u64, lo: Option<u64>, hi: Option<u64>) -> bool {
    if let Some(l) = lo {
        if mtime < l {
            return false;
        }
    }
    if let Some(h) = hi {
        if mtime > h {
            return false;
        }
    }
    true
}

fn main() {
    let agent_arg = std::env::args().nth(1).unwrap_or_else(|| "all".to_string());
    let range = std::env::args()
        .nth(2)
        .unwrap_or_else(|| "today".to_string());
    let agents_to_scan: Vec<&'static str> = match agent_arg.as_str() {
        "all" => vec!["claude", "codex"],
        "claude" => vec!["claude"],
        "codex" => vec!["codex"],
        _ => panic!("unknown agent {agent_arg}"),
    };
    let (lo, hi) = parse_range(&range);

    // pricing 表是 lazy 从 LiteLLM 拉的；CLI 模式下没人触发 init，cost 会算成
    // 0。先用 init() 从磁盘缓存灌入（与 Tauri setup hook 同一路径），失败再
    // 同步 fetch 一次兜底。
    pricing::init();
    // init 把后台线程派出去了，对 CLI 来说我们等不到 —— 但只要缓存存在就够；
    // 如果完全没有缓存，强行 blocking 拉一次。
    if pricing::status().model_count == 0 {
        if let Err(e) = pricing::refresh_blocking() {
            eprintln!("warn: pricing fetch failed ({e}) — costs will be $0.00");
        }
    }

    let mut agg = Aggregator::new_with_range(lo, hi);

    for agent_name in &agents_to_scan {
        let src = match agents::source(agent_name) {
            Ok(s) => s,
            Err(_) => continue,
        };
        let projects = match src.list_projects(false, true) {
            Ok(p) => p,
            Err(_) => continue,
        };
        for p in projects {
            let sessions = match src.discover_stats_sessions(&p.dir_name) {
                Ok(s) => s,
                Err(_) => continue,
            };
            for s in sessions {
                if !in_window(s.modified, lo, hi) {
                    continue;
                }
                let turns = src.read_turns(&s.path).unwrap_or_default();
                agg.feed_session(&SessionFeed {
                    agent: agent_name,
                    project_dir_name: &p.dir_name,
                    project_display: &p.display_path,
                    session_id: &s.id,
                    path: &s.path,
                    title: &s.title,
                    last_modified: s.modified,
                    message_count: s.message_count,
                    turns: &turns,
                });
            }
        }
    }

    let snap = agg.snapshot(&agent_arg);
    println!(
        "{:30} cost=${:>10.2} calls={:>5} sessions={:>4} projects={:>3} in={:>7} out={:>9} cr={:>12} cw={:>11}",
        format!("ours {}/{}", agent_arg, range),
        snap.cost_usd,
        snap.call_count,
        snap.session_count,
        snap.projects.len(),
        snap.usage.input_tokens,
        snap.usage.output_tokens,
        snap.usage.cache_read_input_tokens,
        snap.usage.cache_creation_input_tokens,
    );
}
