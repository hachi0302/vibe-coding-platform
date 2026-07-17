//! 调查工具（不参与构建产物，只在需要时手动跑）：在真实 PTY 里拉起 codex，抓它实际
//! 吐出的 SGR 序列。`src/terminals.ts` 里那套「镜像亮度」的前景归一化就是照这份输出
//! 定的 —— codex 的调色板变了、或者要复核那些魔数时，重新跑它。
//!
//! 两个结论由它证明（别凭直觉推翻）：
//!   1. codex-cli 0.144.4 在 Windows 上**不发** `ESC]11;?` 查背景色（`--answer-osc11`
//!      就是用来验证这点的），也不认 COLORFGBG，所以只能在字节流上改颜色。
//!   2. codex 只画一种背景（深色 #292929），从不用「浅底 + 深字」，所以镜像前景是安全的。
//!
//! 用法：
//!   cargo run --example codex_color_probe -- <codex 可执行文件> <cwd> [--answer-osc11 <rgb:ffff/ffff/ffff>]

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::collections::BTreeSet;
use std::io::Read;
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let exe = args.get(1).cloned().unwrap_or_else(|| "codex".into());
    let cwd = args.get(2).cloned().unwrap_or_else(|| ".".into());
    let answer = args.iter().position(|a| a == "--answer-osc11").map(|i| {
        args.get(i + 1).cloned().unwrap_or_else(|| "rgb:ffff/ffff/ffff".into())
    });

    let pair = native_pty_system()
        .openpty(PtySize { rows: 30, cols: 100, pixel_width: 0, pixel_height: 0 })
        .expect("openpty");

    let mut cmd = CommandBuilder::new(&exe);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("COLORFGBG", "0;15"); // 和 app 里 light 的取值一致
    cmd.cwd(&cwd);

    let mut child = pair.slave.spawn_command(cmd).expect("spawn codex");
    drop(pair.slave);

    let mut reader = pair.master.try_clone_reader().expect("reader");
    let mut writer = pair.master.take_writer().expect("writer");

    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });

    let mut all: Vec<u8> = Vec::new();
    let started = Instant::now();
    let mut answered = false;
    // 敲一些键把 codex 逼进更多界面状态（斜杠菜单、@ 文件选择、弹窗…），
    // 这样能看到它是否会用「浅底 + 深字」的组合。
    let pokes: Vec<(u64, &[u8])> = vec![
        (3000, b"/"),
        (4500, b"\x1b[B"),
        (5000, b"\x1b[B"),
        (6000, b"\x1b"),
        (7000, b"@"),
        (8500, b"\x1b"),
        (9500, b"?"),
    ];
    let mut poke_i = 0;
    while started.elapsed() < Duration::from_secs(14) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(chunk) => all.extend_from_slice(&chunk),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(_) => break,
        }
        if poke_i < pokes.len() && started.elapsed() >= Duration::from_millis(pokes[poke_i].0) {
            use std::io::Write;
            let _ = writer.write_all(pokes[poke_i].1);
            let _ = writer.flush();
            poke_i += 1;
        }
        // codex 查询背景色 → 按参数决定回不回答
        if !answered {
            if let Some(ref reply) = answer {
                if find(&all, b"\x1b]11;?").is_some() {
                    use std::io::Write;
                    let _ = write!(writer, "\x1b]11;{}\x07", reply);
                    let _ = writer.flush();
                    answered = true;
                    eprintln!(">>> codex 查询了 OSC 11，已回答 {reply}");
                }
            }
        }
    }

    let _ = child.kill();

    let queried_11 = find(&all, b"\x1b]11;?").is_some();
    let queried_10 = find(&all, b"\x1b]10;?").is_some();
    println!("== 抓到 {} 字节 ==", all.len());
    println!("codex 查询 OSC 11 (背景色): {queried_11}");
    println!("codex 查询 OSC 10 (前景色): {queried_10}");
    if answer.is_some() {
        println!("已回答 OSC 11: {answered}");
    }

    let mut sgrs: BTreeSet<String> = BTreeSet::new();
    let mut i = 0;
    while i + 2 < all.len() {
        if all[i] == 0x1b && all[i + 1] == b'[' {
            let mut end = i + 2;
            while end < all.len() && !(all[end] >= 0x40 && all[end] <= 0x7e) {
                end += 1;
            }
            if end < all.len() && all[end] == b'm' {
                let params = String::from_utf8_lossy(&all[i + 2..end]).to_string();
                // 只关心带颜色的
                if params.contains("38") || params.contains("48") || params.contains("3") {
                    sgrs.insert(params);
                }
            }
            i = end;
        }
        i += 1;
    }
    println!("\n== 出现过的 SGR 颜色序列 ({}) ==", sgrs.len());
    for s in &sgrs {
        let note = describe(s);
        println!("  ESC[{}m{}", s, note);
    }
}

/// 标出「深字」「浅底」——如果 codex 会画浅底，那镜像前景就有把字弄没的风险。
fn describe(params: &str) -> String {
    let p: Vec<&str> = params.split(';').collect();
    let mut out = String::new();
    let mut i = 0;
    while i < p.len() {
        if (p[i] == "38" || p[i] == "48") && i + 4 < p.len() && p[i + 1] == "2" {
            let rgb: Vec<f64> = p[i + 2..i + 5].iter().filter_map(|v| v.parse().ok()).collect();
            if rgb.len() == 3 {
                let luma = rgb[0] * 0.299 + rgb[1] * 0.587 + rgb[2] * 0.114;
                let what = if p[i] == "38" { "前景" } else { "背景" };
                let tone = if luma > 128.0 { "浅" } else { "深" };
                out.push_str(&format!("   ← {what}{tone} (luma {luma:.0})"));
            }
            i += 5;
        } else {
            i += 1;
        }
    }
    out
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}
