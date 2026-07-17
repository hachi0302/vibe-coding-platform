// Git 变更查看面板的后端支持：全部通过 `git` 子进程 CLI 调用，不引入 git 库。
// `cwd` 决定仓库位置；`hash` / `path` 是用户可控输入，经 stdin/参数拼进 shell 之外的
// `Command::args`（无 shell 解释），但仍需白名单校验防止路径穿越或参数注入（如 `--upload-pack`）。

use crate::types::{DiffHunk, GitCommit, GitDiffFile, GitFileStatus};
use crate::util::{parse_unified_diff, silent_command};

fn valid_hash(s: &str) -> bool {
    (7..=40).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn valid_path(p: &str) -> bool {
    !p.is_empty() && !p.starts_with('/') && !p.split('/').any(|seg| seg == "..")
}

fn repo_root(cwd: &str) -> Result<String, String> {
    let output = silent_command("git")
        .arg("-C")
        .arg(cwd)
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn run_git(cwd: &str, args: &[&str]) -> Result<String, String> {
    let output = silent_command("git")
        .arg("-C")
        .arg(cwd)
        .args(args)
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

pub fn git_has_repo(cwd: &str) -> bool {
    silent_command("git")
        .arg("-C")
        .arg(cwd)
        .arg("rev-parse")
        .arg("--git-dir")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn parse_log_output(text: &str) -> Vec<GitCommit> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, '\u{0}');
            Some(GitCommit {
                hash: parts.next()?.to_string(),
                author: parts.next()?.to_string(),
                date: parts.next()?.to_string(),
                message: parts.next()?.to_string(),
            })
        })
        .collect()
}

pub fn git_log(cwd: &str, limit: Option<u32>) -> Result<Vec<GitCommit>, String> {
    let limit_flag = format!("-{}", limit.unwrap_or(50));
    let out = run_git(
        cwd,
        &["log", &limit_flag, "--format=%H%x00%an%x00%aI%x00%s"],
    )?;
    Ok(parse_log_output(&out))
}

fn parse_status_output(text: &str) -> Vec<GitFileStatus> {
    text.lines()
        .filter(|line| line.len() >= 3)
        .map(|line| {
            let xy = &line[0..2];
            let rest = line[3..].trim();
            let path = match rest.split_once(" -> ") {
                Some((_, new_path)) => new_path.to_string(),
                None => rest.to_string(),
            };
            let status = if xy == "??" {
                "?".to_string()
            } else {
                let x = xy.as_bytes()[0] as char;
                let y = xy.as_bytes()[1] as char;
                (if x != ' ' { x } else { y }).to_string()
            };
            GitFileStatus { path, status }
        })
        .collect()
}

pub fn git_status(cwd: &str) -> Result<Vec<GitFileStatus>, String> {
    let out = run_git(cwd, &["status", "--porcelain", "-uall"])?;
    Ok(parse_status_output(&out))
}

fn parse_numstat_output(text: &str) -> Vec<GitDiffFile> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(3, '\t');
            let additions_raw = parts.next()?;
            let deletions_raw = parts.next()?;
            let path = parts.next()?.to_string();
            let additions: u32 = additions_raw.parse().unwrap_or(0);
            let deletions: u32 = deletions_raw.parse().unwrap_or(0);
            let status = if additions == 0 {
                "D"
            } else if deletions == 0 {
                "A"
            } else {
                "M"
            };
            Some(GitDiffFile {
                path,
                additions,
                deletions,
                status: status.to_string(),
            })
        })
        .collect()
}

pub fn git_diff_files(cwd: &str, git_ref: &str) -> Result<Vec<GitDiffFile>, String> {
    let root = repo_root(cwd)?;
    let out = if git_ref == "working" {
        run_git(&root, &["diff", "HEAD", "--numstat"])?
    } else {
        if !valid_hash(git_ref) {
            return Err("Invalid commit hash".to_string());
        }
        run_git(
            &root,
            &["diff-tree", "-r", "--numstat", "--no-commit-id", git_ref],
        )?
    };
    Ok(parse_numstat_output(&out))
}

pub fn git_diff_file(cwd: &str, git_ref: &str, path: &str) -> Result<Vec<DiffHunk>, String> {
    if !valid_path(path) {
        return Err("Invalid path".to_string());
    }
    let root = repo_root(cwd)?;
    let out = if git_ref == "working" {
        run_git(&root, &["diff", "HEAD", "--", path])?
    } else {
        if !valid_hash(git_ref) {
            return Err("Invalid commit hash".to_string());
        }
        let range = format!("{git_ref}^..{git_ref}");
        run_git(&root, &["diff", &range, "--", path])?
    };
    Ok(parse_unified_diff(&out))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_hash_accepts_short_and_full_sha() {
        assert!(valid_hash("abc1234"));
        assert!(valid_hash("0123456789abcdef0123456789abcdef01234567"));
    }

    #[test]
    fn valid_hash_rejects_bad_input() {
        assert!(!valid_hash("abc12")); // too short
        assert!(!valid_hash("ABC1234")); // uppercase
        assert!(!valid_hash("abc123g")); // non-hex
        assert!(!valid_hash("")); // empty
    }

    #[test]
    fn valid_path_rejects_traversal_and_absolute() {
        assert!(valid_path("src/util.rs"));
        assert!(!valid_path("../etc/passwd"));
        assert!(!valid_path("src/../../etc/passwd"));
        assert!(!valid_path("/etc/passwd"));
        assert!(!valid_path(""));
    }

    #[test]
    fn parse_log_output_splits_nul_separated_fields() {
        let text = "abc123\u{0}Jane Doe\u{0}2026-07-06T00:00:00Z\u{0}fix: thing\n";
        let commits = parse_log_output(text);
        assert_eq!(commits.len(), 1);
        assert_eq!(commits[0].hash, "abc123");
        assert_eq!(commits[0].author, "Jane Doe");
        assert_eq!(commits[0].message, "fix: thing");
    }

    #[test]
    fn parse_status_output_handles_modified_added_untracked() {
        let text = " M src/util.rs\nA  src/new.rs\n?? src/scratch.rs\n";
        let files = parse_status_output(text);
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].status, "M");
        assert_eq!(files[0].path, "src/util.rs");
        assert_eq!(files[1].status, "A");
        assert_eq!(files[2].status, "?");
    }

    #[test]
    fn parse_status_output_handles_rename() {
        let text = "R  old.rs -> new.rs\n";
        let files = parse_status_output(text);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].status, "R");
        assert_eq!(files[0].path, "new.rs");
    }

    #[test]
    fn parse_numstat_output_derives_status() {
        let text = "5\t0\tsrc/added.rs\n0\t5\tsrc/deleted.rs\n3\t2\tsrc/modified.rs\n";
        let files = parse_numstat_output(text);
        assert_eq!(files.len(), 3);
        assert_eq!(files[0].status, "A");
        assert_eq!(files[1].status, "D");
        assert_eq!(files[2].status, "M");
    }
}
