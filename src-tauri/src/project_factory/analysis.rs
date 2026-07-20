use std::fs;
use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tauri::Emitter;
use uuid::Uuid;

use crate::agent_command::{build_agent_process, AgentCommand};

use super::types::{
    AgentAnalysisProgress, AgentAnalysisResult, AgentStackRecommendation, AnalyzeProjectRequest,
    ClarifyingQuestion, TechnologyDecision,
};

const VIBE_TECH_STACK_SKILL: &str =
    include_str!("../../../.agents/skills/vibe-tech-stack-selection/SKILL.md");
const SOFTWARE_ARCHITECT_SKILL: &str =
    include_str!("../../../.agents/skills/software-architect/SKILL.md");
const ARCHITECTURAL_STYLES: &str =
    include_str!("../../../.agents/skills/software-architect/references/architectural-styles.md");
const BACKEND_RUNTIMES: &str =
    include_str!("../../../.agents/skills/software-architect/references/backend-runtimes.md");
const DATA_LAYER: &str =
    include_str!("../../../.agents/skills/software-architect/references/data-layer.md");
const RUNTIME_AND_FRAMEWORKS: &str = include_str!(
    "../../../.agents/skills/vibe-tech-stack-selection/references/runtime-and-frameworks.md"
);
const DATA_AND_INFRASTRUCTURE: &str = include_str!(
    "../../../.agents/skills/vibe-tech-stack-selection/references/data-and-infrastructure.md"
);
const PRODUCTION_BASELINE: &str = include_str!(
    "../../../.agents/skills/vibe-tech-stack-selection/references/production-baseline.md"
);
const SELECTION_MATRIX: &str = include_str!(
    "../../../.agents/skills/vibe-tech-stack-selection/references/selection-matrix.md"
);
const EVALUATION_CASES: &str = include_str!(
    "../../../.agents/skills/vibe-tech-stack-selection/references/evaluation-cases.md"
);

const TEMPLATE_IDS: &[&str] = &[
    "vue-spring-boot",
    "vue-vite",
    "nextjs",
    "tauri-vue",
    "node-nestjs",
    "vue-fastapi",
    "vue-go",
    "vue-axum",
    "vue-aspnet",
    "fastapi-api",
    "go-api",
    "axum-api",
    "aspnet-api",
];

pub fn build_analysis_prompt(request: &AnalyzeProjectRequest) -> String {
    let clarification_answers = serde_json::to_string(&request.clarification_answers)
        .expect("clarification answers must serialize");
    let clarification_instruction = if request.clarification_answers.is_empty() {
        "这是首轮分析。你必须先提取已确定事实，再只提出会改变选型的未知决策；基于每题推荐答案生成完整推荐方案。"
    } else {
        "用户已修改并确认了部分澄清答案。必须将这些答案视为明确约束，直接生成最终方案，clarifyingQuestions 必须为空数组。"
    };
    format!(
        r#"你是 Vibe Coding Platform 的技术选型分析器。只做分析，不得修改文件、执行安装、创建目录或调用网络。

必须遵守以下嵌入的项目 skill。它们来自本项目，路径只用于说明来源：

<skill path=".agents/skills/vibe-tech-stack-selection/SKILL.md">
{VIBE_TECH_STACK_SKILL}
</skill>

<skill path=".agents/skills/software-architect/SKILL.md">
{SOFTWARE_ARCHITECT_SKILL}
</skill>

<reference path=".agents/skills/software-architect/references/architectural-styles.md">
{ARCHITECTURAL_STYLES}
</reference>

<reference path=".agents/skills/software-architect/references/backend-runtimes.md">
{BACKEND_RUNTIMES}
</reference>

<reference path=".agents/skills/software-architect/references/data-layer.md">
{DATA_LAYER}
</reference>

<reference path=".agents/skills/vibe-tech-stack-selection/references/runtime-and-frameworks.md">
{RUNTIME_AND_FRAMEWORKS}
</reference>

<reference path=".agents/skills/vibe-tech-stack-selection/references/data-and-infrastructure.md">
{DATA_AND_INFRASTRUCTURE}
</reference>

<reference path=".agents/skills/vibe-tech-stack-selection/references/production-baseline.md">
{PRODUCTION_BASELINE}
</reference>

<reference path=".agents/skills/vibe-tech-stack-selection/references/selection-matrix.md">
{SELECTION_MATRIX}
</reference>

<reference path=".agents/skills/vibe-tech-stack-selection/references/evaluation-cases.md">
{EVALUATION_CASES}
</reference>

可创建模板如下。只能从其中选择 id；不得因为模板已有某种语言就忽略项目场景：
- vue-spring-boot：Vue + Spring Boot，前后端分离。
- node-nestjs：Vue + NestJS，前后端分离。
- vue-fastapi / fastapi-api：FastAPI 全栈或 API 服务。
- vue-go / go-api：Go 全栈或 API 服务。
- vue-axum / axum-api：Rust Axum 全栈或 API 服务。
- vue-aspnet / aspnet-api：ASP.NET Core 全栈或 API 服务。
- vue-vite：Vue 单应用。
- nextjs：Next.js 单应用。
- tauri-vue：Tauri + Vue 桌面应用。

你必须为每个方案输出 `decisions`，且每类只能有一项：frontend、business-backend、agent、persistence、data-access、cache、messaging、configuration、architecture、deployment、observability。

选型展示分组固定为：
- 前端应用：frontend，覆盖前端框架、语言、构建、状态管理、组件库与前端测试。
- 业务后端：business-backend 与 data-access，覆盖业务语言、框架、API/认证与数据访问方式。
- Agent 服务：agent，只在需求明确需要 LLM、工具调用、工作流、RAG、会话记忆或 Agent 编排时采用；否则必须 `not-needed` 或 `defer`，不得为了堆砌技术而引入。
- 数据与基础设施：persistence、cache、messaging、configuration，覆盖数据库、缓存、消息、配置中心等。
- 部署与工程化：architecture、deployment、observability，覆盖单体/前后端分离/微服务取舍、单机/副本/集群部署与日志监控。

每项使用 adopt/optional/defer/not-needed；说明 reason、provision（project/external-platform/existing-platform/not-applicable），defer 必须写 trigger。中间件不能作为创建项目必须安装的本机工具。用户偏好可胜任时权重高；不采用时要说明原因。不得写死任意框架、运行时或中间件版本。

若“已有基础设施”为 `existing-mysql`，且需求没有给出无法兼容 MySQL 的明确约束，`persistence` 必须采用 MySQL，并标记为 `existing-platform`；不得为了 ACID、事务或“中大型项目”这类泛化理由改用 PostgreSQL。MySQL InnoDB 能提供事务、行级锁和 ACID 能力。已有其他团队平台时，也必须优先复用并说明例外成本。

事实与一致性约束：只能把用户明确提供的技术栈、基础设施或本机检测结果写成“已有”。未知信息必须写入 assumptions。不得声称某开源项目或框架“默认使用”某数据库，除非需求中已提供可验证证据。不得声称数据库事务可以使文件写入、Git 提交、外部 API 或消息投递与数据库提交原子一致；跨资源一致性只能建议任务状态机、幂等、Outbox、补偿、审批或可恢复重试。

本产品不提供固定技术选项菜单。你必须根据需求自行识别产品形态、用户、业务模型、既有系统、团队偏好、合规边界、规模和交付约束；只有用户明确表达或在澄清答案中确认的信息才能视为事实。需求未包含 AI 能力时，Agent 服务不得采用。微服务和 Kubernetes 只有在规模、独立扩缩容、团队边界或用户明确约束成立时才可采用，不能因“生产可用”而默认引入。

澄清问题规则：
- `recognizedConstraints` 必须列出已从需求或用户答案确定的事实。不得重复询问这些事实。
- `clarifyingQuestions` 只包含“当前无法可靠判断且答案会改变最终技术方案”的问题。优先 0 到 3 项，最多 10 项；不确定但不影响第一期选型的信息写入 assumptions，不要提问。
- 每个问题由你通过 `selectionMode` 决定 single 或 multiple。每题 2 到 6 个贴合当前业务的选项，不得罗列通用技术字典或把前端/数据库/MQ 等固定菜单原样返回。
- 每题至少一个选项 `recommended=true`，该推荐必须结合当前需求。single 问题只能有一个推荐选项；multiple 可有多个。推荐方案必须按这些推荐答案生成。
- 已有用户答案时，答案优先于推荐答案，并且不得再次返回问题。

面向非技术用户的提问规则：
- 除非需求明确表明决策者是开发者、技术团队，或主动要求指定技术栈，否则将用户视为非技术用户。
- 非技术用户不得让用户在 Java、TypeScript、MySQL、PostgreSQL、框架、构建工具、ORM、缓存、MQ 或部署产品之间选择；这些由你根据需求自行决策，并在最终方案以易懂理由解释。
- 需要确认既有能力时，只能使用业务语言，例如“是否需要接入公司已有系统或数据”，不能要求用户辨别数据库或中间件品牌。用户回答“不确定”时按保守独立方案处理并写入 assumptions。
- 互斥的备选项必须使用 single，例如数据库、后端语言、前端框架、架构形态和部署形态；只有可以同时成立的业务能力，例如支持的支付方式、终端渠道、角色范围，才能使用 multiple。

当前分析模式：{clarification_instruction}

请为当前需求生成一个简洁、合法的 kebab-case 项目名和命名理由。项目名不是用户输入，不得从空字符串推测；要从业务领域和终端语义推导。

需求：{text}
项目结构偏好：{structure_preference}
用户澄清答案（JSON）：{clarification_answers}

当项目结构偏好为“单体项目”时，推荐方案的 structure 必须为 single-app；当为“前后端分离”时，structure 必须为 frontend-backend；为“自动推荐”或未提供时，按需求自行判断。

只输出 JSON，不要 Markdown、解释前缀或代码块。JSON 必须匹配调用方提供的 schema。"#,
        clarification_instruction = clarification_instruction,
        text = request.text,
        structure_preference = request
            .structure_preference
            .as_deref()
            .unwrap_or("自动推荐"),
        clarification_answers = clarification_answers,
    )
}

fn schema() -> String {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": ["recommended", "alternatives", "notRecommended", "assumptions", "projectName", "projectNameReason", "recognizedConstraints", "clarifyingQuestions"],
        "properties": {
            "recommended": { "$ref": "#/$defs/recommendation" },
            "alternatives": { "type": "array", "maxItems": 2, "items": { "$ref": "#/$defs/recommendation" } },
            "notRecommended": { "type": "array", "maxItems": 3, "items": { "$ref": "#/$defs/recommendation" } },
            "assumptions": { "type": "array", "maxItems": 4, "items": { "type": "string" } },
            "projectName": { "type": "string", "pattern": "^[a-z][a-z0-9-]{1,62}$" },
            "projectNameReason": { "type": "string", "minLength": 1 },
            "recognizedConstraints": { "type": "array", "maxItems": 12, "items": { "$ref": "#/$defs/recognizedConstraint" } },
            "clarifyingQuestions": { "type": "array", "maxItems": 10, "items": { "$ref": "#/$defs/clarifyingQuestion" } }
        },
        "$defs": {
            "recommendation": {
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "title", "frontend", "backend", "database", "cache", "messaging", "decisions", "structure", "packageManager", "reasons", "tradeoffs", "preferenceMatched"],
                "properties": {
                    "id": { "type": "string", "enum": TEMPLATE_IDS },
                    "title": { "type": "string" },
                    "frontend": { "type": "array", "items": { "type": "string" } },
                    "backend": { "type": "array", "items": { "type": "string" } },
                    "database": { "type": "array", "items": { "type": "string" } },
                    "cache": { "type": "array", "items": { "type": "string" } },
                    "messaging": { "type": "array", "items": { "type": "string" } },
                    "decisions": { "type": "array", "minItems": 11, "maxItems": 11, "items": { "$ref": "#/$defs/decision" } },
                    "structure": { "type": "string", "enum": ["single-app", "frontend-backend", "monorepo"] },
                    "packageManager": { "type": "string", "enum": ["npm", "maven", "gradle", "pip", "go", "cargo", "dotnet"] },
                    "reasons": { "type": "array", "minItems": 1, "items": { "type": "string" } },
                    "tradeoffs": { "type": "array", "minItems": 1, "items": { "type": "string" } },
                    "preferenceMatched": { "type": "boolean" }
                }
            },
            "decision": {
                "type": "object",
                "additionalProperties": false,
                "required": ["category", "title", "status", "choices", "reason", "provision", "trigger"],
                "properties": {
                    "category": { "type": "string", "enum": ["frontend", "business-backend", "agent", "persistence", "data-access", "cache", "messaging", "configuration", "architecture", "deployment", "observability"] },
                    "title": { "type": "string", "minLength": 1 },
                    "status": { "type": "string", "enum": ["adopt", "optional", "defer", "not-needed"] },
                    "choices": { "type": "array", "items": { "type": "string" } },
                    "reason": { "type": "string", "minLength": 1 },
                    "provision": { "type": "string", "enum": ["project", "external-platform", "existing-platform", "not-applicable"] },
                    "trigger": { "type": ["string", "null"] }
                }
            },
            "recognizedConstraint": {
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "label", "value"],
                "properties": {
                    "id": { "type": "string", "minLength": 1 },
                    "label": { "type": "string", "minLength": 1 },
                    "value": { "type": "string", "minLength": 1 }
                }
            },
            "clarifyingQuestion": {
                "type": "object",
                "additionalProperties": false,
                "required": ["id", "label", "description", "selectionMode", "options"],
                "properties": {
                    "id": { "type": "string", "pattern": "^[a-z][a-z0-9-]{1,62}$" },
                    "label": { "type": "string", "minLength": 1 },
                    "description": { "type": ["string", "null"] },
                    "selectionMode": { "type": "string", "enum": ["single", "multiple"] },
                    "options": { "type": "array", "minItems": 2, "maxItems": 6, "items": { "$ref": "#/$defs/clarifyingOption" } }
                }
            },
            "clarifyingOption": {
                "type": "object",
                "additionalProperties": false,
                "required": ["value", "label", "description", "recommended"],
                "properties": {
                    "value": { "type": "string", "pattern": "^[a-z][a-z0-9-]{1,62}$" },
                    "label": { "type": "string", "minLength": 1 },
                    "description": { "type": ["string", "null"] },
                    "recommended": { "type": "boolean" }
                }
            }
        }
    })
    .to_string()
}

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "vibe-project-factory-{label}-{}.json",
        Uuid::new_v4()
    ))
}

fn concise_cli_error(output: &[u8]) -> String {
    let text = String::from_utf8_lossy(output);
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let tail = lines.len().saturating_sub(12);
    lines[tail..].join("\n")
}

fn validate_recommendation(item: &AgentStackRecommendation) -> Result<(), String> {
    if !TEMPLATE_IDS.contains(&item.id.as_str()) {
        return Err(format!("智能体返回了当前无法生成的模板：{}", item.id));
    }
    if item.reasons.is_empty() || item.tradeoffs.is_empty() {
        return Err("智能体返回的方案缺少推荐理由或取舍".to_string());
    }
    let expected_structure = match item.id.as_str() {
        "vue-spring-boot" | "node-nestjs" | "vue-fastapi" | "vue-go" | "vue-axum"
        | "vue-aspnet" => "frontend-backend",
        "vue-vite" | "nextjs" | "tauri-vue" | "fastapi-api" | "go-api" | "axum-api"
        | "aspnet-api" => "single-app",
        _ => unreachable!("template ids were checked above"),
    };
    if item.structure != expected_structure {
        return Err(format!(
            "模板 {} 的项目结构必须是 {expected_structure}",
            item.id
        ));
    }
    validate_decisions(&item.decisions)?;
    Ok(())
}

fn validate_decisions(decisions: &[TechnologyDecision]) -> Result<(), String> {
    const CATEGORIES: [&str; 11] = [
        "frontend",
        "business-backend",
        "agent",
        "persistence",
        "data-access",
        "cache",
        "messaging",
        "configuration",
        "architecture",
        "deployment",
        "observability",
    ];
    for category in CATEGORIES {
        let decision = decisions
            .iter()
            .find(|decision| decision.category == category)
            .ok_or_else(|| format!("智能体返回的方案缺少 {category} 决策"))?;
        if !["adopt", "optional", "defer", "not-needed"].contains(&decision.status.as_str()) {
            return Err(format!("{} 的决策状态无效", decision.title));
        }
        if decision.reason.trim().is_empty() || decision.title.trim().is_empty() {
            return Err(format!("{category} 决策缺少理由或标题"));
        }
        if decision.status == "defer" && decision.trigger.as_deref().unwrap_or("").trim().is_empty()
        {
            return Err(format!("{category} 延后决策缺少触发条件"));
        }
    }
    Ok(())
}

fn validate_structure_preference(
    result: AgentAnalysisResult,
    preference: Option<&str>,
) -> Result<AgentAnalysisResult, String> {
    let expected = match preference.unwrap_or("auto") {
        "single-app" => Some("single-app"),
        "frontend-backend" => Some("frontend-backend"),
        _ => None,
    };
    if let Some(expected) = expected {
        if result.recommended.structure != expected {
            return Err(format!(
                "智能体未遵守项目结构偏好：期望 {expected}，实际 {}",
                result.recommended.structure
            ));
        }
    }
    Ok(result)
}

fn validate_clarifying_questions(questions: &[ClarifyingQuestion]) -> Result<(), String> {
    if questions.len() > 10 {
        return Err("智能体返回的澄清问题超过 10 项".to_string());
    }
    let mut ids = std::collections::HashSet::new();
    for question in questions {
        if question.id.trim().is_empty() || !ids.insert(&question.id) {
            return Err("智能体返回了空白或重复的澄清问题标识".to_string());
        }
        if !["single", "multiple"].contains(&question.selection_mode.as_str()) {
            return Err(format!("澄清问题 {} 的选择方式无效", question.label));
        }
        if !(2..=6).contains(&question.options.len()) {
            return Err(format!(
                "澄清问题 {} 的选项数量必须在 2 到 6 项",
                question.label
            ));
        }
        let recommended = question
            .options
            .iter()
            .filter(|option| option.recommended)
            .count();
        if recommended == 0 || (question.selection_mode == "single" && recommended != 1) {
            return Err(format!("澄清问题 {} 缺少有效推荐答案", question.label));
        }
    }
    Ok(())
}

fn validate_refinement_result(
    result: AgentAnalysisResult,
    has_clarification_answers: bool,
) -> Result<AgentAnalysisResult, String> {
    if has_clarification_answers && !result.clarifying_questions.is_empty() {
        return Err("智能体在收到用户澄清答案后仍重复提问".to_string());
    }
    Ok(result)
}

fn parse_analysis(provider: &str, text: &str) -> Result<AgentAnalysisResult, String> {
    let mut result: AgentAnalysisResult = serde_json::from_str(text)
        .map_err(|error| format!("{provider} 返回的选型结果不是有效 JSON：{error}"))?;
    validate_recommendation(&result.recommended)?;
    for item in result
        .alternatives
        .iter()
        .chain(result.not_recommended.iter())
    {
        validate_recommendation(item)?;
    }
    validate_clarifying_questions(&result.clarifying_questions)?;
    result.provider = provider.to_string();
    Ok(result)
}

fn build_codex_process(
    prompt: &str,
    schema_path: &Path,
    output_path: &Path,
) -> std::process::Command {
    let cwd = env!("CARGO_MANIFEST_DIR").trim_end_matches("/src-tauri");
    let command = AgentCommand::new("codex")
        .arg("exec")
        .arg("--sandbox")
        .arg("read-only")
        .arg("--ephemeral")
        .arg("--skip-git-repo-check")
        .arg("--output-schema")
        .arg(schema_path.to_string_lossy().into_owned())
        .arg("--output-last-message")
        .arg(output_path.to_string_lossy().into_owned())
        .arg("-C")
        .arg(cwd)
        .arg(prompt);
    build_agent_process(cwd, &command, false)
}

fn run_codex(
    prompt: &str,
    schema_path: &Path,
    output_path: &Path,
) -> Result<AgentAnalysisResult, String> {
    let output = build_codex_process(prompt, schema_path, output_path)
        .output()
        .map_err(|error| format!("无法启动 Codex CLI：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Codex CLI 分析失败：{}",
            concise_cli_error(&output.stderr)
        ));
    }
    let content = fs::read_to_string(output_path)
        .map_err(|error| format!("无法读取 Codex 分析结果：{error}"))?;
    parse_analysis("codex", &content)
}

fn build_claude_process(prompt: &str, schema: &str) -> std::process::Command {
    let cwd = env!("CARGO_MANIFEST_DIR").trim_end_matches("/src-tauri");
    let command = AgentCommand::new("claude")
        .arg("--print")
        .arg("--no-session-persistence")
        .arg("--permission-mode")
        .arg("plan")
        .arg("--tools")
        .arg("Read")
        .arg("--output-format")
        .arg("json")
        .arg("--json-schema")
        .arg(schema)
        .arg(prompt);
    build_agent_process(cwd, &command, false)
}

fn run_claude(prompt: &str, schema: &str) -> Result<AgentAnalysisResult, String> {
    let output = build_claude_process(prompt, schema)
        .output()
        .map_err(|error| format!("无法启动 Claude Code：{error}"))?;
    if !output.status.success() {
        return Err(format!(
            "Claude Code 分析失败：{}",
            concise_cli_error(&output.stderr)
        ));
    }
    let outer: Value = serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("Claude Code 返回格式异常：{error}"))?;
    let content = outer
        .get("result")
        .and_then(Value::as_str)
        .ok_or_else(|| "Claude Code 未返回结构化选型结果".to_string())?;
    parse_analysis("claude", content)
}

fn analyze_with_progress<F>(
    request: &AnalyzeProjectRequest,
    mut report: F,
) -> Result<AgentAnalysisResult, String>
where
    F: FnMut(AgentAnalysisProgress),
{
    if request.text.trim().is_empty() {
        return Err("项目需求不能为空".to_string());
    }
    report(AgentAnalysisProgress {
        phase: "prepare".to_string(),
        percent: 12,
        detail: "正在整理需求与项目约束".to_string(),
    });
    let prompt = build_analysis_prompt(request);
    let schema = schema();
    let schema_path = temp_path("analysis-schema");
    let output_path = temp_path("analysis-output");
    fs::write(&schema_path, &schema).map_err(|error| format!("无法创建选型 schema：{error}"))?;

    report(AgentAnalysisProgress {
        phase: "codex".to_string(),
        percent: 42,
        detail: "正在比较候选技术方案".to_string(),
    });
    let codex_result = run_codex(&prompt, &schema_path, &output_path)
        .and_then(|result| {
            validate_structure_preference(result, request.structure_preference.as_deref())
        })
        .and_then(|result| {
            validate_refinement_result(result, !request.clarification_answers.is_empty())
        });
    let _ = fs::remove_file(&schema_path);
    let _ = fs::remove_file(&output_path);
    match codex_result {
        Ok(result) => {
            report(AgentAnalysisProgress {
                phase: "validate".to_string(),
                percent: 88,
                detail: "正在校验推荐结果".to_string(),
            });
            Ok(result)
        }
        Err(_codex_error) => {
            report(AgentAnalysisProgress {
                phase: "claude".to_string(),
                percent: 58,
                detail: "正在生成候选技术方案".to_string(),
            });
            let result = run_claude(&prompt, &schema)
                .and_then(|result| {
                    validate_structure_preference(result, request.structure_preference.as_deref())
                })
                .and_then(|result| {
                    validate_refinement_result(result, !request.clarification_answers.is_empty())
                })
                .map_err(|claude_error| {
                    format!("自动选型暂不可用，请检查本机智能体配置后重试。原因：{claude_error}")
                })?;
            report(AgentAnalysisProgress {
                phase: "validate".to_string(),
                percent: 88,
                detail: "正在校验推荐结果".to_string(),
            });
            Ok(result)
        }
    }
}

pub fn analyze_with_agent(request: &AnalyzeProjectRequest) -> Result<AgentAnalysisResult, String> {
    analyze_with_progress(request, |_| {})
}

pub fn analyze_with_agent_progress(
    app: &tauri::AppHandle,
    request: &AnalyzeProjectRequest,
) -> Result<AgentAnalysisResult, String> {
    analyze_with_progress(request, |progress| {
        let _ = app.emit("project-factory://analysis-progress", progress);
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rendered_process(process: &std::process::Command) -> String {
        process
            .get_args()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn analysis_agents_use_packaged_gui_path_resolution() {
        let codex = build_codex_process(
            "analyze it",
            Path::new("/tmp/schema path.json"),
            Path::new("/tmp/output path.json"),
        );
        let claude = build_claude_process("analyze it", "{}");
        let codex_rendered = rendered_process(&codex);
        let claude_rendered = rendered_process(&claude);

        assert!(codex_rendered.contains("codex"));
        assert!(claude_rendered.contains("claude"));
        #[cfg(unix)]
        {
            assert!(codex_rendered.starts_with("-l -i -c "));
            assert!(claude_rendered.starts_with("-l -i -c "));
        }
        #[cfg(windows)]
        {
            assert!(codex_rendered.contains("GetEnvironmentVariable('Path', 'Machine')"));
            assert!(claude_rendered.contains("GetEnvironmentVariable('Path', 'Machine')"));
        }
    }

    fn analysis_with_questions(count: usize) -> AgentAnalysisResult {
        let question = ClarifyingQuestion {
            id: "target-user".to_string(),
            label: "主要使用者是谁？".to_string(),
            description: Some("会影响权限与交付形态。".to_string()),
            selection_mode: "single".to_string(),
            options: vec![
                super::super::types::ClarifyingOption {
                    value: "internal".to_string(),
                    label: "内部人员".to_string(),
                    description: None,
                    recommended: true,
                },
                super::super::types::ClarifyingOption {
                    value: "external".to_string(),
                    label: "外部用户".to_string(),
                    description: None,
                    recommended: false,
                },
            ],
        };
        let clarifying_questions = (0..count)
            .map(|index| ClarifyingQuestion {
                id: format!("question-{index}"),
                ..question.clone()
            })
            .collect();

        AgentAnalysisResult {
            provider: "codex".to_string(),
            recommended: AgentStackRecommendation {
                id: "nextjs".to_string(),
                title: "Next.js".to_string(),
                frontend: vec![],
                backend: vec![],
                database: vec![],
                cache: vec![],
                messaging: vec![],
                decisions: vec![],
                structure: "single-app".to_string(),
                package_manager: "npm".to_string(),
                reasons: vec![],
                tradeoffs: vec![],
                preference_matched: false,
            },
            alternatives: vec![],
            not_recommended: vec![],
            assumptions: vec![],
            project_name: "test-project".to_string(),
            project_name_reason: "测试".to_string(),
            recognized_constraints: vec![],
            clarifying_questions,
        }
    }

    #[test]
    fn rejects_an_unknown_template_returned_by_an_agent() {
        let response = r#"{
          "recommended":{"id":"flutter","title":"Flutter","frontend":[],"backend":[],"database":[],"cache":[],"messaging":[],"structure":"single-app","packageManager":"npm","reasons":["x"],"tradeoffs":["y"],"preferenceMatched":false},
          "alternatives":[],"notRecommended":[],"assumptions":[]
        }"#;
        assert!(parse_analysis("codex", response).is_err());
    }

    #[test]
    fn adds_provider_to_a_schema_valid_agent_response() {
        let response = r#"{
          "recommended":{"id":"nextjs","title":"Next.js","frontend":["Next.js"],"backend":[],"database":[],"cache":[],"messaging":[],"decisions":[
            {"category":"frontend","title":"前端应用","status":"adopt","choices":["Next.js"],"reason":"SEO","provision":"project"},
            {"category":"business-backend","title":"业务后端","status":"not-needed","choices":[],"reason":"静态站","provision":"not-applicable"},
            {"category":"agent","title":"Agent 服务","status":"not-needed","choices":[],"reason":"未要求 AI 能力","provision":"not-applicable"},
            {"category":"persistence","title":"数据","status":"not-needed","choices":[],"reason":"静态站","provision":"not-applicable"},
            {"category":"data-access","title":"数据访问","status":"not-needed","choices":[],"reason":"静态站","provision":"not-applicable"},
            {"category":"cache","title":"缓存","status":"not-needed","choices":[],"reason":"静态站","provision":"not-applicable"},
            {"category":"messaging","title":"消息","status":"not-needed","choices":[],"reason":"静态站","provision":"not-applicable"},
            {"category":"configuration","title":"配置","status":"not-needed","choices":[],"reason":"静态站","provision":"not-applicable"},
            {"category":"architecture","title":"架构形态","status":"adopt","choices":["单应用"],"reason":"静态站","provision":"project"},
            {"category":"deployment","title":"部署形态","status":"adopt","choices":["静态托管"],"reason":"无需服务集群","provision":"external-platform"},
            {"category":"observability","title":"上线","status":"adopt","choices":["健康检查"],"reason":"上线需要","provision":"project"}
          ],"structure":"single-app","packageManager":"npm","reasons":["SEO"],"tradeoffs":["React"],"preferenceMatched":false},
          "alternatives":[],"notRecommended":[],"assumptions":["对外网站"],"projectName":"brand-site","projectNameReason":"品牌官网",
          "recognizedConstraints":[{"id":"public-site","label":"产品形态","value":"对外品牌官网"}],
          "clarifyingQuestions":[{"id":"content-owner","label":"内容由谁维护？","description":"内容更新方式会影响后台边界。","selectionMode":"single","options":[{"value":"internal","label":"内部运营维护","description":null,"recommended":true},{"value":"external","label":"外包维护","description":null,"recommended":false}]}]
        }"#;
        let result = parse_analysis("codex", response).expect("schema response must parse");
        assert_eq!(result.provider, "codex");
        assert_eq!(result.recommended.id, "nextjs");
        assert_eq!(result.recognized_constraints.len(), 1);
        assert_eq!(result.clarifying_questions[0].selection_mode, "single");
    }

    #[test]
    fn keeps_only_the_cli_error_tail_for_the_ui() {
        let output = b"prompt\nsecret requirement\nline 1\nline 2\nline 3\nline 4\nline 5\nline 6\nline 7\nline 8\nline 9\nline 10\nline 11\nline 12\nactual error";
        let error = concise_cli_error(output);
        assert!(!error.contains("secret requirement"));
        assert!(error.ends_with("actual error"));
    }

    #[test]
    fn rejects_more_than_ten_dynamic_clarifying_questions() {
        let result =
            validate_clarifying_questions(&analysis_with_questions(11).clarifying_questions);
        assert!(result.is_err_and(|error| error.contains("超过 10 项")));
    }

    #[test]
    fn rejects_follow_up_questions_after_user_answers() {
        let result = validate_refinement_result(analysis_with_questions(1), true);
        assert!(result.is_err_and(|error| error.contains("仍重复提问")));
    }
}
