use std::fs;
use std::path::Path;

use super::ai_rules::{validate_skill_designer, write_skill_designer};
use super::docs::{project_file_contents, project_files_named, project_layers, ProjectLayers};
use super::types::{
    ExistingProjectInitPreparation, ExistingProjectInitResult, ExistingProjectInitStatus,
};

const PLATFORM_INIT_MARKER: &str = "<!-- vibe-coding-platform:init:v3 -->";
const DETAIL_DESIGN_TEMPLATE: &str =
    include_str!("../../../docs/规范约束/文档模板/公共/详设文档模板.md");
const PROGRESS_TEMPLATE: &str =
    include_str!("../../../docs/规范约束/文档模板/公共/开发进度文档模板.md");
const FRONTEND_INTEGRATION_TEMPLATE: &str =
    include_str!("../../../docs/规范约束/文档模板/公共/前端接入说明模板.md");
const INIT_REFERENCE_DIR: &str = ".vibe-coding-platform/init-reference-v3";

/// 初始化 Agent 的只读参考包。正式长期文档不能直接复制空模板，因此参考包只在初始化期间
/// 存在；最终真实产物校验通过后立即删除。
const INIT_REFERENCE_FILES: &[(&str, &str)] = &[
    (
        "文档模板/公共/Agent入口文档模板.md",
        include_str!("../../../docs/规范约束/文档模板/公共/Agent入口文档模板.md"),
    ),
    (
        "文档模板/公共/前端接入说明模板.md",
        FRONTEND_INTEGRATION_TEMPLATE,
    ),
    ("文档模板/公共/开发进度文档模板.md", PROGRESS_TEMPLATE),
    ("文档模板/公共/详设文档模板.md", DETAIL_DESIGN_TEMPLATE),
    (
        "文档模板/前端/MOC模板.md",
        include_str!("../../../docs/规范约束/文档模板/前端/MOC模板.md"),
    ),
    (
        "文档模板/前端/index模板.md",
        include_str!("../../../docs/规范约束/文档模板/前端/index模板.md"),
    ),
    (
        "文档模板/前端/业务功能总览模板.md",
        include_str!("../../../docs/规范约束/文档模板/前端/业务功能总览模板.md"),
    ),
    (
        "文档模板/前端/前端架构模板.md",
        include_str!("../../../docs/规范约束/文档模板/前端/前端架构模板.md"),
    ),
    (
        "文档模板/前端/变更记录模板.md",
        include_str!("../../../docs/规范约束/文档模板/前端/变更记录模板.md"),
    ),
    (
        "文档模板/前端/组件与公共能力模板.md",
        include_str!("../../../docs/规范约束/文档模板/前端/组件与公共能力模板.md"),
    ),
    (
        "文档模板/后端/API接口总览模板.md",
        include_str!("../../../docs/规范约束/文档模板/后端/API接口总览模板.md"),
    ),
    (
        "文档模板/后端/MOC模板.md",
        include_str!("../../../docs/规范约束/文档模板/后端/MOC模板.md"),
    ),
    (
        "文档模板/后端/index模板.md",
        include_str!("../../../docs/规范约束/文档模板/后端/index模板.md"),
    ),
    (
        "文档模板/后端/业务功能总览模板.md",
        include_str!("../../../docs/规范约束/文档模板/后端/业务功能总览模板.md"),
    ),
    (
        "文档模板/后端/回调接口总览模板.md",
        include_str!("../../../docs/规范约束/文档模板/后端/回调接口总览模板.md"),
    ),
    (
        "文档模板/后端/枚举值总览模板.md",
        include_str!("../../../docs/规范约束/文档模板/后端/枚举值总览模板.md"),
    ),
    (
        "文档模板/后端/物理模型总览模板.md",
        include_str!("../../../docs/规范约束/文档模板/后端/物理模型总览模板.md"),
    ),
    (
        "文档模板/后端/第三方集成模板.md",
        include_str!("../../../docs/规范约束/文档模板/后端/第三方集成模板.md"),
    ),
    (
        "文档模板/后端/系统架构详解模板.md",
        include_str!("../../../docs/规范约束/文档模板/后端/系统架构详解模板.md"),
    ),
    (
        "规则模板/公共/Git协作与历史保护.md",
        include_str!("../../../docs/规范约束/规则模板/公共/Git协作与历史保护.md"),
    ),
    (
        "规则模板/公共/事实与兜底边界.md",
        include_str!("../../../docs/规范约束/规则模板/公共/事实与兜底边界.md"),
    ),
    (
        "规则模板/公共/复用与影响面.md",
        include_str!("../../../docs/规范约束/规则模板/公共/复用与影响面.md"),
    ),
    (
        "规则模板/公共/开发基线.md",
        include_str!("../../../docs/规范约束/规则模板/公共/开发基线.md"),
    ),
    (
        "规则模板/公共/开发流程与文档同步.md",
        include_str!("../../../docs/规范约束/规则模板/公共/开发流程与文档同步.md"),
    ),
    (
        "规则模板/公共/自测与交付.md",
        include_str!("../../../docs/规范约束/规则模板/公共/自测与交付.md"),
    ),
    (
        "规则模板/前端/前端工程规则.md",
        include_str!("../../../docs/规范约束/规则模板/前端/前端工程规则.md"),
    ),
    (
        "规则模板/前端/前端验证规则.md",
        include_str!("../../../docs/规范约束/规则模板/前端/前端验证规则.md"),
    ),
    (
        "规则模板/后端/API与业务实现规则.md",
        include_str!("../../../docs/规范约束/规则模板/后端/API与业务实现规则.md"),
    ),
    (
        "规则模板/后端/异步与第三方规则.md",
        include_str!("../../../docs/规范约束/规则模板/后端/异步与第三方规则.md"),
    ),
    (
        "规则模板/后端/持久化与迁移规则.md",
        include_str!("../../../docs/规范约束/规则模板/后端/持久化与迁移规则.md"),
    ),
    (
        "技能模板/公共/code-review/SKILL.md",
        include_str!("../../../docs/规范约束/技能模板/公共/code-review/SKILL.md"),
    ),
    (
        "技能模板/公共/detail-design-writer/SKILL.md",
        include_str!("../../../docs/规范约束/技能模板/公共/detail-design-writer/SKILL.md"),
    ),
    (
        "技能模板/公共/developer/SKILL.md",
        include_str!("../../../docs/规范约束/技能模板/公共/developer/SKILL.md"),
    ),
    (
        "技能模板/公共/problem-diagnose/SKILL.md",
        include_str!("../../../docs/规范约束/技能模板/公共/problem-diagnose/SKILL.md"),
    ),
    (
        "技能模板/公共/review-feedback-handler/SKILL.md",
        include_str!("../../../docs/规范约束/技能模板/公共/review-feedback-handler/SKILL.md"),
    ),
    (
        "技能模板/前端/frontend-self-test/SKILL.md",
        include_str!("../../../docs/规范约束/技能模板/前端/frontend-self-test/SKILL.md"),
    ),
    (
        "技能模板/后端/backend-self-test/SKILL.md",
        include_str!("../../../docs/规范约束/技能模板/后端/backend-self-test/SKILL.md"),
    ),
    (
        "技能模板/可选/backend-log-diagnose/SKILL.md",
        include_str!("../../../docs/规范约束/技能模板/可选/backend-log-diagnose/SKILL.md"),
    ),
    (
        "技能模板/可选/database-read-diagnose/SKILL.md",
        include_str!("../../../docs/规范约束/技能模板/可选/database-read-diagnose/SKILL.md"),
    ),
    (
        "技能模板/可选/ddl-review/SKILL.md",
        include_str!("../../../docs/规范约束/技能模板/可选/ddl-review/SKILL.md"),
    ),
    (
        "技能模板/可选/external-integration/SKILL.md",
        include_str!("../../../docs/规范约束/技能模板/可选/external-integration/SKILL.md"),
    ),
];

fn contains_any(source: &str, candidates: &[&str]) -> bool {
    candidates
        .iter()
        .any(|candidate| source.contains(candidate))
}

fn detected_stack(root: &Path, layers: ProjectLayers) -> Vec<String> {
    let package = project_file_contents(root, "package.json");
    let python = format!(
        "{}\n{}",
        project_file_contents(root, "pyproject.toml"),
        project_file_contents(root, "requirements.txt")
    );
    let pom = project_file_contents(root, "pom.xml");
    let gradle = format!(
        "{}\n{}",
        project_file_contents(root, "build.gradle"),
        project_file_contents(root, "build.gradle.kts")
    );
    let cargo = project_file_contents(root, "Cargo.toml");
    let go_mod = project_file_contents(root, "go.mod");
    let ruby = project_file_contents(root, "Gemfile");
    let php = project_file_contents(root, "composer.json");
    let scala = project_file_contents(root, "build.sbt");
    let mut stack = Vec::new();

    if layers.frontend {
        for (needle, label) in [
            ("\"vue\"", "Vue"),
            ("\"react\"", "React"),
            ("\"next\"", "Next.js"),
            ("\"svelte\"", "Svelte"),
            ("\"nuxt\"", "Nuxt"),
            ("\"astro\"", "Astro"),
            ("\"@angular/core\"", "Angular"),
            ("typescript", "TypeScript"),
            ("vite", "Vite"),
        ] {
            if package.contains(needle) {
                stack.push(label.to_string());
            }
        }
        if !project_files_named(root, "tauri.conf.json").is_empty() || cargo.contains("tauri") {
            stack.push("Tauri".to_string());
        }
    }
    if layers.backend {
        if pom.contains("spring-boot") || gradle.contains("org.springframework.boot") {
            stack.push("Spring Boot".to_string());
        }
        if !project_files_named(root, "pom.xml").is_empty() || !gradle.is_empty() {
            stack.push("Java".to_string());
        }
        if gradle.contains("kotlin") {
            stack.push("Kotlin".to_string());
        }
        if python.contains("fastapi") {
            stack.push("FastAPI".to_string());
        } else if python.contains("django") {
            stack.push("Django".to_string());
        } else if python.contains("flask") {
            stack.push("Flask".to_string());
        }
        if !python.trim().is_empty() || !project_files_named(root, "manage.py").is_empty() {
            stack.push("Python".to_string());
        }
        if !go_mod.is_empty() {
            stack.push("Go".to_string());
        }
        if !cargo.is_empty() {
            stack.push("Rust".to_string());
        }
        if contains_any(
            &package,
            &["nestjs", "@nestjs", "express", "fastify", "koa"],
        ) {
            stack.push("Node.js".to_string());
        }
        if !project_files_named(root, "Program.cs").is_empty() {
            stack.push(".NET".to_string());
        }
        if !ruby.is_empty() {
            stack.push(
                if ruby.contains("rails") {
                    "Ruby on Rails"
                } else {
                    "Ruby"
                }
                .to_string(),
            );
        }
        if !php.is_empty() {
            stack.push(
                if php.contains("laravel/framework") {
                    "Laravel"
                } else {
                    "PHP"
                }
                .to_string(),
            );
        }
        if !scala.is_empty() {
            stack.push("Scala".to_string());
        }
    }
    stack.sort();
    stack.dedup();
    if stack.is_empty() {
        stack.push("待 Agent 根据项目文件补充识别".to_string());
    }
    stack
}

fn has_database_dependency(root: &Path) -> bool {
    let source = [
        "package.json",
        "pyproject.toml",
        "requirements.txt",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "Cargo.toml",
        "go.mod",
        "Gemfile",
        "composer.json",
        "build.sbt",
    ]
    .iter()
    .map(|name| project_file_contents(root, name))
    .collect::<Vec<_>>()
    .join("\n");
    contains_any(
        &source,
        &[
            "mysql",
            "postgres",
            "postgresql",
            "sqlite",
            "mongodb",
            "prisma",
            "typeorm",
            "sqlalchemy",
            "sqlx",
            "jooq",
            "mybatis",
            "jpa",
        ],
    ) || has_database_connection_evidence(root)
        || has_database_model_evidence(root)
}

fn should_scan_evidence_dir(name: &str) -> bool {
    !matches!(
        name,
        ".git"
            | ".claude"
            | ".agents"
            | "docs"
            | "node_modules"
            | "target"
            | "dist"
            | "build"
            | "vendor"
    )
}

fn is_text_evidence_file(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .map(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "java"
                    | "kt"
                    | "kts"
                    | "rs"
                    | "go"
                    | "py"
                    | "ts"
                    | "tsx"
                    | "js"
                    | "jsx"
                    | "cs"
                    | "sql"
                    | "xml"
                    | "yaml"
                    | "yml"
                    | "toml"
                    | "json"
                    | "conf"
                    | "env"
                    | "properties"
                    | "prisma"
            )
        })
        .unwrap_or(false)
}

fn any_project_source_file(root: &Path, mut predicate: impl FnMut(&Path, &str) -> bool) -> bool {
    fn visit(path: &Path, depth: usize, predicate: &mut impl FnMut(&Path, &str) -> bool) -> bool {
        if depth > 8 {
            return false;
        }
        let Ok(entries) = fs::read_dir(path) else {
            return false;
        };
        for entry in entries.flatten() {
            let child = entry.path();
            if child.is_dir() {
                if should_scan_evidence_dir(&entry.file_name().to_string_lossy())
                    && visit(&child, depth + 1, predicate)
                {
                    return true;
                }
                continue;
            }
            if !is_text_evidence_file(&child) {
                continue;
            }
            let Ok(metadata) = child.metadata() else {
                continue;
            };
            if metadata.len() > 2 * 1024 * 1024 {
                continue;
            }
            let Ok(content) = fs::read_to_string(&child) else {
                continue;
            };
            let normalized = content.to_ascii_lowercase();
            if predicate(&child, &normalized) {
                return true;
            }
        }
        false
    }
    visit(root, 0, &mut predicate)
}

fn project_source_contains(root: &Path, candidates: &[&str]) -> bool {
    any_project_source_file(root, |_, source| {
        candidates
            .iter()
            .any(|candidate| source.contains(candidate))
    })
}

fn source_has_api_evidence(source: &str) -> bool {
    contains_any(
        source,
        &[
            "@restcontroller",
            "@requestmapping",
            "@getmapping",
            "@postmapping",
            "@putmapping",
            "@deletemapping",
            "@patchmapping",
            "fastapi(",
            "apirouter(",
            "@app.get(",
            "@app.post(",
            "@app.put(",
            "@app.delete(",
            "@app.patch(",
            "@router.get(",
            "@router.post(",
            "@router.put(",
            "@router.delete(",
            "@router.patch(",
            "app.get(",
            "app.post(",
            "app.put(",
            "app.delete(",
            "app.patch(",
            "router.get(",
            "router.post(",
            "router.put(",
            "router.delete(",
            "router.patch(",
            "axum::router",
            "actix_web::",
            "#[get(\"/",
            "#[post(\"/",
            "http.handlefunc(",
            ".handlefunc(",
            "gin.default(",
            "echo.new(",
            "mapget(",
            "mappost(",
            "mapput(",
            "mapdelete(",
            "[apicontroller]",
        ],
    )
}

fn has_api_evidence(root: &Path) -> bool {
    any_project_source_file(root, |_, source| source_has_api_evidence(source))
}

fn has_callback_evidence(root: &Path) -> bool {
    any_project_source_file(root, |_, source| {
        source_has_api_evidence(source)
            && contains_any(source, &["/callback", "/webhook", "/notify"])
    })
}

fn has_boundary_enum_evidence(root: &Path) -> bool {
    any_project_source_file(root, |path, source| {
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        match extension.as_str() {
            "java" | "kt" | "kts" | "cs" => contains_any(
                source,
                &["public enum ", "enum class ", "sealed interface "],
            ),
            "ts" | "tsx" => contains_any(source, &["export enum ", " enum "]),
            "rs" => contains_any(source, &["pub enum ", "#[serde", "#[repr("]),
            "py" => contains_any(source, &["class ", "(enum):", "(str, enum):"]),
            _ => false,
        }
    })
}

fn has_external_integration_evidence(root: &Path) -> bool {
    any_project_source_file(root, |_, source| {
        contains_any(
            source,
            &[
                "@feignclient",
                "stripe-java",
                "stripe-python",
                "paypal-sdk",
                "wechatpay",
                "alipay-sdk",
                "yop-java-sdk",
                "twilio",
                "sendgrid",
                "aws-sdk",
                "software.amazon.awssdk",
            ],
        ) || (contains_any(
            source,
            &[
                "webclient.builder",
                "resttemplate",
                "okhttpclient",
                "axios.create",
                "httpx.client",
                "requests.session",
                "reqwest::client",
            ],
        ) && contains_any(
            source,
            &["https://", "http://", "baseurl", "base_url", "external"],
        ))
    })
}

fn has_database_connection_evidence(root: &Path) -> bool {
    project_source_contains(
        root,
        &[
            "spring.datasource",
            "datasource.url",
            "jdbc:",
            "database_url",
            "db_host",
            "db_url",
            "mongodb.uri",
            "mongodb://",
            "mongodb+srv://",
            "postgres://",
            "postgresql://",
            "mysql://",
            "sqlite://",
        ],
    )
}

fn has_database_model_evidence(root: &Path) -> bool {
    !project_files_named(root, "schema.prisma").is_empty()
        || project_source_contains(
            root,
            &[
                "create table ",
                "create table\n",
                "createtable",
                "create_table ",
                "@entity",
                "@table(",
                "@tablename(",
                "__tablename__",
                "models.model",
                ".define(",
                "dbset<",
                "sqlx::fromrow",
                "gorm.model",
                "diesel::table!",
            ],
        )
}

fn list_existing(root: &Path, relative: &str) -> Vec<String> {
    root.join(relative)
        .exists()
        .then(|| relative.to_string())
        .into_iter()
        .collect()
}

fn write_if_missing(path: &Path, content: &str) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, content).map_err(|error| error.to_string())
}

fn write_initialization_reference_bundle(root: &Path) -> Result<(), String> {
    let base = root.join(INIT_REFERENCE_DIR);
    if base.exists() {
        fs::remove_dir_all(&base).map_err(|error| error.to_string())?;
    }
    for (relative, content) in INIT_REFERENCE_FILES {
        let path = base.join(relative);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        fs::write(path, content).map_err(|error| error.to_string())?;
    }
    fs::write(
        base.join("README.md"),
        "# 初始化只读参考包\n\n本目录由平台临时生成。后台 Agent 必须先逐份读取这里命中当前代码层的文档、规则和 skill 模板，再依据目标项目真实代码填充正式产物。禁止把模板占位符或空表复制进正式长期文档。最终校验成功后平台会自动删除本目录。\n",
    )
    .map_err(|error| error.to_string())
}

fn remove_initialization_reference_bundle(root: &Path) -> Result<(), String> {
    let base = root.join(INIT_REFERENCE_DIR);
    if base.exists() {
        fs::remove_dir_all(base).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn install_project_document_templates(root: &Path, layers: ProjectLayers) -> Result<(), String> {
    if layers.frontend {
        let base = root.join("docs/frontend/latest/规范约束");
        write_if_missing(&base.join("详设文档模板.md"), DETAIL_DESIGN_TEMPLATE)?;
        write_if_missing(&base.join("开发进度文档模板.md"), PROGRESS_TEMPLATE)?;
    }
    if layers.backend {
        let base = root.join("docs/backend/latest/规范约束");
        write_if_missing(&base.join("详设文档模板.md"), DETAIL_DESIGN_TEMPLATE)?;
        write_if_missing(&base.join("开发进度文档模板.md"), PROGRESS_TEMPLATE)?;
        if layers.frontend {
            write_if_missing(
                &base.join("前端接入说明模板.md"),
                FRONTEND_INTEGRATION_TEMPLATE,
            )?;
        }
    }
    Ok(())
}

pub fn prepare_existing_project_initialization(
    project_path: &str,
) -> Result<ExistingProjectInitPreparation, String> {
    let root = Path::new(project_path);
    if !root.is_dir() {
        return Err("项目路径不存在或不是目录".to_string());
    }
    let layers = project_layers(root);
    if !layers.frontend && !layers.backend {
        return Err("未识别到前端或后端代码层；请确认项目根目录后再初始化".to_string());
    }
    // 先原样安装 IPS 的 skill-designer，随后 Agent 必须用它设计项目专属 skills。
    // 这里只写入这一项初始化工具，不复制整套模板库，也不触碰业务代码和既有 docs。
    write_skill_designer(root)?;
    // 业务总览、架构、API、物理模型、规则与 skills 都需要严格参照平台模板，但不能把空模板
    // 当成正式项目产物。故这里只提供初始化期间的隐藏只读参考包，成功后自动清理。
    write_initialization_reference_bundle(root)?;
    // 详设、进度、前端接入本来就是目标项目长期保留的规范模板；只在缺失时补齐，绝不
    // 覆盖项目已有版本。其他长期文档必须由 Agent 读取完整源码后填写，不能预铺空壳。
    install_project_document_templates(root, layers)?;
    let agents_skills = root.join(".agents/skills");
    if fs::symlink_metadata(&agents_skills).is_err() {
        fs::create_dir_all(root.join(".agents")).map_err(|error| error.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink("../.claude/skills", &agents_skills).map_err(|error| error.to_string())?;
        }
        #[cfg(not(unix))]
        fs::create_dir_all(&agents_skills).map_err(|error| error.to_string())?;
    }
    Ok(ExistingProjectInitPreparation {
        project_path: root.to_string_lossy().to_string(),
        layers,
        detected_stack: detected_stack(root, layers),
        existing_docs: list_existing(root, "docs"),
        existing_agent_material: ["CLAUDE.md", "AGENTS.md", ".claude", ".agents"]
            .iter()
            .flat_map(|relative| list_existing(root, relative))
            .collect(),
    })
}

fn file_is_real_document(root: &Path, relative: &str) -> Result<(), String> {
    let content = fs::read_to_string(root.join(relative))
        .map_err(|_| format!("缺少初始化后的真实文档：{relative}"))?;
    let compact = content.split_whitespace().collect::<String>();
    if content
        .chars()
        .filter(|character| ('\u{4e00}'..='\u{9fff}').contains(character))
        .count()
        < 10
    {
        return Err(format!("文档不是中文实填内容：{relative}"));
    }
    if compact.len() < 60
        || [
            "{{",
            "待填写",
            "初始化扫描未发现对应证据",
            "|  |",
            "TODO",
            "TBD",
        ]
        .iter()
        .any(|token| content.contains(token))
    {
        return Err(format!("文档仍是空模板或内容不足：{relative}"));
    }
    Ok(())
}

fn require_document_template(root: &Path, relative: &str, headings: &[&str]) -> Result<(), String> {
    let content = fs::read_to_string(root.join(relative))
        .map_err(|_| format!("缺少项目规范模板：{relative}"))?;
    if content.lines().count() < 80 {
        return Err(format!("项目规范模板内容不完整：{relative}"));
    }
    for heading in headings {
        if !content.contains(heading) {
            return Err(format!("项目规范模板缺少章节“{heading}”：{relative}"));
        }
    }
    Ok(())
}

fn require_real_file(root: &Path, relative: &str, kind: &str) -> Result<(), String> {
    let content = fs::read_to_string(root.join(relative))
        .map_err(|_| format!("缺少项目专属{kind}：{relative}"))?;
    if content
        .chars()
        .filter(|character| ('\u{4e00}'..='\u{9fff}').contains(character))
        .count()
        < 10
    {
        return Err(format!("{kind}不是中文实填内容：{relative}"));
    }
    if content.split_whitespace().collect::<String>().len() < 60
        || content.contains("{{")
        || content.contains("待填写")
    {
        return Err(format!("{kind}仍是空模板或内容不足：{relative}"));
    }
    Ok(())
}

fn require_project_skill(root: &Path, relative: &str) -> Result<(), String> {
    let content = fs::read_to_string(root.join(relative))
        .map_err(|_| format!("缺少项目专属skill：{relative}"))?;
    let compact = content.split_whitespace().collect::<String>();
    if content
        .chars()
        .filter(|character| ('\u{4e00}'..='\u{9fff}').contains(character))
        .count()
        < 10
    {
        return Err(format!("skill不是中文实填内容：{relative}"));
    }
    if compact.len() < 300
        || ["{{", "待填写", "初始化扫描未发现对应证据"]
            .iter()
            .any(|token| content.contains(token))
    {
        return Err(format!("skill仍是空模板或内容不足：{relative}"));
    }
    for required in [
        "metadata:",
        "pattern:",
        "## 项目资源",
        "## 执行流程",
        "## 完成 Gate",
        "## 失败处理",
        "CLAUDE.md",
        "docs/",
        ".claude/rules/",
    ] {
        if !content.contains(required) {
            return Err(format!("skill缺少项目化内容“{required}”：{relative}"));
        }
    }
    Ok(())
}

fn require_runtime_assets(
    root: &Path,
    layers: ProjectLayers,
    database_dependency: bool,
    database_model: bool,
    database_connection: bool,
    external_integration: bool,
) -> Result<(), String> {
    let errors = runtime_asset_errors(
        root,
        layers,
        database_dependency,
        database_model,
        database_connection,
        external_integration,
    );
    if errors.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "项目规则与 skills 共有 {} 个校验缺口：\n- {}",
            errors.len(),
            errors.join("\n- ")
        ))
    }
}

fn runtime_asset_errors(
    root: &Path,
    layers: ProjectLayers,
    database_dependency: bool,
    database_model: bool,
    database_connection: bool,
    external_integration: bool,
) -> Vec<String> {
    let mut errors = Vec::new();
    let mut collect = |result: Result<(), String>| {
        if let Err(error) = result {
            errors.push(error);
        }
    };
    collect(require_real_file(
        root,
        ".claude/rules/README.md",
        "规则索引",
    ));
    for rule in [
        ".claude/rules/公共/开发基线.md",
        ".claude/rules/公共/复用与影响面.md",
        ".claude/rules/公共/事实与兜底边界.md",
        ".claude/rules/公共/开发流程与文档同步.md",
        ".claude/rules/公共/自测与交付.md",
    ] {
        collect(require_real_file(root, rule, "规则"));
    }
    if root.join(".git").exists() {
        collect(require_real_file(
            root,
            ".claude/rules/公共/Git协作与历史保护.md",
            "规则",
        ));
    }
    if layers.frontend {
        collect(require_real_file(
            root,
            ".claude/rules/前端/前端工程规则.md",
            "规则",
        ));
        collect(require_real_file(
            root,
            ".claude/rules/前端/前端验证规则.md",
            "规则",
        ));
    }
    if layers.backend {
        collect(require_real_file(
            root,
            ".claude/rules/后端/API与业务实现规则.md",
            "规则",
        ));
        if database_dependency {
            collect(require_real_file(
                root,
                ".claude/rules/后端/持久化与迁移规则.md",
                "规则",
            ));
        }
        if external_integration {
            collect(require_real_file(
                root,
                ".claude/rules/后端/异步与第三方规则.md",
                "规则",
            ));
        }
    }
    for skill in [
        "detail-design-writer",
        "developer",
        "problem-diagnose",
        "code-review",
        "review-feedback-handler",
    ] {
        collect(require_project_skill(
            root,
            &format!(".claude/skills/{skill}/SKILL.md"),
        ));
    }
    if layers.frontend {
        collect(require_project_skill(
            root,
            ".claude/skills/frontend-self-test/SKILL.md",
        ));
    }
    if layers.backend {
        collect(require_project_skill(
            root,
            ".claude/skills/backend-self-test/SKILL.md",
        ));
        collect(require_project_skill(
            root,
            ".claude/skills/backend-log-diagnose/SKILL.md",
        ));
        if database_model {
            collect(require_project_skill(
                root,
                ".claude/skills/ddl-review/SKILL.md",
            ));
        }
        if database_connection {
            collect(require_project_skill(
                root,
                ".claude/skills/database-read-diagnose/SKILL.md",
            ));
        }
        if external_integration {
            collect(require_project_skill(
                root,
                ".claude/skills/external-integration/SKILL.md",
            ));
        }
    }
    collect(validate_skill_designer(root));
    errors
}

fn ensure_agent_links(root: &Path) -> Result<(), String> {
    fs::create_dir_all(root.join(".claude/scripts")).map_err(|error| error.to_string())?;
    fs::create_dir_all(root.join(".agents")).map_err(|error| error.to_string())?;
    for name in ["rules", "skills", "scripts"] {
        let target = root.join(".agents").join(name);
        if fs::symlink_metadata(&target).is_ok() {
            let metadata = fs::symlink_metadata(&target).map_err(|error| error.to_string())?;
            if !metadata.file_type().is_symlink() {
                return Err(format!(
                    ".agents/{name} 必须合并现有内容后软链接到 .claude/{name}"
                ));
            }
            let actual = fs::read_link(&target).map_err(|error| error.to_string())?;
            let expected = std::path::PathBuf::from(format!("../.claude/{name}"));
            if actual != expected {
                return Err(format!(
                    ".agents/{name} 软链接目标错误：应为 {}，实际为 {}",
                    expected.display(),
                    actual.display()
                ));
            }
            continue;
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(format!("../.claude/{name}"), target).map_err(|error| error.to_string())?;
        }
        #[cfg(not(unix))]
        fs::create_dir_all(target).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn validate_agent_entry_and_link(root: &Path) -> Result<String, String> {
    let entry = root.join("CLAUDE.md");
    let content =
        fs::read_to_string(&entry).map_err(|_| "缺少已按当前项目填写的 CLAUDE.md".to_string())?;
    if content.contains("{{")
        || content.contains("待填写")
        || content
            .chars()
            .filter(|character| ('\u{4e00}'..='\u{9fff}').contains(character))
            .count()
            < 20
        || content.split_whitespace().collect::<String>().len() < 120
    {
        return Err("CLAUDE.md 仍是空模板、泛化入口或非中文实填内容".to_string());
    }
    for required in ["docs/", ".claude/rules/", ".claude/skills/", "构建"] {
        if !content.contains(required) {
            return Err(format!("CLAUDE.md 缺少项目导航或开发命令：{required}"));
        }
    }
    if ![
        "测试",
        "自测",
        "TDD",
        "mvn test",
        "cargo test",
        "npm test",
        "pnpm test",
    ]
    .iter()
    .any(|keyword| content.contains(keyword))
    {
        return Err("CLAUDE.md 缺少项目测试或自测命令导航".to_string());
    }
    let agents = root.join("AGENTS.md");
    if agents.exists() || fs::symlink_metadata(&agents).is_ok() {
        let metadata = fs::symlink_metadata(&agents).map_err(|error| error.to_string())?;
        if !metadata.file_type().is_symlink() {
            return Err("AGENTS.md 必须软链接到 CLAUDE.md；请先合并已有双份入口内容".to_string());
        }
        let actual = fs::read_link(&agents).map_err(|error| error.to_string())?;
        if actual != Path::new("CLAUDE.md") {
            return Err(format!(
                "AGENTS.md 软链接目标错误：应为 CLAUDE.md，实际为 {}",
                actual.display()
            ));
        }
    } else {
        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink("CLAUDE.md", &agents).map_err(|error| error.to_string())?;
        }
    }
    Ok(content)
}

fn append_initialization_marker(root: &Path, mut content: String) -> Result<(), String> {
    let entry = root.join("CLAUDE.md");
    if !content.contains(PLATFORM_INIT_MARKER) {
        content.push_str(&format!("\n{PLATFORM_INIT_MARKER}\n"));
        fs::write(&entry, &content).map_err(|error| error.to_string())?;
    }
    #[cfg(not(unix))]
    fs::write(root.join("AGENTS.md"), content).map_err(|error| error.to_string())?;
    Ok(())
}

pub fn finalize_existing_project_initialization(
    project_path: &str,
) -> Result<ExistingProjectInitResult, String> {
    let preparation = prepare_existing_project_initialization(project_path)?;
    let root = Path::new(project_path);
    let database_dependency = has_database_dependency(root);
    let database_model = has_database_model_evidence(root);
    let database_connection = has_database_connection_evidence(root);
    let api = has_api_evidence(root);
    let callback = has_callback_evidence(root);
    let boundary_enum = has_boundary_enum_evidence(root);
    let external_integration = has_external_integration_evidence(root);
    let mut required = Vec::new();
    if preparation.layers.frontend {
        required.extend([
            "docs/frontend/MOC.md",
            "docs/frontend/latest/index.md",
            "docs/frontend/latest/业务/业务功能总览.md",
            "docs/frontend/latest/系统架构/前端架构.md",
            "docs/frontend/latest/公共能力/组件与公共能力.md",
        ]);
    }
    if preparation.layers.backend {
        required.extend([
            "docs/backend/MOC.md",
            "docs/backend/latest/index.md",
            "docs/backend/latest/业务/业务功能总览.md",
            "docs/backend/latest/系统架构/系统架构详解.md",
        ]);
        if api {
            required.push("docs/backend/latest/接口文档/API接口总览.md");
        }
        if callback {
            required.push("docs/backend/latest/接口文档/回调接口总览.md");
        }
        if boundary_enum {
            required.push("docs/backend/latest/接口文档/枚举值总览.md");
        }
        if database_model {
            required.push("docs/backend/latest/接口文档/物理模型总览.md");
        }
        if external_integration {
            required.push("docs/backend/latest/第三方集成/第三方集成总览.md");
        }
    }
    for relative in &required {
        file_is_real_document(root, relative)?;
    }
    if preparation.layers.frontend {
        require_document_template(
            root,
            "docs/frontend/latest/规范约束/详设文档模板.md",
            &["前置材料", "变更摘要", "自测"],
        )?;
        require_document_template(
            root,
            "docs/frontend/latest/规范约束/开发进度文档模板.md",
            &["完成状态", "用户反馈", "文档同步"],
        )?;
    }
    if preparation.layers.backend {
        require_document_template(
            root,
            "docs/backend/latest/规范约束/详设文档模板.md",
            &["前置材料", "变更摘要", "自测"],
        )?;
        require_document_template(
            root,
            "docs/backend/latest/规范约束/开发进度文档模板.md",
            &["完成状态", "用户反馈", "文档同步"],
        )?;
        if preparation.layers.frontend {
            require_document_template(
                root,
                "docs/backend/latest/规范约束/前端接入说明模板.md",
                &["变更概览", "接口清单", "联调验收"],
            )?;
        }
    }
    require_runtime_assets(
        root,
        preparation.layers,
        database_dependency,
        database_model,
        database_connection,
        external_integration,
    )?;
    ensure_agent_links(root)?;
    let entry_content = validate_agent_entry_and_link(root)?;
    remove_initialization_reference_bundle(root)?;
    append_initialization_marker(root, entry_content)?;
    Ok(ExistingProjectInitResult {
        project_path: preparation.project_path,
        layers: preparation.layers,
        detected_stack: preparation.detected_stack,
        generated: required.into_iter().map(str::to_string).collect(),
    })
}

/// 初始化状态只认 Agent 入口中的平台机器标识，不使用浏览器缓存推测。
pub fn existing_project_init_status(
    project_path: &str,
) -> Result<ExistingProjectInitStatus, String> {
    let root = Path::new(project_path);
    if !root.is_dir() {
        return Err("项目路径不存在或不是目录".to_string());
    }
    let initialized = fs::read_to_string(root.join("CLAUDE.md"))
        .map(|content| content.contains(PLATFORM_INIT_MARKER))
        .unwrap_or(false);
    Ok(ExistingProjectInitStatus {
        initialized,
        marker_version: initialized.then(|| "v3".to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::{runtime_asset_errors, ProjectLayers};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn runtime_asset_validation_reports_all_missing_project_skills_at_once() {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("vibe-runtime-assets-{suffix}"));
        fs::create_dir_all(&root).expect("temp project");

        let errors = runtime_asset_errors(
            &root,
            ProjectLayers {
                frontend: false,
                backend: true,
            },
            true,
            true,
            true,
            true,
        );

        let joined = errors.join("\n");
        assert!(joined.contains("backend-log-diagnose"));
        assert!(joined.contains("ddl-review"));
        assert!(joined.contains("database-read-diagnose"));
        assert!(joined.contains("external-integration"));
        assert!(
            errors.len() > 4,
            "应一次返回全部缺口，而不是遇到首个错误就停止"
        );

        fs::remove_dir_all(root).expect("cleanup temp project");
    }
}
