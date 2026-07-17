// 直接调用 lib 函数 verify 真实 Claude JSONL 的 dedup 效果
use claude_session_viewer_lib::agents;
use claude_session_viewer_lib::stats::aggregate::{Aggregator, SessionFeed};

fn main() {
    let path = std::env::args().nth(1).expect("path arg");
    let src = agents::source("claude").unwrap();
    let turns = src.read_turns(&path).unwrap();
    let total_calls_in_turns: usize = turns.iter().map(|t| t.calls.len()).sum();
    let with_msgid: usize = turns
        .iter()
        .flat_map(|t| &t.calls)
        .filter(|c| c.message_id.is_some())
        .count();
    let unique_msgid: std::collections::HashSet<&String> = turns
        .iter()
        .flat_map(|t| &t.calls)
        .filter_map(|c| c.message_id.as_ref())
        .collect();
    println!("turns: {}", turns.len());
    println!("total calls in turns (pre-dedup): {}", total_calls_in_turns);
    println!("calls with message_id: {}", with_msgid);
    println!("unique message_ids: {}", unique_msgid.len());
    let mut agg = Aggregator::new();
    agg.feed_session(&SessionFeed {
        agent: "claude",
        project_dir_name: "p",
        project_display: "/p",
        session_id: "s",
        path: &path,
        title: "t",
        last_modified: 1,
        message_count: 0,
        turns: &turns,
    });
    let snap = agg.snapshot("test");
    println!("\nafter aggregator (with dedup):");
    println!("  call_count: {}", snap.call_count);
    println!("  cost_usd: ${:.2}", snap.cost_usd);
    println!("  input_tokens: {}", snap.usage.input_tokens);
}
