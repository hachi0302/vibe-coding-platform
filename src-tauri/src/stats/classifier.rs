// "By Activity" 活动分类 —— 把每一轮对话归到一个语义类别。
//
// 直接移植自 codeburn 0.9.10 的 src/classifier.ts，规则纯正则 + 工具集合，
// 不依赖 LLM。13 类：Coding / Debugging / Refactoring / Testing / Feature /
// Git / Build/Deploy / Exploration / Planning / Delegation / Conversation /
// Brainstorming / General。
//
// 输入 = 一个 `Turn`（用户消息 + N 个 assistant call），输出 = TaskCategory。

use once_cell::sync::Lazy;
use regex_lite::Regex;

use crate::stats::types::Turn;

/// 13 类活动 —— 跟前端的 stats.activity.* 翻译 key 对齐。
/// 用 snake_case + 一一对应的 i18n key（Codeburn 用 'build/deploy' 这种带斜杠的串，
/// 我们这里全部下划线化，更适合做 i18n key suffix）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TaskCategory {
    Coding,
    Debugging,
    Feature,
    Refactoring,
    Testing,
    Exploration,
    Planning,
    Delegation,
    Git,
    BuildDeploy,
    Conversation,
    Brainstorming,
    General,
}

impl TaskCategory {
    pub fn key(&self) -> &'static str {
        match self {
            TaskCategory::Coding => "coding",
            TaskCategory::Debugging => "debugging",
            TaskCategory::Feature => "feature",
            TaskCategory::Refactoring => "refactoring",
            TaskCategory::Testing => "testing",
            TaskCategory::Exploration => "exploration",
            TaskCategory::Planning => "planning",
            TaskCategory::Delegation => "delegation",
            TaskCategory::Git => "git",
            TaskCategory::BuildDeploy => "build_deploy",
            TaskCategory::Conversation => "conversation",
            TaskCategory::Brainstorming => "brainstorming",
            TaskCategory::General => "general",
        }
    }

    pub fn all() -> &'static [TaskCategory] {
        &[
            TaskCategory::Coding,
            TaskCategory::Debugging,
            TaskCategory::Feature,
            TaskCategory::Refactoring,
            TaskCategory::Testing,
            TaskCategory::Exploration,
            TaskCategory::Planning,
            TaskCategory::Delegation,
            TaskCategory::Git,
            TaskCategory::BuildDeploy,
            TaskCategory::Conversation,
            TaskCategory::Brainstorming,
            TaskCategory::General,
        ]
    }
}

// ---- 正则（lazy，进程级共享）-----------------------------------------------

static TEST_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r"(?i)\b(test|pytest|vitest|jest|mocha|spec|coverage|npm\s+test|npx\s+vitest|npx\s+jest)\b",
    )
    .unwrap()
});
static GIT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
    r"(?i)\bgit\s+(push|pull|commit|merge|rebase|checkout|branch|stash|log|diff|status|add|reset|cherry-pick|tag)\b"
).unwrap()
});
static BUILD_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
    r"(?i)\b(npm\s+run\s+build|npm\s+publish|pip\s+install|docker|deploy|make\s+build|npm\s+run\s+dev|npm\s+start|pm2|systemctl|brew|cargo\s+build)\b"
).unwrap()
});
static INSTALL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b(npm\s+install|pip\s+install|brew\s+install|apt\s+install|cargo\s+add)\b")
        .unwrap()
});

static DEBUG_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
    r"(?i)\b(fix|bug|error|broken|failing|crash|issue|debug|traceback|exception|stack\s*trace|not\s+working|wrong|unexpected|status\s+code|404|500|401|403)\b"
).unwrap()
});
static FEATURE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
    r"(?i)\b(add|create|implement|new|build|feature|introduce|set\s*up|scaffold|generate|make\s+(?:a|me|the)|write\s+(?:a|me|the))\b"
).unwrap()
});
static REFACTOR_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
    r"(?i)\b(refactor|clean\s*up|rename|reorganize|simplify|extract|restructure|move|migrate|split)\b"
).unwrap()
});
static BRAINSTORM_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
    r"(?i)\b(brainstorm|idea|what\s+if|explore|think\s+about|approach|strategy|design|consider|how\s+should|what\s+would|opinion|suggest|recommend)\b"
).unwrap()
});
static RESEARCH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
    r"(?i)\b(research|investigate|look\s+into|find\s+out|check|search|analyze|review|understand|explain|how\s+does|what\s+is|show\s+me|list|compare)\b"
).unwrap()
});

static FILE_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
    r"(?i)\.(py|js|ts|tsx|jsx|json|yaml|yml|toml|sql|sh|go|rs|java|rb|php|css|html|md|csv|xml)\b"
).unwrap()
});
static SCRIPT_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
    r"(?i)\b(run\s+\S+\.\w+|execute|scrip?t|curl|api\s+\S+|endpoint|request\s+url|fetch\s+\S+|query|database|db\s+\S+)\b"
).unwrap()
});
static URL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)https?://\S+").unwrap());

// ---- 工具集合（按用途分组）-------------------------------------------------

const EDIT_TOOLS: &[&str] = &[
    "Edit",
    "Write",
    "FileEditTool",
    "FileWriteTool",
    "NotebookEdit",
    "cursor:edit",
];
const READ_TOOLS: &[&str] = &[
    "Read",
    "Grep",
    "Glob",
    "FileReadTool",
    "GrepTool",
    "GlobTool",
];
const BASH_TOOLS: &[&str] = &["Bash", "BashTool", "PowerShellTool"];
const TASK_TOOLS: &[&str] = &[
    "TaskCreate",
    "TaskUpdate",
    "TaskGet",
    "TaskList",
    "TaskOutput",
    "TaskStop",
    "TodoWrite",
];
const SEARCH_TOOLS: &[&str] = &["WebSearch", "WebFetch", "ToolSearch"];

fn has_any(tools: &[String], set: &[&str]) -> bool {
    tools.iter().any(|t| set.contains(&t.as_str()))
}

fn has_mcp(tools: &[String]) -> bool {
    tools.iter().any(|t| t.starts_with("mcp__"))
}

fn has_skill(tools: &[String]) -> bool {
    tools.iter().any(|t| t == "Skill")
}

/// 用 tools / hasPlanMode / hasAgentSpawn 的组合先粗分类；返回 None 表示
/// 工具线索不足以判定，调用方 fallback 到关键词。
fn classify_by_tool_pattern(turn: &Turn) -> Option<TaskCategory> {
    let mut tools: Vec<String> = Vec::new();
    let mut has_plan = false;
    let mut has_spawn = false;
    for c in &turn.calls {
        tools.extend(c.tools.iter().cloned());
        has_plan |= c.has_plan_mode;
        has_spawn |= c.has_agent_spawn;
    }
    if tools.is_empty() {
        return None;
    }
    if has_plan {
        return Some(TaskCategory::Planning);
    }
    if has_spawn {
        return Some(TaskCategory::Delegation);
    }
    let edits = has_any(&tools, EDIT_TOOLS);
    let reads = has_any(&tools, READ_TOOLS);
    let bash = has_any(&tools, BASH_TOOLS);
    let tasks = has_any(&tools, TASK_TOOLS);
    let search = has_any(&tools, SEARCH_TOOLS);
    let mcp = has_mcp(&tools);
    let skill = has_skill(&tools);

    if bash && !edits {
        let msg = &turn.user_message;
        if TEST_RE.is_match(msg) {
            return Some(TaskCategory::Testing);
        }
        if GIT_RE.is_match(msg) {
            return Some(TaskCategory::Git);
        }
        if BUILD_RE.is_match(msg) || INSTALL_RE.is_match(msg) {
            return Some(TaskCategory::BuildDeploy);
        }
    }
    if edits {
        return Some(TaskCategory::Coding);
    }
    if bash && reads {
        return Some(TaskCategory::Exploration);
    }
    if bash {
        return Some(TaskCategory::Coding);
    }
    if search || mcp {
        return Some(TaskCategory::Exploration);
    }
    if reads && !edits {
        return Some(TaskCategory::Exploration);
    }
    if tasks && !edits {
        return Some(TaskCategory::Planning);
    }
    if skill {
        return Some(TaskCategory::General);
    }
    None
}

/// "first-match-wins" 关键词分类：返回最早出现的关键词对应的类别。
/// `candidates` 的顺序决定 tie-break 优先级（同位置时先列出的赢）。
fn first_matching_category(
    text: &str,
    candidates: &[(&Lazy<Regex>, TaskCategory)],
) -> Option<TaskCategory> {
    let mut best: Option<(usize, usize, TaskCategory)> = None;
    for (i, (re, cat)) in candidates.iter().enumerate() {
        if let Some(m) = re.find(text) {
            let idx = m.start();
            if best.is_none()
                || idx < best.unwrap().0
                || (idx == best.unwrap().0 && i < best.unwrap().1)
            {
                best = Some((idx, i, *cat));
            }
        }
    }
    best.map(|(_, _, c)| c)
}

/// 工具粗分类拿到 coding / exploration 后，再用关键词精化为更具体的类别。
fn refine_by_keywords(category: TaskCategory, user_message: &str) -> TaskCategory {
    match category {
        TaskCategory::Coding => {
            // tie-break：refactor → feature → debug（refactor 词最具体）
            first_matching_category(
                user_message,
                &[
                    (&REFACTOR_RE, TaskCategory::Refactoring),
                    (&FEATURE_RE, TaskCategory::Feature),
                    (&DEBUG_RE, TaskCategory::Debugging),
                ],
            )
            .unwrap_or(TaskCategory::Coding)
        }
        TaskCategory::Exploration => {
            if RESEARCH_RE.is_match(user_message) {
                TaskCategory::Exploration
            } else if DEBUG_RE.is_match(user_message) {
                TaskCategory::Debugging
            } else {
                TaskCategory::Exploration
            }
        }
        other => other,
    }
}

/// 纯文本会话（assistant 没有调用任何工具）—— 完全靠关键词分类。
fn classify_conversation(user_message: &str) -> TaskCategory {
    if BRAINSTORM_RE.is_match(user_message) {
        return TaskCategory::Brainstorming;
    }
    if RESEARCH_RE.is_match(user_message) {
        return TaskCategory::Exploration;
    }
    if let Some(cat) = first_matching_category(
        user_message,
        &[
            (&FEATURE_RE, TaskCategory::Feature),
            (&DEBUG_RE, TaskCategory::Debugging),
        ],
    ) {
        return cat;
    }
    if FILE_RE.is_match(user_message) || SCRIPT_RE.is_match(user_message) {
        return TaskCategory::Coding;
    }
    if URL_RE.is_match(user_message) {
        return TaskCategory::Exploration;
    }
    TaskCategory::Conversation
}

/// 入口：给一个 Turn 返回它的分类。
pub fn classify(turn: &Turn) -> TaskCategory {
    let tools_empty = turn.calls.iter().all(|c| c.tools.is_empty());
    if tools_empty {
        return classify_conversation(&turn.user_message);
    }
    match classify_by_tool_pattern(turn) {
        Some(cat) => refine_by_keywords(cat, &turn.user_message),
        None => classify_conversation(&turn.user_message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stats::types::{CallRecord, Turn};
    use crate::types::UsageSummary;

    fn turn(user: &str, tools: Vec<&str>) -> Turn {
        Turn {
            user_message: user.to_string(),
            project_path: String::new(),
            session_id: String::new(),
            calls: vec![CallRecord {
                tools: tools.into_iter().map(String::from).collect(),
                ..Default::default()
            }],
            timestamp_ms: 0,
        }
    }

    fn empty_turn(user: &str) -> Turn {
        Turn {
            user_message: user.to_string(),
            project_path: String::new(),
            session_id: String::new(),
            calls: vec![CallRecord::default()],
            timestamp_ms: 0,
        }
    }

    #[test]
    fn no_tools_brainstorm_keyword_wins() {
        let t = empty_turn("Let's brainstorm a new approach");
        assert_eq!(classify(&t), TaskCategory::Brainstorming);
    }

    #[test]
    fn no_tools_url_means_exploration() {
        let t = empty_turn("read https://example.com");
        assert_eq!(classify(&t), TaskCategory::Exploration);
    }

    #[test]
    fn edit_tool_becomes_coding_or_refines() {
        let t = turn("add login button to the page", vec!["Edit"]);
        assert_eq!(classify(&t), TaskCategory::Feature);
        let t = turn("refactor the login flow", vec!["Edit"]);
        assert_eq!(classify(&t), TaskCategory::Refactoring);
        let t = turn("fix the off-by-one bug", vec!["Edit"]);
        assert_eq!(classify(&t), TaskCategory::Debugging);
        let t = turn("just tweak alignment", vec!["Edit"]);
        assert_eq!(classify(&t), TaskCategory::Coding);
    }

    #[test]
    fn bash_with_test_keyword_is_testing() {
        let t = turn("run vitest now", vec!["Bash"]);
        assert_eq!(classify(&t), TaskCategory::Testing);
    }

    #[test]
    fn bash_with_git_keyword_is_git() {
        let t = turn("git push to remote", vec!["Bash"]);
        assert_eq!(classify(&t), TaskCategory::Git);
    }

    #[test]
    fn bash_with_install_is_build_deploy() {
        let t = turn("npm install something", vec!["Bash"]);
        assert_eq!(classify(&t), TaskCategory::BuildDeploy);
    }

    #[test]
    fn read_only_is_exploration() {
        let t = turn("show me where the auth lives", vec!["Read", "Grep"]);
        assert_eq!(classify(&t), TaskCategory::Exploration);
    }

    #[test]
    fn web_search_alone_is_exploration() {
        let t = turn("search for similar prior art", vec!["WebSearch"]);
        assert_eq!(classify(&t), TaskCategory::Exploration);
    }

    #[test]
    fn task_tools_without_edits_is_planning() {
        let t = turn("plan the next steps", vec!["TaskCreate"]);
        assert_eq!(classify(&t), TaskCategory::Planning);
    }

    #[test]
    fn mcp_tool_is_exploration() {
        let t = turn("check via integrations", vec!["mcp__github__list_repos"]);
        assert_eq!(classify(&t), TaskCategory::Exploration);
    }

    #[test]
    fn first_match_wins_handles_add_error_handling() {
        // 历史 bug：DEBUG 在 FEATURE 前匹配，"add error handling" 被错分为 debug。
        // 现在正则按首次出现位置定序，"add" 先于 "error"，应归 Feature。
        let t = turn("add error handling for the upload", vec!["Edit"]);
        assert_eq!(classify(&t), TaskCategory::Feature);
    }

    #[test]
    fn plan_mode_flag_overrides_tools() {
        let mut t = turn("anything", vec!["Edit"]);
        t.calls[0].has_plan_mode = true;
        assert_eq!(classify(&t), TaskCategory::Planning);
    }

    #[test]
    fn agent_spawn_flag_overrides_tools() {
        let mut t = turn("anything", vec!["Edit"]);
        t.calls[0].has_agent_spawn = true;
        assert_eq!(classify(&t), TaskCategory::Delegation);
    }

    #[test]
    fn pure_conversation_with_no_keywords_falls_back_to_conversation() {
        let t = empty_turn("Hi there, thanks!");
        assert_eq!(classify(&t), TaskCategory::Conversation);
    }

    // Smoke test — ensure CallRecord default constructable
    #[test]
    fn call_record_default_smoke() {
        let c = CallRecord::default();
        assert_eq!(c.cost_usd, 0.0);
        assert_eq!(c.usage, UsageSummary::default());
    }
}
