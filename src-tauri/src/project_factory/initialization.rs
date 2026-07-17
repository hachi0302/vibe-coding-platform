use std::path::Path;
use std::process::Command;

use tauri::Emitter;

use super::existing::{
    finalize_existing_project_initialization, prepare_existing_project_initialization,
};
use super::types::{ExistingProjectInitResult, ExistingProjectInitializationProgress};

const MAX_REPAIR_ATTEMPTS: usize = 2;

#[derive(Debug, Clone, Copy)]
enum InitializationStage {
    Documents,
    RulesAndSkills,
    Repair,
}

impl InitializationStage {
    fn instruction(self) -> &'static str {
        match self {
            Self::Documents => {
                "本轮只完成项目分析与长期文档：完整读取项目后，按真实代码填充命中的前端/后端长期文档；保留既有真实资料，不修改业务代码。"
            }
            Self::RulesAndSkills => {
                "本轮完成项目入口、规则与项目专属 skills：先读取刚生成的长期文档和真实代码，再填充 CLAUDE.md、rules 与 skills；不得修改业务代码。"
            }
            Self::Repair => {
                "本轮是产物修复：只补齐校验指出的缺口并重新自检，不重写已经合格的真实文档，不修改业务代码。"
            }
        }
    }
}

/// 给无 UI、无会话持久化的 Agent CLI 使用。初始化过程不得依赖聊天框、`/init` 回显或
/// 模型自行声明 checkpoint；是否完成只由后端的真实文件校验决定。
pub fn build_headless_initialization_prompt(base: &str, validation_error: Option<&str>) -> String {
    let repair = validation_error
        .map(|error| {
            format!(
                "\n上一次产物校验失败：\n{error}\n只补齐校验指出的缺口；不要删除、覆盖或泛化改写已经存在的真实项目资料。\n"
            )
        })
        .unwrap_or_default();
    format!(
        r#"你正在执行 Vibe Coding Platform 的后台非会话任务。用户界面不会展示你的内部思考、命令或交互，因此不要询问用户、不要等待确认，也不要创建聊天会话。

总要求：
- 完整读取项目根目录、源码、构建与测试脚本、配置、已有 docs、CLAUDE.md、AGENTS.md、.claude 和 .agents，再根据真实证据生成产物。
- 第一动作必须逐份读取 `.vibe-coding-platform/init-reference-v3/` 下命中当前代码层的文档、规则与 skill 模板；正式产物保持模板章节、表格和 Gate，但必须用目标项目真实内容填满，禁止复制空模板。
- 所有面向用户的新文档、规则和项目专属 skill 必须使用中文；技术名、类名、方法名、文件路径可保留原文。
- 不得修改业务代码，不得删除或覆盖既有真实业务文档、规则和 skill；仅补缺或合并平台旧空壳。
- 后端固定生成中文实填的业务功能总览与系统架构；只有扫描到真实服务端 Controller / Router / Handler 时才生成 `API接口总览.md`，不得把前端路由当成 API；存在真实回调入口或跨边界枚举证据时才生成对应总览。条件不成立时不创建空文件。
- 检测到真实第三方客户端或 SDK 调用时，生成 `docs/backend/latest/第三方集成/第三方集成总览.md`、项目化 `.claude/rules/后端/异步与第三方规则.md` 与 `.claude/skills/external-integration/SKILL.md`；三者必须引用同一条真实集成链路。
- 存在数据库实体、迁移或 schema 证据时生成物理模型总览，内容是表用途以及字段、类型、是否为空、含义、约束；前端项目不得生成后端或数据库文档。
- 必须保留平台安装的 skill-designer 原始目录和字节，不得改写 skill-designer。
- Worktree 是平台产品能力，不得生成 worktree skill；Git/worktree 纪律只在项目真实存在 Git 时写入 Git 协作规则或入口。
- 不得生成运维、部署、监控或 CI 运维文档和 skill；发现实体、迁移或 schema 证据时用 skill-designer 生成项目专属 `ddl-review` skill。所有后端项目都生成项目专属 `backend-log-diagnose` skill；控制台、本地文件、容器、集中日志和远程日志只登记真实可用来源，未配置来源明确写不可用及接入条件。
- 检测到数据库连接配置时生成项目专属 `database-read-diagnose` skill，只记录配置文件、配置键与脱敏状态，禁止写入账号、密码、token 或连接串；只有安全只读探测成功才标记“可用”，否则写“有证据但需配置 / 不适用”。任何数据库诊断只允许 SELECT / SHOW / DESCRIBE / EXPLAIN 等只读动作。
- 禁止留下 {{占位符}}、待填写、空表和“以后补充”等空壳；正式详设/进度/前端接入模板中的模板占位符除外。
- 禁止吞异常、伪造默认值、模拟成功、自动降级或猜测配置等兜底逻辑。

{base}
{repair}
完成后直接退出。完成状态由平台读取并校验真实文件决定，不需要输出任何工作流检查点。"#
    )
}

fn stage_prompt(base: &str, stage: InitializationStage, validation_error: Option<&str>) -> String {
    format!(
        "{}\n\n当前阶段强制要求：{}",
        build_headless_initialization_prompt(base, validation_error),
        stage.instruction()
    )
}

fn concise_cli_error(output: &[u8]) -> String {
    let text = String::from_utf8_lossy(output);
    let lines = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    lines[lines.len().saturating_sub(16)..].join("\n")
}

fn run_codex(project_path: &str, prompt: &str) -> Result<(), String> {
    let output = Command::new("codex")
        .args([
            "exec",
            "--sandbox",
            "workspace-write",
            "--ephemeral",
            "--skip-git-repo-check",
            "-C",
            project_path,
        ])
        .arg(prompt)
        .output()
        .map_err(|error| format!("无法启动 Codex CLI：{error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Codex 后台初始化失败：{}",
            concise_cli_error(&output.stderr)
        ))
    }
}

fn run_claude(project_path: &str, prompt: &str) -> Result<(), String> {
    let output = Command::new("claude")
        .args([
            "--print",
            "--no-session-persistence",
            "--dangerously-skip-permissions",
            "--output-format",
            "text",
        ])
        .arg(prompt)
        .current_dir(project_path)
        .output()
        .map_err(|error| format!("无法启动 Claude Code：{error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Claude Code 后台初始化失败：{}",
            concise_cli_error(&output.stderr)
        ))
    }
}

fn run_agent(agent: &str, project_path: &str, prompt: &str) -> Result<(), String> {
    match agent {
        "codex" => run_codex(project_path, prompt),
        "claude" => run_claude(project_path, prompt),
        _ => Err("项目初始化只支持 Claude 或 Codex".to_string()),
    }
}

fn report<F>(reporter: &mut F, project_path: &str, phase: &str, percent: u8, detail: &str)
where
    F: FnMut(ExistingProjectInitializationProgress),
{
    reporter(ExistingProjectInitializationProgress {
        project_path: project_path.to_string(),
        phase: phase.to_string(),
        percent,
        detail: detail.to_string(),
    });
}

fn initialize_with_progress<F>(
    project_path: &str,
    agent: &str,
    base_prompt: &str,
    mut reporter: F,
) -> Result<ExistingProjectInitResult, String>
where
    F: FnMut(ExistingProjectInitializationProgress),
{
    if !Path::new(project_path).is_dir() {
        return Err("项目路径不存在或不是目录".to_string());
    }
    report(
        &mut reporter,
        project_path,
        "analyze",
        8,
        "正在分析项目代码、配置与已有资料",
    );
    prepare_existing_project_initialization(project_path)?;

    run_agent(
        agent,
        project_path,
        &stage_prompt(base_prompt, InitializationStage::Documents, None),
    )?;
    report(
        &mut reporter,
        project_path,
        "documents",
        42,
        "项目分析完成，正在填充真实长期文档",
    );

    run_agent(
        agent,
        project_path,
        &stage_prompt(base_prompt, InitializationStage::RulesAndSkills, None),
    )?;
    report(
        &mut reporter,
        project_path,
        "rules",
        72,
        "长期文档已生成，正在生成项目规则与 skills",
    );

    let mut last_error = String::new();
    for attempt in 0..=MAX_REPAIR_ATTEMPTS {
        report(
            &mut reporter,
            project_path,
            "validate",
            88 + attempt as u8 * 4,
            if attempt == 0 {
                "正在校验真实产物、软链接与初始化标识"
            } else {
                "正在修复产物校验发现的缺口"
            },
        );
        match finalize_existing_project_initialization(project_path) {
            Ok(result) => {
                report(&mut reporter, project_path, "complete", 100, "初始化完成");
                return Ok(result);
            }
            Err(error) if attempt < MAX_REPAIR_ATTEMPTS => {
                last_error = error;
                run_agent(
                    agent,
                    project_path,
                    &stage_prompt(base_prompt, InitializationStage::Repair, Some(&last_error)),
                )?;
            }
            Err(error) => {
                last_error = error;
                break;
            }
        }
    }
    Err(format!("初始化产物校验仍未通过：{last_error}"))
}

pub fn initialize_existing_project_with_agent_progress(
    app: &tauri::AppHandle,
    project_path: &str,
    agent: &str,
    prompt: &str,
) -> Result<ExistingProjectInitResult, String> {
    initialize_with_progress(project_path, agent, prompt, |progress| {
        let _ = app.emit("project-factory://initialization-progress", progress);
    })
}
