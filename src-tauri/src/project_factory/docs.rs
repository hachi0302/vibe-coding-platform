use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

use super::types::CreateProjectRequest;

const DETAIL_DESIGN_TEMPLATE: &str =
    include_str!("../../../docs/规范约束/文档模板/公共/详设文档模板.md");
const PROGRESS_TEMPLATE: &str =
    include_str!("../../../docs/规范约束/文档模板/公共/开发进度文档模板.md");
const FRONTEND_INTEGRATION_TEMPLATE: &str =
    include_str!("../../../docs/规范约束/文档模板/公共/前端接入说明模板.md");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectLayers {
    pub frontend: bool,
    pub backend: bool,
}

const PROJECT_SCAN_MAX_DEPTH: usize = 5;

fn should_skip_scan_dir(name: &str) -> bool {
    matches!(
        name,
        ".git"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | ".next"
            | ".nuxt"
            | "vendor"
            | "docs"
    )
}

fn collect_named_files(root: &Path, name: &str, depth: usize, files: &mut Vec<PathBuf>) {
    if depth > PROJECT_SCAN_MAX_DEPTH {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && entry.file_name().to_string_lossy() == name {
            files.push(path);
            continue;
        }
        if path.is_dir() && !should_skip_scan_dir(&entry.file_name().to_string_lossy()) {
            collect_named_files(&path, name, depth + 1, files);
        }
    }
}

pub fn project_files_named(root: &Path, name: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_named_files(root, name, 0, &mut files);
    files
}

pub fn project_file_contents(root: &Path, name: &str) -> String {
    project_files_named(root, name)
        .into_iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .collect::<Vec<_>>()
        .join("\n")
        .to_lowercase()
}

fn has_project_file(root: &Path, name: &str) -> bool {
    !project_files_named(root, name).is_empty()
}

fn contains_any(source: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| source.contains(candidate))
}

pub fn project_layers(root: &Path) -> ProjectLayers {
    let package = project_file_contents(root, "package.json");
    let gradle = format!(
        "{}\n{}",
        project_file_contents(root, "build.gradle"),
        project_file_contents(root, "build.gradle.kts")
    );
    let python = format!(
        "{}\n{}\n{}",
        project_file_contents(root, "pyproject.toml"),
        project_file_contents(root, "requirements.txt"),
        project_file_contents(root, "setup.py")
    );
    let cargo = project_file_contents(root, "Cargo.toml");
    let go = project_file_contents(root, "go.mod");
    let go_main = project_file_contents(root, "main.go");
    let dotnet = project_file_contents(root, "Program.cs");
    let ruby = project_file_contents(root, "Gemfile");
    let php = project_file_contents(root, "composer.json");
    let scala = project_file_contents(root, "build.sbt");
    let frontend = has_project_file(root, "App.vue")
        || has_project_file(root, "App.tsx")
        || has_project_file(root, "page.tsx")
        || has_project_file(root, "tauri.conf.json")
        || contains_any(
            &package,
            &[
                "\"vue\"",
                "\"react\"",
                "\"next\"",
                "\"svelte\"",
                "\"@angular/core\"",
                "\"nuxt\"",
                "\"astro\"",
                "\"solid-js\"",
            ],
        );
    let backend = has_project_file(root, "pom.xml")
        || has_project_file(root, "app.module.ts")
        || has_project_file(root, "manage.py")
        || contains_any(
            &gradle,
            &[
                "org.springframework.boot",
                "io.ktor",
                "io.micronaut",
                "io.quarkus",
            ],
        )
        || contains_any(
            &python,
            &["fastapi", "flask", "django", "starlette", "sanic", "falcon"],
        )
        || contains_any(
            &cargo,
            &[
                "axum",
                "actix-web",
                "rocket",
                "warp",
                "poem",
                "salvo",
                "tonic",
                "sqlx",
                "diesel",
                "sea-orm",
            ],
        )
        || contains_any(
            &go,
            &[
                "github.com/gin-gonic/gin",
                "github.com/labstack/echo",
                "github.com/go-chi/chi",
                "github.com/gofiber/fiber",
                "github.com/gorilla/mux",
            ],
        )
        || (go_main.contains("net/http")
            && contains_any(
                &go_main,
                &["http.handlefunc(", "http.listenandserve(", "http.server{"],
            ))
        || contains_any(
            &dotnet,
            &[
                "webapplication",
                "createbuilder",
                "mapget(",
                "mapcontrollers(",
            ],
        )
        || contains_any(&ruby, &["rails", "sinatra", "hanami"])
        || contains_any(
            &php,
            &["laravel/framework", "symfony/framework", "slim/slim"],
        )
        || contains_any(
            &scala,
            &["playframework", "http4s", "akka-http", "pekko-http"],
        )
        || contains_any(
            &package,
            &["@nestjs", "\"express\"", "\"fastify\"", "\"koa\""],
        );
    ProjectLayers { frontend, backend }
}

fn write_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    if path.exists() {
        return Ok(());
    }
    fs::write(path, content).map_err(|error| error.to_string())
}

fn list_or_none(items: &[String], empty: &str) -> String {
    if items.is_empty() {
        empty.to_string()
    } else {
        items.join("、")
    }
}

fn markdown_cell(value: &str) -> String {
    value
        .replace('|', "\\|")
        .replace(['\r', '\n'], " ")
        .trim()
        .to_string()
}

fn bullet_list(items: &[String], empty: &str) -> String {
    if items.is_empty() {
        format!("- {empty}")
    } else {
        items
            .iter()
            .map(|item| format!("- {}", item.trim()))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn decision_status(value: &str) -> &str {
    match value {
        "adopt" => "采用",
        "defer" => "后续引入",
        "not-needed" => "当前不需要",
        _ => value,
    }
}

fn provision_label(value: &str) -> &str {
    match value {
        "project" => "项目内提供",
        "existing-platform" => "复用已有平台",
        "external-service" => "外部服务",
        "not-applicable" => "不适用",
        _ => value,
    }
}

fn has_adopted_relational_database(request: &CreateProjectRequest) -> bool {
    const RELATIONAL_DATABASES: [&str; 7] = [
        "mysql",
        "postgresql",
        "postgres",
        "mariadb",
        "sql server",
        "oracle",
        "sqlite",
    ];
    request.recommendation.decisions.iter().any(|decision| {
        decision.category == "persistence"
            && decision.status == "adopt"
            && decision.choices.iter().any(|choice| {
                let normalized = choice.to_lowercase();
                RELATIONAL_DATABASES
                    .iter()
                    .any(|database| normalized.contains(database))
            })
    })
}

fn requirement_and_stack_document(request: &CreateProjectRequest, layers: ProjectLayers) -> String {
    let requirement = if request.concise_requirement.trim().is_empty() {
        request.profile.summary.trim()
    } else {
        request.concise_requirement.trim()
    };
    let constraints = if request.recognized_constraints.is_empty() {
        "| — | 本次分析未识别到额外硬约束 |\n".to_string()
    } else {
        request
            .recognized_constraints
            .iter()
            .map(|constraint| {
                format!(
                    "| {} | {} |\n",
                    markdown_cell(&constraint.label),
                    markdown_cell(&constraint.value)
                )
            })
            .collect::<String>()
    };
    let decisions = if request.recommendation.decisions.is_empty() {
        "| — | — | 当前分析未返回分项决策 | — | — | — |\n".to_string()
    } else {
        request
            .recommendation
            .decisions
            .iter()
            .map(|decision| {
                format!(
                    "| {} | {} | {} | {} | {} | {} |\n",
                    markdown_cell(&decision.title),
                    decision_status(&decision.status),
                    markdown_cell(&list_or_none(&decision.choices, "无")),
                    provision_label(&decision.provision),
                    markdown_cell(&decision.reason),
                    markdown_cell(decision.trigger.as_deref().unwrap_or("—"))
                )
            })
            .collect::<String>()
    };
    let layers = match (layers.frontend, layers.backend) {
        (true, true) => "前端 + 后端",
        (true, false) => "前端",
        (false, true) => "后端",
        (false, false) => "未识别代码层",
    };
    format!(
        "# {} 项目需求与技术选型\n\n> 本文记录创建项目时由用户确认的需求、约束与技术决策，是后续详设与开发的输入，不代表业务功能已经实现。\n\n## 1. 已确认需求\n\n{}\n\n## 2. 项目边界\n\n- 系统类型：{}\n- 生成的代码层：{}\n- 工程结构：{}\n- 本次交付：工程骨架、项目长期文档、规则和 skills。\n- 本次不交付：业务详细设计、业务代码开发、业务数据表和未被需求确认的接口。\n\n## 3. 已识别约束\n\n| 约束 | 已确认值 |\n|---|---|\n{}\n## 4. 技术方案\n\n- 方案：{}\n- 前端：{}\n- 后端：{}\n- 数据库：{}\n- 缓存：{}\n- 消息：{}\n- 包管理/构建入口：{}\n- 是否命中用户技术偏好：{}\n\n## 5. 分项决策\n\n| 决策 | 状态 | 选择 | 提供方式 | 理由 | 后续触发条件 |\n|---|---|---|---|---|---|\n{}\n## 6. 选择理由\n\n{}\n\n## 7. 代价与取舍\n\n{}\n\n## 8. 分析假设\n\n{}\n\n> 假设不是已确认事实。后续详设读取真实代码、配置和用户材料后，必须验证或删除相应假设，不得据此私自增加兜底逻辑。\n\n## 9. 后续使用方式\n\n后续用户另开 Agent 会话提出具体需求时，先读取本文和 `docs/*/latest/` 长期文档，再使用 `latest/规范约束/` 下模板创建成对的 `v{{版本}}/详细设计` 与 `v{{版本}}/开发进度` 文档；详设经用户确认后才进入开发和自测。\n",
        request.project_name,
        requirement,
        request.profile.system_type,
        layers,
        request.recommendation.structure,
        constraints,
        request.recommendation.title,
        frontend_stack(request),
        backend_stack(request),
        list_or_none(&request.recommendation.database, "当前不采用"),
        list_or_none(&request.recommendation.cache, "当前不采用"),
        list_or_none(&request.recommendation.messaging, "当前不采用"),
        request
            .recommendation
            .package_manager
            .as_deref()
            .unwrap_or("以生成后的构建清单为准"),
        if request.recommendation.preference_matched {
            "是"
        } else {
            "否"
        },
        decisions,
        bullet_list(&request.recommendation.reasons, "未提供额外理由"),
        bullet_list(&request.recommendation.tradeoffs, "未识别额外取舍"),
        bullet_list(&request.assumptions, "无")
    )
}

fn relative_paths(root: &Path, names: &[&str]) -> Vec<String> {
    let mut paths = names
        .iter()
        .flat_map(|name| project_files_named(root, name))
        .filter_map(|path| path.strip_prefix(root).ok().map(Path::to_path_buf))
        .map(|path| format!("`{}`", path.to_string_lossy()))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn evidence_or_none(root: &Path, names: &[&str]) -> String {
    let paths = relative_paths(root, names);
    if paths.is_empty() {
        "当前生成工程中未识别到对应实现。".to_string()
    } else {
        paths.join("、")
    }
}

fn frontend_stack(request: &CreateProjectRequest) -> String {
    list_or_none(&request.recommendation.frontend, "未识别前端技术栈")
}

fn backend_stack(request: &CreateProjectRequest) -> String {
    list_or_none(&request.recommendation.backend, "未识别后端技术栈")
}

fn project_commands(root: &Path, layer: &str) -> String {
    let mut rows = Vec::new();
    if layer == "前端" && has_project_file(root, "package.json") {
        rows.push("| 安装依赖 | `npm install` | `package.json` |");
        rows.push("| 本地启动 | `npm run dev` | `package.json` scripts |");
        rows.push("| 单元测试 | `npm test` | `package.json` scripts |");
        rows.push("| 生产构建 | `npm run build` | `package.json` scripts |");
    }
    if layer == "后端" && has_project_file(root, "pom.xml") {
        rows.push("| 单元测试 | `mvn test` | `pom.xml` |");
        rows.push("| 构建 | `mvn clean package` | `pom.xml` |");
        rows.push("| 本地启动 | `mvn spring-boot:run` | `pom.xml` |");
    } else if layer == "后端" && has_project_file(root, "pyproject.toml") {
        rows.push("| 安装依赖 | `python -m pip install -e .` | `pyproject.toml` |");
        rows.push("| 单元测试 | `pytest` | `pyproject.toml` |");
        rows.push("| 本地启动 | `uvicorn app.main:app --reload` | `pyproject.toml` 与源码入口 |");
    } else if layer == "后端" && has_project_file(root, "go.mod") {
        rows.push("| 单元测试 | `go test ./...` | `go.mod` |");
        rows.push("| 构建 | `go build ./...` | `go.mod` |");
        rows.push("| 本地启动 | `go run .` | `go.mod` 与源码入口 |");
    } else if layer == "后端" && has_project_file(root, "Cargo.toml") {
        rows.push("| 单元测试 | `cargo test` | `Cargo.toml` |");
        rows.push("| 构建 | `cargo build` | `Cargo.toml` |");
        rows.push("| 本地启动 | `cargo run` | `Cargo.toml` |");
    } else if layer == "后端" && has_project_file(root, "Program.cs") {
        rows.push("| 单元测试 | `dotnet test` | `.csproj` 与测试工程 |");
        rows.push("| 构建 | `dotnet build` | `.csproj` |");
        rows.push("| 本地启动 | `dotnet run` | `.csproj` 与 `Program.cs` |");
    } else if layer == "后端" && has_project_file(root, "app.module.ts") {
        rows.push("| 安装依赖 | `npm install` | `package.json` |");
        rows.push("| 单元测试 | `npm test` | `package.json` scripts |");
        rows.push("| 构建 | `npm run build` | `package.json` scripts |");
        rows.push("| 本地启动 | `npm run start:dev` | `package.json` scripts |");
    }
    if rows.is_empty() {
        "| 待确认 | 当前工程未识别到可证明的命令 | 构建清单 |".to_string()
    } else {
        rows.join("\n")
    }
}

fn doc_header(title: &str, request: &CreateProjectRequest) -> String {
    format!(
        "# {title}\n\n> 项目：`{}`  \n> 系统类型：{}  \n> 文档依据：用户确认的项目方案与当前源码。\n",
        request.project_name, request.profile.system_type
    )
}

fn frontend_moc(request: &CreateProjectRequest) -> String {
    format!(
        "# {} 前端文档导航\n\n- [项目需求与技术选型](../项目需求与技术选型.md)\n\n## 长期文档\n\n| 文档 | 用途 |\n|---|---|\n| [项目总览](latest/index.md) | 项目定位、技术栈、命令和文档入口 |\n| [业务功能总览](latest/业务/业务功能总览.md) | 需求范围、已实现页面与功能边界 |\n| [前端架构](latest/系统架构/前端架构.md) | 目录、入口、状态、请求与组件边界 |\n| [组件与公共能力](latest/公共能力/组件与公共能力.md) | 当前可复用能力索引 |\n| [变更记录](latest/变更记录.md) | 后续真实交付记录 |\n\n## 后续迭代模板\n\n- [详设文档模板](latest/规范约束/详设文档模板.md)\n- [开发进度文档模板](latest/规范约束/开发进度文档模板.md)\n\n真实需求开始后才创建 `v{{版本}}/详细设计` 与 `v{{版本}}/开发进度`；项目创建不伪造初始化需求和开发记录。\n",
        request.project_name
    )
}

fn backend_moc(
    request: &CreateProjectRequest,
    has_frontend: bool,
    has_physical_model: bool,
) -> String {
    let integration = if has_frontend {
        "- [前端接入说明模板](latest/规范约束/前端接入说明模板.md)\n"
    } else {
        ""
    };
    let physical_model = if has_physical_model {
        "| [物理模型总览](latest/接口文档/物理模型总览.md) | 当前真实表与字段索引；当前没有业务表时明确记录空状态 |\n"
    } else {
        ""
    };
    format!(
        "# {} 后端文档导航\n\n- [项目需求与技术选型](../项目需求与技术选型.md)\n\n## 长期文档\n\n| 文档 | 用途 |\n|---|---|\n| [项目总览](latest/index.md) | 项目定位、模块、技术栈、命令与入口 |\n| [业务功能总览](latest/业务/业务功能总览.md) | 需求范围、已实现能力与业务边界 |\n| [系统架构详解](latest/系统架构/系统架构详解.md) | 模块职责、依赖和公共能力 |\n| [API 接口总览](latest/接口文档/API接口总览.md) | 当前源码真实存在的 HTTP API |\n{physical_model}\n## 后续迭代模板\n\n- [详设文档模板](latest/规范约束/详设文档模板.md)\n- [开发进度文档模板](latest/规范约束/开发进度文档模板.md)\n{integration}\n真实需求开始后才创建 `v{{版本}}/详细设计` 与 `v{{版本}}/开发进度`；项目创建不伪造初始化需求和开发记录。\n",
        request.project_name
    )
}

fn index_document(root: &Path, request: &CreateProjectRequest, layer: &str) -> String {
    let stack = if layer == "前端" {
        frontend_stack(request)
    } else {
        backend_stack(request)
    };
    let evidence = if layer == "前端" {
        evidence_or_none(root, &["package.json", "App.vue", "App.tsx", "page.tsx"])
    } else {
        evidence_or_none(
            root,
            &[
                "pom.xml",
                "pyproject.toml",
                "go.mod",
                "Cargo.toml",
                "Program.cs",
                "app.module.ts",
            ],
        )
    };
    let commands = project_commands(root, layer);
    format!(
        "{}\n## 项目定位\n\n{}\n\n> 完整需求、约束和选型理由见 [项目需求与技术选型](../../项目需求与技术选型.md)。\n\n## 技术栈\n\n- {}：{}\n- 工程结构：{}\n\n## 代码依据\n\n{}\n\n## 常用开发命令\n\n| 用途 | 命令 | 证据 |\n|---|---|---|\n{}\n\n## 文档使用约定\n\n- `latest/` 保存当前长期真源；功能变化后同步更新。\n- 具体需求确认后，按 `v{{版本}}/详细设计` 与 `v{{版本}}/开发进度` 成对归档。\n- 未被源码、配置、测试或用户确认材料证明的内容不得写成事实。\n- 项目创建只准备工程与工作流，不代表业务功能已经开发。\n",
        doc_header(&format!("{} {}项目文档索引", request.project_name, layer), request),
        request.profile.summary,
        layer,
        stack,
        request.recommendation.structure,
        evidence,
        commands
    )
}

fn business_overview(root: &Path, request: &CreateProjectRequest, layer: &str) -> String {
    let evidence = if layer == "前端" {
        evidence_or_none(root, &["App.vue", "App.tsx", "page.tsx"])
    } else {
        evidence_or_none(
            root,
            &[
                "HealthController.java",
                "main.py",
                "main.go",
                "main.rs",
                "Program.cs",
                "app.module.ts",
            ],
        )
    };
    let requirement = if request.concise_requirement.trim().is_empty() {
        request.profile.summary.trim()
    } else {
        request.concise_requirement.trim()
    };
    let implemented = if layer == "后端" {
        format!(
            "| 基础健康检查 | 开发/监控调用方 | 已实现 | `GET /api/health` | {} |",
            evidence
        )
    } else {
        format!(
            "| 应用启动页 | 最终用户 | 已实现基础骨架 | 应用根入口 | {} |",
            evidence
        )
    };
    format!(
        "{}\n## 已确认需求范围\n\n{}\n\n> 需求已确认不等于业务已经实现。后续必须先完成详设和用户确认，再进入开发。\n\n## 当前实现清单\n\n| 功能 | 用户/调用方 | 当前状态 | 入口 | 代码证据 |\n|---|---|---|---|---|\n{}\n\n## 待设计与开发\n\n| 范围 | 状态 | 下一步 |\n|---|---|---|\n| 已确认业务需求 | 待详细设计 | 使用 `latest/规范约束/详设文档模板.md` 创建版本详设，用户确认后再开发 |\n\n## 当前边界\n\n- 本次只生成可启动的{}工程、项目文档、规则与 skills。\n- 未在代码中出现的业务能力、接口、数据模型和第三方集成均未实现。\n- 不根据项目名称或技术选型推测业务功能。\n\n## 维护规则\n\n新增、修改或下线业务功能时必须同步本表，并引用真实入口、代码与测试。\n",
        doc_header(&format!("{} {}业务功能总览", request.project_name, layer), request),
        requirement,
        implemented,
        layer
    )
}

fn frontend_architecture(root: &Path, request: &CreateProjectRequest) -> String {
    format!(
        "{}\n## 技术与入口\n\n- 技术栈：{}\n- 应用入口：{}\n- 包管理器：{}\n\n## 目录职责\n\n| 目录/文件 | 职责 | 证据 |\n|---|---|---|\n| `src/` | 前端业务源码 | 当前工程目录 |\n| `package.json` | 依赖与脚本真源 | `package.json` |\n\n## 数据与交互\n\n当前生成工程未发现可确认的业务 API、状态模块或权限链路；后续以代码为准补充，不预设框架。\n\n## 架构约束\n\n1. 新增能力前检索已有组件、组合式函数、状态和请求封装。\n2. 页面只编排交互，复用逻辑进入项目既有公共层。\n3. 加载、空、成功、失败和权限状态必须有明确用户体验。\n4. 不为了模板完整度引入项目尚未使用的依赖。\n",
        doc_header(&format!("{} 前端架构", request.project_name), request),
        frontend_stack(request),
        evidence_or_none(root, &["main.ts", "main.tsx", "App.vue", "App.tsx", "page.tsx"]),
        "以 `package.json` 与锁文件为准"
    )
}

fn shared_frontend_capabilities(root: &Path, request: &CreateProjectRequest) -> String {
    format!(
        "{}\n## 公共能力清单\n\n| 能力 | 位置 | 使用场景 | 状态 |\n|---|---|---|---|\n| 应用根组件 | {} | 页面入口与基础展示 | 已生成 |\n\n## 复用约定\n\n- 新建组件、工具、状态或请求封装前先检索 `src/`。\n- 只有两个以上明确调用方且职责稳定时才抽取公共能力。\n- 本文只登记已经存在并可引用的能力，不记录设想。\n",
        doc_header(&format!("{} 组件与公共能力", request.project_name), request),
        evidence_or_none(root, &["App.vue", "App.tsx", "page.tsx"])
    )
}

fn backend_architecture(root: &Path, request: &CreateProjectRequest) -> String {
    format!(
        "{}\n## 系统定位\n\n{}\n\n## 技术栈与入口\n\n- 技术栈：{}\n- 工程入口：{}\n- 构建清单：{}\n\n## 当前调用链\n\n```text\nHTTP 调用方 → 健康检查入口 → 应用响应\n```\n\n## 模块职责\n\n| 模块/目录 | 职责 | 依赖方向 |\n|---|---|---|\n| 应用源码 | 接收请求并返回健康状态 | 以当前生成骨架为准 |\n| 构建清单 | 管理运行时和依赖 | 不承载业务逻辑 |\n\n## 公共能力与扩展边界\n\n当前仅确认健康检查骨架。新增业务前必须先扫描项目内已有路由、Service、Repository、错误处理、日志和测试基座；不存在时才按当前技术栈惯例新增。\n",
        doc_header(&format!("{} 系统架构详解", request.project_name), request),
        request.profile.summary,
        backend_stack(request),
        evidence_or_none(root, &["HealthController.java", "main.py", "main.go", "main.rs", "Program.cs", "app.module.ts"]),
        evidence_or_none(root, &["pom.xml", "pyproject.toml", "go.mod", "Cargo.toml", "Program.cs", "app.module.ts"])
    )
}

fn api_overview(root: &Path, request: &CreateProjectRequest) -> String {
    let source = evidence_or_none(
        root,
        &[
            "HealthController.java",
            "main.py",
            "main.go",
            "main.rs",
            "Program.cs",
            "app.module.ts",
        ],
    );
    format!(
        "{}\n## 全局约定\n\n- Base URL：由本地启动端口决定。\n- 鉴权：当前健康检查接口未配置鉴权。\n- 响应：JSON。\n\n## 接口索引\n\n| 模块 | 方法 | 路径 | 调用方 | 权限 | 响应 | 状态 |\n|---|---|---|---|---|---|---|\n| 基础能力 | GET | `/api/health` | 开发/健康检查 | 无 | `status`、`application` | 已实现 |\n\n## 实现证据\n\n{}\n\n## 维护规则\n\n新增、修改或下线接口时同步方法、路径、鉴权、请求/响应、错误和兼容策略；不得从命名推测接口。\n",
        doc_header(&format!("{} API 接口总览", request.project_name), request),
        source
    )
}

fn physical_model_overview(request: &CreateProjectRequest) -> String {
    let databases = request
        .recommendation
        .decisions
        .iter()
        .filter(|decision| decision.category == "persistence" && decision.status == "adopt")
        .flat_map(|decision| decision.choices.clone())
        .collect::<Vec<_>>();
    format!(
        "{}\n## 数据库选型\n\n{}\n\n## 当前表清单\n\n当前工程尚未实现业务表、实体模型或数据库迁移脚本，因此没有可以登记的物理表。不得根据需求名称或技术选型编造表名和字段。\n\n| 表名 | 中文名称 | 所属模块 | 主要用途 | 定义证据 |\n|---|---|---|---|---|\n| — | 当前无业务表 | — | — | 项目内未生成实体/schema/迁移脚本 |\n\n## 维护规则\n\n后续业务详设确认并产生真实实体或迁移脚本后再补充表清单与逐表字段；每张表只记录表名、中文名称、所属模块、主要用途，以及字段名、类型、可空性、中文含义和约束。\n",
        doc_header(&format!("{} 物理模型总览", request.project_name), request),
        bullet_list(&databases, "当前未采用关系型数据库")
    )
}

fn change_log(request: &CreateProjectRequest) -> String {
    format!(
        "{}\n| 版本/阶段 | 变更 | 用户可见影响 | 证据 |\n|---|---|---|---|\n| 项目创建 | 生成{}基础工程与项目工作流 | 仅具备基础启动页，业务需求尚未开发 | 当前代码与 `docs/项目需求与技术选型.md` |\n\n后续只记录已经开发并完成验证的真实功能变化；不得把技术选型或待开发需求写成已交付。\n",
        doc_header(&format!("{} 变更记录", request.project_name), request),
        request.profile.system_type
    )
}

fn write_frontend_docs(
    docs: &Path,
    root: &Path,
    request: &CreateProjectRequest,
) -> Result<(), String> {
    let base = docs.join("frontend");
    write_file(&base.join("MOC.md"), &frontend_moc(request))?;
    write_file(
        &base.join("latest/index.md"),
        &index_document(root, request, "前端"),
    )?;
    write_file(
        &base.join("latest/业务/业务功能总览.md"),
        &business_overview(root, request, "前端"),
    )?;
    write_file(
        &base.join("latest/系统架构/前端架构.md"),
        &frontend_architecture(root, request),
    )?;
    write_file(
        &base.join("latest/公共能力/组件与公共能力.md"),
        &shared_frontend_capabilities(root, request),
    )?;
    write_file(&base.join("latest/变更记录.md"), &change_log(request))?;
    write_file(
        &base.join("latest/规范约束/详设文档模板.md"),
        DETAIL_DESIGN_TEMPLATE,
    )?;
    write_file(
        &base.join("latest/规范约束/开发进度文档模板.md"),
        PROGRESS_TEMPLATE,
    )?;
    Ok(())
}

fn write_backend_docs(
    docs: &Path,
    root: &Path,
    request: &CreateProjectRequest,
    has_frontend: bool,
) -> Result<(), String> {
    let base = docs.join("backend");
    let has_physical_model = has_adopted_relational_database(request);
    write_file(
        &base.join("MOC.md"),
        &backend_moc(request, has_frontend, has_physical_model),
    )?;
    write_file(
        &base.join("latest/index.md"),
        &index_document(root, request, "后端"),
    )?;
    write_file(
        &base.join("latest/业务/业务功能总览.md"),
        &business_overview(root, request, "后端"),
    )?;
    write_file(
        &base.join("latest/系统架构/系统架构详解.md"),
        &backend_architecture(root, request),
    )?;
    write_file(
        &base.join("latest/接口文档/API接口总览.md"),
        &api_overview(root, request),
    )?;
    if has_physical_model {
        write_file(
            &base.join("latest/接口文档/物理模型总览.md"),
            &physical_model_overview(request),
        )?;
    }
    write_file(
        &base.join("latest/规范约束/详设文档模板.md"),
        DETAIL_DESIGN_TEMPLATE,
    )?;
    write_file(
        &base.join("latest/规范约束/开发进度文档模板.md"),
        PROGRESS_TEMPLATE,
    )?;
    if has_frontend {
        write_file(
            &base.join("latest/规范约束/前端接入说明模板.md"),
            FRONTEND_INTEGRATION_TEMPLATE,
        )?;
    }
    Ok(())
}

pub fn write_project_docs(root: &Path, request: &CreateProjectRequest) -> Result<(), String> {
    let docs = root.join("docs");
    let layers = project_layers(root);
    write_file(
        &docs.join("项目需求与技术选型.md"),
        &requirement_and_stack_document(request, layers),
    )?;
    write_file(
        &docs.join("README.md"),
        &format!(
            "# {} 项目文档\n\n- [项目需求与技术选型](项目需求与技术选型.md)\n{}{}\n文档只记录当前代码或用户确认材料能够证明的事实。长期内容在各代码层的 `latest/`；具体业务需求确认后，才在 `v{{版本}}/` 下创建成对的详设与进度文档。\n",
            request.project_name,
            if layers.frontend { "- [前端文档](frontend/MOC.md)\n" } else { "" },
            if layers.backend { "- [后端文档](backend/MOC.md)\n" } else { "" }
        ),
    )?;
    if layers.frontend {
        write_frontend_docs(&docs, root, request)?;
    }
    if layers.backend {
        write_backend_docs(&docs, root, request, layers.frontend)?;
    }
    Ok(())
}

#[cfg(test)]
mod layer_detection_tests {
    use super::*;

    fn fixture(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("vibe-layer-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("create layer fixture");
        root
    }

    #[test]
    fn detects_gradle_spring_and_requirements_fastapi_as_backends() {
        let gradle = fixture("gradle-spring");
        fs::write(
            gradle.join("build.gradle.kts"),
            "plugins { id(\"org.springframework.boot\") version \"3.2.0\" }",
        )
        .expect("write gradle fixture");
        assert_eq!(
            project_layers(&gradle),
            ProjectLayers {
                frontend: false,
                backend: true,
            }
        );

        let fastapi = fixture("requirements-fastapi");
        fs::write(fastapi.join("requirements.txt"), "fastapi==0.111\nuvicorn")
            .expect("write requirements fixture");
        assert_eq!(
            project_layers(&fastapi),
            ProjectLayers {
                frontend: false,
                backend: true,
            }
        );

        fs::remove_dir_all(gradle).expect("cleanup gradle fixture");
        fs::remove_dir_all(fastapi).expect("cleanup fastapi fixture");
    }

    #[test]
    fn does_not_treat_a_rust_cli_as_a_backend_service() {
        let root = fixture("rust-cli");
        fs::write(
            root.join("Cargo.toml"),
            "[package]\nname = \"demo-cli\"\nversion = \"0.1.0\"\n",
        )
        .expect("write cargo fixture");
        fs::create_dir_all(root.join("src")).expect("create rust src");
        fs::write(
            root.join("src/main.rs"),
            "fn main() { println!(\"hello\"); }",
        )
        .expect("write rust main");
        assert_eq!(
            project_layers(&root),
            ProjectLayers {
                frontend: false,
                backend: false,
            }
        );
        fs::remove_dir_all(root).expect("cleanup rust fixture");
    }

    #[test]
    fn detects_nuxt_and_astro_as_frontends_without_inventing_a_backend() {
        for (name, dependency) in [("nuxt", "nuxt"), ("astro", "astro")] {
            let root = fixture(name);
            fs::write(
                root.join("package.json"),
                format!("{{\"dependencies\":{{\"{dependency}\":\"latest\"}}}}"),
            )
            .expect("write package fixture");
            assert_eq!(
                project_layers(&root),
                ProjectLayers {
                    frontend: true,
                    backend: false,
                }
            );
            fs::remove_dir_all(root).expect("cleanup frontend fixture");
        }
    }
}
