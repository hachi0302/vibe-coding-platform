use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use super::docs::{project_files_named, project_layers, ProjectLayers};
use super::types::CreateProjectRequest;

const DETAIL_DESIGN: &str =
    include_str!("../../../docs/规范约束/技能模板/公共/detail-design-writer/SKILL.md");
const REVIEW_FEEDBACK: &str =
    include_str!("../../../docs/规范约束/技能模板/公共/review-feedback-handler/SKILL.md");
const CODE_REVIEW: &str = include_str!("../../../docs/规范约束/技能模板/公共/code-review/SKILL.md");
const DEVELOPER: &str = include_str!("../../../docs/规范约束/技能模板/公共/developer/SKILL.md");
const PROBLEM_DIAGNOSE: &str =
    include_str!("../../../docs/规范约束/技能模板/公共/problem-diagnose/SKILL.md");
const FRONTEND_SELF_TEST: &str =
    include_str!("../../../docs/规范约束/技能模板/前端/frontend-self-test/SKILL.md");
const BACKEND_SELF_TEST: &str =
    include_str!("../../../docs/规范约束/技能模板/后端/backend-self-test/SKILL.md");
const DDL_REVIEW: &str = include_str!("../../../docs/规范约束/技能模板/可选/ddl-review/SKILL.md");
const BACKEND_LOG_DIAGNOSE: &str =
    include_str!("../../../docs/规范约束/技能模板/可选/backend-log-diagnose/SKILL.md");
const DATABASE_READ_DIAGNOSE: &str =
    include_str!("../../../docs/规范约束/技能模板/可选/database-read-diagnose/SKILL.md");
const EXTERNAL_INTEGRATION: &str =
    include_str!("../../../docs/规范约束/技能模板/可选/external-integration/SKILL.md");

const SKILL_DESIGNER: &str =
    include_str!("../../../docs/规范约束/技能模板/公共/skill-designer/SKILL.md");
const SKILL_DESIGNER_DECISION_TREE: &str =
    include_str!("../../../docs/规范约束/技能模板/公共/skill-designer/references/decision-tree.md");
const SKILL_DESIGNER_GENERATOR: &str = include_str!(
    "../../../docs/规范约束/技能模板/公共/skill-designer/references/generator-example.md"
);
const SKILL_DESIGNER_INVERSION: &str = include_str!(
    "../../../docs/规范约束/技能模板/公共/skill-designer/references/inversion-example.md"
);
const SKILL_DESIGNER_PIPELINE: &str = include_str!(
    "../../../docs/规范约束/技能模板/公共/skill-designer/references/pipeline-example.md"
);
const SKILL_DESIGNER_REVIEWER: &str = include_str!(
    "../../../docs/规范约束/技能模板/公共/skill-designer/references/reviewer-example.md"
);
const SKILL_DESIGNER_TOOL_WRAPPER: &str = include_str!(
    "../../../docs/规范约束/技能模板/公共/skill-designer/references/tool-wrapper-example.md"
);
const SKILL_DESIGNER_EVALS: &str =
    include_str!("../../../docs/规范约束/技能模板/公共/skill-designer/evals/evals.json");

const DEVELOPMENT_BASELINE_RULE: &str =
    include_str!("../../../docs/规范约束/规则模板/公共/开发基线.md");
const REUSE_AND_IMPACT_RULE: &str =
    include_str!("../../../docs/规范约束/规则模板/公共/复用与影响面.md");
const FACT_AND_FALLBACK_RULE: &str =
    include_str!("../../../docs/规范约束/规则模板/公共/事实与兜底边界.md");
const DEVELOPMENT_FLOW_RULE: &str =
    include_str!("../../../docs/规范约束/规则模板/公共/开发流程与文档同步.md");
const GIT_COLLABORATION_RULE: &str =
    include_str!("../../../docs/规范约束/规则模板/公共/Git协作与历史保护.md");
const SELF_TEST_AND_DELIVERY_RULE: &str =
    include_str!("../../../docs/规范约束/规则模板/公共/自测与交付.md");
const DOC_SYNC_REVIEW_RULE: &str =
    include_str!("../../../docs/规范约束/规则模板/公共/doc-sync-review.md");
const DOC_SYNC_REVIEW_SKILL: &str =
    include_str!("../../../docs/规范约束/技能模板/公共/doc-sync-review/SKILL.md");
const DOC_SYNC_GATE: &str =
    include_str!("../../../docs/规范约束/技能模板/公共/doc-sync-review/scripts/doc-sync-gate.sh");
const DOC_SYNC_HOOK_MARKER: &str = "# vibe-coding-platform:doc-sync-gate:v1";
const FRONTEND_ENGINEERING_RULE: &str =
    include_str!("../../../docs/规范约束/规则模板/前端/前端工程规则.md");
const FRONTEND_VERIFICATION_RULE: &str =
    include_str!("../../../docs/规范约束/规则模板/前端/前端验证规则.md");
const BACKEND_API_RULE: &str =
    include_str!("../../../docs/规范约束/规则模板/后端/API与业务实现规则.md");
const BACKEND_PERSISTENCE_RULE: &str =
    include_str!("../../../docs/规范约束/规则模板/后端/持久化与迁移规则.md");
const BACKEND_INTEGRATION_RULE: &str =
    include_str!("../../../docs/规范约束/规则模板/后端/异步与第三方规则.md");

const PLATFORM_INIT_MARKER: &str = "<!-- vibe-coding-platform:init:v3 -->";

fn write_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    fs::write(path, content).map_err(|error| error.to_string())
}

fn write_skill(root: &Path, name: &str, content: &str) -> Result<(), String> {
    write_file(
        &root.join(".claude/skills").join(name).join("SKILL.md"),
        content,
    )
}

fn write_rule(root: &Path, path: &str, content: &str) -> Result<(), String> {
    write_file(&root.join(".claude/rules").join(path), content)
}

fn doc_sync_long_document_mapping(layers: ProjectLayers) -> String {
    let mut rows = Vec::new();
    if layers.frontend {
        rows.extend([
            "- 业务功能变化 → `docs/frontend/latest/业务/业务功能总览.md`",
            "- 页面、路由、状态管理或整体分层变化 → `docs/frontend/latest/系统架构/前端架构.md`",
            "- 公共组件、工具或请求封装变化 → `docs/frontend/latest/公共能力/组件与公共能力.md`",
        ]);
    }
    if layers.backend {
        rows.extend([
            "- 业务功能变化 → `docs/backend/latest/业务/业务功能总览.md`",
            "- 模块边界、调用链或公共能力变化 → `docs/backend/latest/系统架构/系统架构详解.md`",
            "- API 变化 → `docs/backend/latest/接口文档/API接口总览.md`",
            "- 回调变化 → `docs/backend/latest/接口文档/回调接口总览.md`",
            "- 实体、迁移、表或字段变化 → `docs/backend/latest/接口文档/物理模型总览.md`",
            "- 对外枚举值变化 → `docs/backend/latest/接口文档/枚举值总览.md`",
        ]);
    }
    rows.join("\n")
}

#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;
    let mut permissions = fs::metadata(path)
        .map_err(|error| error.to_string())?
        .permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).map_err(|error| error.to_string())
}

#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), String> {
    Ok(())
}

/// 安装所有项目通用的提交前文档一致性审核。规则和 skill 共用一份来源；hook 只做
/// 确定性的暂存区指纹校验，不让 shell 猜测业务影响。
pub(super) fn install_doc_sync_review(root: &Path, layers: ProjectLayers) -> Result<(), String> {
    let rule = format!(
        "{}\n{}\n",
        DOC_SYNC_REVIEW_RULE.trim_end(),
        doc_sync_long_document_mapping(layers)
    );
    write_rule(root, "code/doc-sync-review.md", &rule)?;
    write_skill(root, "doc-sync-review", DOC_SYNC_REVIEW_SKILL)?;
    let gate = root.join(".claude/skills/doc-sync-review/scripts/doc-sync-gate.sh");
    write_file(&gate, DOC_SYNC_GATE)?;
    make_executable(&gate)?;

    let hook = root.join(".githooks/pre-commit");
    if hook.exists() {
        let existing = fs::read_to_string(&hook).map_err(|error| error.to_string())?;
        if !existing.contains(DOC_SYNC_HOOK_MARKER) {
            let preserved = root.join(".githooks/pre-commit.user");
            if !preserved.exists() {
                write_file(&preserved, &existing)?;
                make_executable(&preserved)?;
            }
        }
    }
    write_file(
        &hook,
        &format!(
            "#!/bin/sh\n{DOC_SYNC_HOOK_MARKER}\nset -eu\nif [ -x .githooks/pre-commit.user ]; then\n  .githooks/pre-commit.user\nfi\nexec .claude/skills/doc-sync-review/scripts/doc-sync-gate.sh --check\n"
        ),
    )?;
    make_executable(&hook)?;

    if is_git_repository(root) {
        let output = Command::new("git")
            .args(["config", "core.hooksPath", ".githooks"])
            .current_dir(root)
            .output()
            .map_err(|error| format!("配置 Git hooksPath 失败：{error}"))?;
        if !output.status.success() {
            return Err(format!(
                "配置 Git hooksPath 失败：{}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
    }
    Ok(())
}

fn package_scripts(root: &Path) -> String {
    project_files_named(root, "package.json")
        .into_iter()
        .filter_map(|path| fs::read_to_string(path).ok())
        .collect::<Vec<_>>()
        .join("\n")
}

fn package_script_commands(root: &Path) -> BTreeMap<String, String> {
    let mut commands = BTreeMap::new();
    for path in project_files_named(root, "package.json") {
        let Ok(raw) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            continue;
        };
        let Some(scripts) = value.get("scripts").and_then(Value::as_object) else {
            continue;
        };
        for name in scripts.keys() {
            commands
                .entry(name.clone())
                .or_insert_with(|| format!("{} run {name}", package_manager(root)));
        }
    }
    commands
}

fn package_manager(root: &Path) -> &'static str {
    if root.join("pnpm-lock.yaml").exists() {
        "pnpm"
    } else if root.join("yarn.lock").exists() {
        "yarn"
    } else if root.join("bun.lock").exists() || root.join("bun.lockb").exists() {
        "bun"
    } else {
        "npm"
    }
}

fn first_script(scripts: &BTreeMap<String, String>, names: &[&str], missing: &str) -> String {
    names
        .iter()
        .find_map(|name| scripts.get(*name).cloned())
        .unwrap_or_else(|| missing.to_string())
}

fn reject_forbidden_material(content: &str, label: &str) -> Result<(), String> {
    for marker in ["{{", "待填写", "初始化扫描未发现对应证据"] {
        if content.contains(marker) {
            return Err(format!("{label} 仍包含未解析模板内容：{marker}"));
        }
    }
    Ok(())
}

fn validate_generated_materials(root: &Path) -> Result<(), String> {
    fn visit(path: &Path, base: &Path) -> Result<(), String> {
        for entry in fs::read_dir(path).map_err(|error| error.to_string())? {
            let entry = entry.map_err(|error| error.to_string())?;
            let current = entry.path();
            if current.is_dir() {
                visit(&current, base)?;
                continue;
            }
            if !matches!(
                current.extension().and_then(|value| value.to_str()),
                Some("md" | "json")
            ) {
                continue;
            }
            let content = fs::read_to_string(&current).map_err(|error| error.to_string())?;
            reject_forbidden_material(&content, &relative(base, &current))?;
        }
        Ok(())
    }

    for relative in [
        ".claude/rules/公共/开发基线.md",
        ".claude/rules/公共/复用与影响面.md",
        ".claude/rules/公共/事实与兜底边界.md",
        ".claude/rules/公共/开发流程与文档同步.md",
        ".claude/rules/公共/自测与交付.md",
        ".claude/rules/公共/Git协作与历史保护.md",
        ".claude/rules/code/doc-sync-review.md",
        ".claude/rules/前端/前端工程规则.md",
        ".claude/rules/前端/前端验证规则.md",
        ".claude/rules/后端/API与业务实现规则.md",
        ".claude/rules/后端/持久化与迁移规则.md",
        ".claude/rules/后端/异步与第三方规则.md",
    ] {
        let path = root.join(relative);
        if path.is_file() {
            let content = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            reject_forbidden_material(&content, relative)?;
        }
    }
    for name in [
        "skill-designer",
        "detail-design-writer",
        "review-feedback-handler",
        "code-review",
        "developer",
        "problem-diagnose",
        "frontend-self-test",
        "backend-self-test",
        "backend-log-diagnose",
        "database-read-diagnose",
        "ddl-review",
        "external-integration",
    ] {
        let directory = root.join(".claude/skills").join(name);
        if directory.is_dir() {
            visit(&directory, root)?;
        }
    }
    Ok(())
}

fn is_git_repository(root: &Path) -> bool {
    Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(root)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn commands(root: &Path, layers: ProjectLayers) -> (String, String, String, String) {
    let package = package_scripts(root);
    let scripts = package_script_commands(root);
    if layers.frontend && !layers.backend {
        return (
            first_script(
                &scripts,
                &["test:run", "test", "test:unit"],
                "package.json 未定义测试脚本",
            ),
            first_script(&scripts, &["lint"], "package.json 未定义 lint 脚本"),
            first_script(
                &scripts,
                &["typecheck", "type-check", "check"],
                "package.json 未定义类型检查脚本",
            ),
            first_script(&scripts, &["build"], "package.json 未定义构建脚本"),
        );
    }
    if !project_files_named(root, "pom.xml").is_empty() {
        return (
            "mvn test".to_string(),
            "未配置独立 lint 命令".to_string(),
            "由 Maven 编译阶段检查".to_string(),
            "mvn clean verify".to_string(),
        );
    }
    if !project_files_named(root, "pyproject.toml").is_empty() {
        return (
            "未配置测试框架".to_string(),
            "未配置 lint 命令".to_string(),
            "未配置类型检查命令".to_string(),
            "python -m compileall app".to_string(),
        );
    }
    if !project_files_named(root, "go.mod").is_empty() {
        return (
            "go test ./...".to_string(),
            "gofmt -w 前先使用 gofmt -d 检查".to_string(),
            "go test ./...".to_string(),
            "go build ./...".to_string(),
        );
    }
    if !project_files_named(root, "Cargo.toml").is_empty() {
        return (
            "cargo test".to_string(),
            "cargo clippy --all-targets".to_string(),
            "cargo check".to_string(),
            "cargo build".to_string(),
        );
    }
    if !project_files_named(root, "Program.cs").is_empty() {
        return (
            "未配置独立测试项目".to_string(),
            "dotnet format --verify-no-changes".to_string(),
            "dotnet build".to_string(),
            "dotnet build".to_string(),
        );
    }
    if layers.backend && package.contains("@nestjs") {
        return (
            first_script(
                &scripts,
                &["test", "test:run"],
                "package.json 未定义测试脚本",
            ),
            first_script(&scripts, &["lint"], "package.json 未定义 lint 脚本"),
            first_script(&scripts, &["typecheck", "type-check"], "npx tsc --noEmit"),
            first_script(&scripts, &["build"], "package.json 未定义构建脚本"),
        );
    }
    (
        "未配置".to_string(),
        "未配置".to_string(),
        "未配置".to_string(),
        "未配置".to_string(),
    )
}

fn stack_summary(request: &CreateProjectRequest, layers: ProjectLayers) -> String {
    let mut stack = Vec::new();
    if layers.frontend {
        stack.extend(request.recommendation.frontend.clone());
    }
    if layers.backend {
        stack.extend(request.recommendation.backend.clone());
        stack.extend(request.recommendation.database.clone());
        stack.extend(request.recommendation.cache.clone());
        stack.extend(request.recommendation.messaging.clone());
    }
    stack.sort();
    stack.dedup();
    if stack.is_empty() {
        "以构建文件为准".to_string()
    } else {
        stack.join("、")
    }
}

#[derive(Clone, Copy)]
enum MaterialLayer {
    Common,
    Frontend,
    Backend,
}

fn frontend_commands(root: &Path) -> (String, String, String, String) {
    let scripts = package_script_commands(root);
    (
        first_script(
            &scripts,
            &["test:run", "test", "test:unit"],
            "package.json 未定义测试脚本",
        ),
        first_script(&scripts, &["lint"], "package.json 未定义 lint 脚本"),
        first_script(
            &scripts,
            &["typecheck", "type-check", "check"],
            "package.json 未定义类型检查脚本",
        ),
        first_script(&scripts, &["build"], "package.json 未定义构建脚本"),
    )
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn existing_files(root: &Path, names: &[&str]) -> Vec<String> {
    let mut paths = names
        .iter()
        .flat_map(|name| project_files_named(root, name))
        .map(|path| relative(root, &path))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn collect_dirs(root: &Path, depth: usize, names: &[&str], output: &mut Vec<PathBuf>) {
    if depth > 4 {
        return;
    }
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        if matches!(
            name.as_str(),
            ".git" | "node_modules" | "target" | "dist" | "build" | "docs"
        ) {
            continue;
        }
        if names.iter().any(|candidate| *candidate == name) {
            output.push(path.clone());
        }
        collect_dirs(&path, depth + 1, names, output);
    }
}

fn existing_dirs(root: &Path, names: &[&str]) -> Vec<String> {
    let mut dirs = Vec::new();
    collect_dirs(root, 0, names, &mut dirs);
    let mut paths = dirs
        .into_iter()
        .map(|path| relative(root, &path))
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    paths
}

fn collect_source_files(
    base: &Path,
    current: &Path,
    depth: usize,
    name_patterns: &[&str],
    output: &mut Vec<PathBuf>,
) {
    if depth > 8 || output.len() >= 48 {
        return;
    }
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            if matches!(
                name.as_str(),
                ".git" | ".idea" | "node_modules" | "target" | "dist" | "build" | "docs"
            ) {
                continue;
            }
            collect_source_files(base, &path, depth + 1, name_patterns, output);
            continue;
        }
        let lower = name.to_lowercase();
        let path_lower = relative(base, &path).to_lowercase();
        let source_file = matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("java" | "kt" | "ts" | "tsx" | "js" | "jsx" | "vue" | "py" | "go" | "rs" | "cs")
        );
        if source_file
            && name_patterns.iter().any(|pattern| {
                let pattern = pattern.to_lowercase();
                lower.contains(&pattern) || path_lower.contains(&pattern)
            })
        {
            output.push(path);
            if output.len() >= 48 {
                return;
            }
        }
    }
}

fn source_files_matching(root: &Path, patterns: &[&str]) -> Vec<String> {
    let mut files = Vec::new();
    collect_source_files(root, root, 0, patterns, &mut files);
    let mut relative_paths = files
        .into_iter()
        .map(|path| relative(root, &path))
        .collect::<Vec<_>>();
    relative_paths.sort();
    relative_paths.dedup();
    relative_paths
}

fn manifest_contents(root: &Path) -> String {
    [
        "package.json",
        "pom.xml",
        "build.gradle",
        "build.gradle.kts",
        "pyproject.toml",
        "requirements.txt",
        "go.mod",
        "Cargo.toml",
        "Program.cs",
    ]
    .into_iter()
    .flat_map(|name| project_files_named(root, name))
    .filter_map(|path| fs::read_to_string(path).ok())
    .collect::<Vec<_>>()
    .join("\n")
    .to_lowercase()
}

fn actual_backend_framework(root: &Path, request: &CreateProjectRequest) -> String {
    let manifests = manifest_contents(root);
    let known = [
        ("spring-boot", "Spring Boot"),
        ("spring boot", "Spring Boot"),
        ("quarkus", "Quarkus"),
        ("micronaut", "Micronaut"),
        ("fastapi", "FastAPI"),
        ("django", "Django"),
        ("flask", "Flask"),
        ("@nestjs", "NestJS"),
        ("express", "Express"),
        ("axum", "Axum"),
        ("actix-web", "Actix Web"),
        ("gin-gonic", "Gin"),
        ("aspnetcore", "ASP.NET Core"),
    ];
    let mut frameworks = known
        .iter()
        .filter(|(needle, _)| manifests.contains(needle))
        .map(|(_, label)| (*label).to_string())
        .collect::<Vec<_>>();
    frameworks.sort();
    frameworks.dedup();
    if frameworks.is_empty() {
        if request.recommendation.backend.is_empty() {
            "当前构建文件未识别出具体后端框架".to_string()
        } else {
            format!(
                "技术选型为 {}；实现时仍以构建文件和源码为准",
                request.recommendation.backend.join("、")
            )
        }
    } else {
        frameworks.join("、")
    }
}

fn markdown_paths(paths: Vec<String>, empty: &str) -> String {
    if paths.is_empty() {
        empty.to_string()
    } else {
        paths
            .into_iter()
            .map(|path| format!("`{path}`"))
            .collect::<Vec<_>>()
            .join("、")
    }
}

fn project_evidence(root: &Path) -> String {
    let mut paths = existing_files(
        root,
        &[
            "package.json",
            "pom.xml",
            "pyproject.toml",
            "go.mod",
            "Cargo.toml",
            "Program.cs",
            "tsconfig.json",
        ],
    );
    paths.extend(existing_dirs(root, &["src", "app", "packages", "modules"]));
    paths.sort();
    paths.dedup();
    markdown_paths(paths, "项目根目录与当前源码")
}

fn test_evidence(root: &Path) -> String {
    let mut paths = existing_dirs(root, &["test", "tests", "__tests__"]);
    paths.extend(existing_files(
        root,
        &["vitest.config.ts", "jest.config.js", "pytest.ini"],
    ));
    paths.sort();
    paths.dedup();
    markdown_paths(
        paths,
        "当前工程尚未形成独立测试目录；新增行为时先按项目测试脚本建立测试",
    )
}

fn build_evidence(root: &Path) -> String {
    markdown_paths(
        existing_files(
            root,
            &[
                "package.json",
                "pom.xml",
                "pyproject.toml",
                "go.mod",
                "Cargo.toml",
                "Program.cs",
            ],
        ),
        "项目根目录",
    )
}

fn docs_root(layers: ProjectLayers, target: MaterialLayer) -> &'static str {
    match target {
        MaterialLayer::Frontend => "docs/frontend",
        MaterialLayer::Backend => "docs/backend",
        MaterialLayer::Common if layers.backend => "docs/backend",
        MaterialLayer::Common => "docs/frontend",
    }
}

fn selected_database(request: &CreateProjectRequest) -> String {
    if request.recommendation.database.is_empty() {
        "当前选型未采用关系型数据库".to_string()
    } else {
        request.recommendation.database.join("、")
    }
}

fn database_config_paths(root: &Path) -> Vec<String> {
    existing_files(
        root,
        &[
            "application.yaml",
            "application.yml",
            "application.properties",
            ".env",
            ".env.example",
            "config.toml",
            "database.yml",
            "prisma.schema",
        ],
    )
}

fn has_database_capability_evidence(root: &Path, request: &CreateProjectRequest) -> bool {
    if !request.recommendation.database.is_empty() || !database_config_paths(root).is_empty() {
        return true;
    }
    let dependencies = format!(
        "{}\n{}\n{}",
        package_scripts(root),
        project_files_named(root, "pom.xml")
            .into_iter()
            .filter_map(|path| fs::read_to_string(path).ok())
            .collect::<Vec<_>>()
            .join("\n"),
        project_files_named(root, "pyproject.toml")
            .into_iter()
            .filter_map(|path| fs::read_to_string(path).ok())
            .collect::<Vec<_>>()
            .join("\n")
    )
    .to_lowercase();
    [
        "mysql",
        "postgres",
        "mariadb",
        "oracle",
        "sqlserver",
        "sqlite",
        "prisma",
        "sqlalchemy",
        "jdbc",
    ]
    .iter()
    .any(|needle| dependencies.contains(needle))
}

fn maven_multi_module_test_guidance(root: &Path) -> String {
    if project_files_named(root, "pom.xml").is_empty() {
        "不适用：当前未检测到 Maven `pom.xml`".to_string()
    } else {
        "`mvn -pl <模块> -am -Dtest=<测试类> test -Dsurefire.failIfNoSpecifiedTests=false`；避免上游依赖模块没有同名测试时误报失败".to_string()
    }
}

fn log_capability_rows(root: &Path) -> String {
    let local = existing_files(
        root,
        &[
            "application.yaml",
            "application.yml",
            "application.properties",
            "logback.xml",
            "log4j2.xml",
            "tracing.ts",
        ],
    );
    let containers = existing_files(
        root,
        &["Dockerfile", "docker-compose.yml", "docker-compose.yaml"],
    );
    let kubernetes = existing_files(
        root,
        &["deployment.yaml", "deployment.yml", "kustomization.yaml"],
    );
    let row = |name: &str, evidence: Vec<String>, missing: &str, probe: &str| {
        let (status, location) = if evidence.is_empty() {
            ("未配置", "项目中未发现接入线索".to_string())
        } else {
            (
                "有证据但需配置",
                markdown_paths(evidence, "项目中未发现接入线索"),
            )
        };
        format!(
            "| {name} | {status} | {missing} | {location} | `{probe}` | 命令退出码 0、环境/服务/时间与脱敏结果摘要 |"
        )
    };
    let rows = [
        row(
            "本地文件/标准输出",
            local,
            "真实日志路径、启动方式、时间格式与检索命令",
            "读取一行当前环境日志",
        ),
        row(
            "Docker/容器",
            containers,
            "容器名、服务名与只读日志命令",
            "docker logs <container> --tail 1",
        ),
        row(
            "Kubernetes",
            kubernetes,
            "context、namespace、服务/Deployment、label selector、container",
            "kubectl logs -n <namespace> <pod> -c <container> --tail=1",
        ),
        row(
            "集中日志",
            Vec::new(),
            "Loki/ELK/Grafana/云日志入口、查询范围与只读授权",
            "使用已授权 CLI/API 查询一条日志",
        ),
        row(
            "SSH/远程 API",
            Vec::new(),
            "主机别名或 API/CLI、只读权限与脱敏规则",
            "读取一行非生产敏感日志",
        ),
    ]
    .join("\n");
    format!(
        "| 能力项 | 当前状态 | 缺少信息 | 补充位置/证据 | 最小只读探测 | 验收证据 |\n|---|---|---|---|---|---|\n{rows}"
    )
}

fn database_capability_rows(root: &Path, request: &CreateProjectRequest) -> String {
    let configs = database_config_paths(root);
    let evidence = if configs.is_empty() {
        if request.recommendation.database.is_empty() {
            "项目中未发现数据库接入线索".to_string()
        } else {
            format!("技术选型：{}", selected_database(request))
        }
    } else {
        markdown_paths(configs, "项目中未发现数据库接入线索")
    };
    format!(
        "| 能力项 | 当前状态 | 缺少信息 | 补充位置/证据 | 最小只读探测 | 验收证据 |\n|---|---|---|---|---|---|\n| 数据库只读诊断 | 有证据但需配置 | 环境到数据库映射、数据库类型/版本、凭证来源名称、只读账号、CLI/MCP/脚本 | {evidence} | `SELECT 1`，并查询当前库/用户/版本/只读状态/授权范围 | 命令退出码 0、验证时间/环境与脱敏结果摘要 |"
    )
}

fn adopted_decision(request: &CreateProjectRequest, category: &str) -> bool {
    request.recommendation.decisions.iter().any(|decision| {
        decision.status == "adopt" && decision.category.eq_ignore_ascii_case(category)
    })
}

fn git_branch(root: &Path) -> String {
    let head = fs::read_to_string(root.join(".git/HEAD")).unwrap_or_default();
    head.trim()
        .strip_prefix("ref: refs/heads/")
        .unwrap_or("尚未创建首个分支提交")
        .to_string()
}

fn development_baseline_rows(
    root: &Path,
    request: &CreateProjectRequest,
    layers: ProjectLayers,
) -> String {
    let (_, lint, typecheck, build) = commands(root, layers);
    format!(
        "| 代码层/模块 | {} | {} | 以同目录现有文件为准 |\n| 命名/格式 | 延续现有源码与配置 | {} | 不引入第二套风格 |\n| 构建/检查 | `{}`；`{}`；`{}` | {} | 命令退出码为 0 |",
        stack_summary(request, layers),
        project_evidence(root),
        markdown_paths(existing_files(root, &[".editorconfig", "eslint.config.js", "eslint.config.ts", ".prettierrc", "rustfmt.toml", "pom.xml"]), "当前构建文件与同目录源码"),
        lint,
        typecheck,
        build,
        build_evidence(root),
    )
}

fn common_capability_rows(root: &Path) -> String {
    let paths = existing_dirs(
        root,
        &[
            "components",
            "composables",
            "hooks",
            "utils",
            "util",
            "common",
            "shared",
            "types",
            "api",
            "clients",
        ],
    );
    let source_files = source_files_matching(
        root,
        &[
            "util",
            "helper",
            "converter",
            "mapper",
            "enum",
            "constant",
            "exception",
            "errorcode",
            "client",
            "adapter",
            "base",
            "abstract",
        ],
    );
    if paths.is_empty() && source_files.is_empty() {
        "| 当前未形成独立公共目录 | 新增前先全局检索同职责实现 | 当前源码 | 以实际调用方为证据 | 不为单一调用提前抽象 |".to_string()
    } else {
        let mut rows = paths
            .into_iter()
            .map(|path| format!("| `{path}` | 复用其中已有组件、类型、工具或客户端 | `{path}` | 使用前检索真实调用方 | 保持当前目录职责 |"))
            .collect::<Vec<_>>();
        rows.extend(source_files.into_iter().map(|path| {
            format!("| `{path}` | 复用该文件已定义的公共类型、工具、枚举或错误表达 | `{path}` | 全局检索该符号的真实调用方 | 修改前评估所有引用方 |")
        }));
        rows.join("\n")
    }
}

fn git_rows(root: &Path) -> String {
    format!(
        "| 主/开发分支 | `{}` | `.git/HEAD` |\n| 分支命名 | 遵循仓库现有分支；无历史时由用户确认 | Git 分支列表 |\n| commit 格式 | 遵循相邻历史提交；无历史时提交前由用户确认 | `git log` |\n| 提交前检查 | 只暂存本任务文件并执行项目真实验证命令 | 构建文件与测试结果 |",
        git_branch(root)
    )
}

fn verification_rows(root: &Path, layers: ProjectLayers) -> String {
    let mut rows = Vec::new();
    if layers.frontend {
        let (test, lint, typecheck, build) = frontend_commands(root);
        rows.push(format!(
            "| 前端 | `{test}` | `{test}` | `{build}` | `{lint}`；`{typecheck}` | `package.json` |"
        ));
    }
    if layers.backend {
        let (test, lint, typecheck, build) = commands(root, layers);
        rows.push(format!(
            "| 后端 | `{test}` | `{test}` | `{build}` | `{lint}`；`{typecheck}` | {} |",
            build_evidence(root)
        ));
    }
    rows.join("\n")
}

fn dependency_fact<'a>(package: &str, needles: &[&str], yes: &'a str, no: &'a str) -> &'a str {
    if needles.iter().any(|needle| package.contains(needle)) {
        yes
    } else {
        no
    }
}

fn actual_frontend_framework(root: &Path, request: &CreateProjectRequest) -> String {
    let manifests = manifest_contents(root);
    let known = [
        ("\"vue\"", "Vue 3"),
        ("\"react\"", "React"),
        ("\"next\"", "Next.js"),
        ("\"svelte\"", "Svelte"),
        ("@angular/core", "Angular"),
        ("\"nuxt\"", "Nuxt"),
        ("\"astro\"", "Astro"),
        ("solid-js", "SolidJS"),
    ];
    let mut frameworks = known
        .iter()
        .filter(|(needle, _)| manifests.contains(needle))
        .map(|(_, label)| (*label).to_string())
        .collect::<Vec<_>>();
    frameworks.sort();
    frameworks.dedup();
    if frameworks.is_empty() {
        if request.recommendation.frontend.is_empty() {
            "当前构建文件未识别出具体前端框架".to_string()
        } else {
            format!(
                "技术选型为 {}；实现时仍以构建文件和源码为准",
                request.recommendation.frontend.join("、")
            )
        }
    } else {
        frameworks.join("、")
    }
}

fn frontend_engineering_rows(root: &Path, request: &CreateProjectRequest) -> String {
    let package = package_scripts(root).to_lowercase();
    let routes = markdown_paths(
        source_files_matching(root, &["router", "routes", "routing"]),
        "当前源码中未识别出独立路由文件",
    );
    let state = markdown_paths(
        source_files_matching(root, &["store", "stores", "state", "context"]),
        "当前源码中未识别出独立共享状态文件",
    );
    let clients = markdown_paths(
        source_files_matching(root, &["api", "client", "request", "http"]),
        "当前源码中未识别出独立请求客户端文件",
    );
    let components = markdown_paths(
        source_files_matching(root, &["component", "components", "layout"]),
        "当前源码中未识别出公共组件文件",
    );
    let styles = markdown_paths(
        source_files_matching(root, &["style", "theme", "token"]),
        "当前源码中未识别出独立样式 token 文件",
    );
    format!(
        "| 前端框架 | {} | {} | 以构建依赖和已有源码用法为准 |\n| 路由 | {} | {} | 使用前读取现有路由入口和守卫 |\n| 状态管理 | {} | {} | 使用前读取现有 store/context 调用方 |\n| 请求封装 | {} | {} | 不在页面或组件中重复创建客户端 |\n| 公共组件 | 优先复用当前组件体系 | {} | 新增前检索真实调用方 |\n| UI/样式 | {} | {}；{} | 延续现有组件、视觉语言与 token |\n| 权限与错误展示 | 只按路由、组件和请求拦截的真实证据实现 | {}；{} | 未发现证据时不自创权限框架，不吞错或伪造成功 |",
        actual_frontend_framework(root, request),
        build_evidence(root),
        dependency_fact(&package, &["vue-router", "react-router", "next"], "已检测到路由依赖", "未检测到独立路由依赖"), routes,
        dependency_fact(&package, &["pinia", "vuex", "redux", "zustand", "mobx"], "已检测到状态库", "未检测到独立状态库"), state,
        dependency_fact(&package, &["axios", "ky", "@tanstack/query"], "已检测到请求依赖", "使用运行时原生请求能力或当前源码封装"), clients,
        components,
        dependency_fact(&package, &["element-plus", "antd", "tailwind", "vuetify", "@mui"], "已检测到 UI/样式依赖", "使用当前源码样式体系"), components, styles,
        routes, clients,
    )
}

fn backend_engineering_rows(root: &Path, request: &CreateProjectRequest) -> String {
    let evidence = project_evidence(root);
    let api_entries = markdown_paths(
        source_files_matching(root, &["controller", "resource", "router", "route"]),
        "当前源码中未识别出命名明确的 API 入口",
    );
    let error_entries = markdown_paths(
        source_files_matching(root, &["exception", "errorcode", "advice", "problem"]),
        "当前源码中未识别出独立异常/错误码文件",
    );
    let enum_entries = markdown_paths(
        source_files_matching(root, &["enum", "status", "type"]),
        "当前源码中未识别出命名明确的枚举文件",
    );
    let common_entries = markdown_paths(
        source_files_matching(root, &["util", "helper", "converter", "mapper", "client"]),
        "当前源码中未识别出命名明确的公共工具文件",
    );
    format!(
        "| 后端框架 | {} | {} | 以构建依赖和已有源码用法为准 |\n| API 层 | 沿用当前框架入口与序列化方式 | {} | 修改前读取真实入口及调用链 |\n| 业务层 | 按当前项目分层组织 | {} | 新增前检索同职责实现 |\n| 公共工具/转换器/客户端 | 优先复用现有实现 | {} | 使用前读取真实调用方 |\n| 枚举与状态 | 复用已有枚举和值语义 | {} | 改值前全局检索序列化和分支引用 |\n| 错误体系 | 复用现有异常、错误码与响应结构 | {} | 无现成体系时先在详设确定 |\n| 鉴权 | 只采用代码或需求已确认的机制 | {} | 未确认时不默认放行 |\n| 日志 | 复用运行时日志门面与当前格式 | {} | 敏感信息脱敏 |",
        actual_backend_framework(root, request),
        build_evidence(root),
        api_entries,
        evidence,
        common_entries,
        enum_entries,
        error_entries,
        evidence,
        evidence,
    )
}

fn persistence_rows(root: &Path, request: &CreateProjectRequest) -> String {
    let evidence = build_evidence(root);
    format!(
        "| 数据库/驱动 | {} | {} | 以已选依赖和配置为准 |\n| ORM/查询层 | 以当前源码实际引入为准 | {} | 未引入时不得假设 ORM |\n| 迁移工具 | 以迁移目录和构建依赖为准 | {} | 未配置时先在详设确认 |\n| 事务 | 以当前运行时与持久化框架为准 | {} | 不跨边界伪造原子性 |",
        selected_database(request), evidence, project_evidence(root), evidence, evidence,
    )
}

fn integration_rows(root: &Path, request: &CreateProjectRequest) -> String {
    let messaging = if request.recommendation.messaging.is_empty() {
        "当前选型未采用消息中间件".to_string()
    } else {
        request.recommendation.messaging.join("、")
    };
    let evidence = project_evidence(root);
    format!(
        "| 消息/任务 | {} | {} | 使用前核对真实生产与消费入口 |\n| 外部客户端/SDK | 只按需求与依赖中已确认的客户端实现 | {} | 官方契约优先 |\n| 回调 | 只有需求明确包含回调时设计 | {} | 必须覆盖验签、幂等和重试影响 |",
        messaging, evidence, evidence, evidence,
    )
}

fn log_source(root: &Path) -> String {
    let configs = existing_files(
        root,
        &[
            "application.yaml",
            "application.yml",
            "logback.xml",
            "log4j2.xml",
            "tracing.ts",
        ],
    );
    if configs.is_empty() {
        "应用启动进程的标准输出；持久化或集中日志入口尚未由项目配置证明".to_string()
    } else {
        format!(
            "应用标准输出；配置 {} 仅作为日志接入线索，尚未完成读取探测",
            markdown_paths(configs, "")
        )
    }
}

fn render_template(
    template: &str,
    root: &Path,
    request: &CreateProjectRequest,
    layers: ProjectLayers,
) -> Result<String, String> {
    render_template_for(template, root, request, layers, MaterialLayer::Common)
}

fn render_template_for(
    template: &str,
    root: &Path,
    request: &CreateProjectRequest,
    layers: ProjectLayers,
    target: MaterialLayer,
) -> Result<String, String> {
    let (test, lint, typecheck, build) = match target {
        MaterialLayer::Frontend => frontend_commands(root),
        _ => commands(root, layers),
    };
    let docs_root = docs_root(layers, target);
    let rendered = template
        .replace("{{项目名称}}", &request.project_name)
        .replace("{{项目定位}}", &request.profile.summary)
        .replace("{{项目技术栈}}", &stack_summary(request, layers))
        .replace("{{Agent入口}}", "CLAUDE.md")
        .replace("{{项目总览路径}}", &format!("{docs_root}/latest/index.md"))
        .replace(
            "{{详设模板路径}}",
            &format!("{docs_root}/latest/规范约束/详设文档模板.md"),
        )
        .replace(
            "{{进度模板路径}}",
            &format!("{docs_root}/latest/规范约束/开发进度文档模板.md"),
        )
        .replace("{{详设目录}}", &format!("{docs_root}/v0.1/详细设计"))
        .replace(
            "{{分支规范}}",
            &format!("当前分支 `{}` 与用户确认的任务分支", git_branch(root)),
        )
        .replace("{{后端完整验证命令}}", &build)
        .replace("{{lint命令}}", &lint)
        .replace("{{类型检查命令}}", &typecheck)
        .replace("{{test命令}}", &test)
        .replace("{{测试命令}}", &test)
        .replace(
            "{{Maven多模块定向测试说明}}",
            &maven_multi_module_test_guidance(root),
        )
        .replace("{{构建命令}}", &build)
        .replace("{{typecheck命令}}", &typecheck)
        .replace("{{build命令}}", &build)
        .replace("{{项目证据路径}}", &project_evidence(root))
        .replace("{{测试证据路径}}", &test_evidence(root))
        .replace(
            "{{提交规范证据}}",
            ".claude/rules/公共/Git协作与历史保护.md",
        )
        .replace(
            "{{开发基线事实表}}",
            &development_baseline_rows(root, request, layers),
        )
        .replace("{{公共能力事实表}}", &common_capability_rows(root))
        .replace("{{Git事实表}}", &git_rows(root))
        .replace("{{验证命令事实表}}", &verification_rows(root, layers))
        .replace(
            "{{前端工程事实表}}",
            &frontend_engineering_rows(root, request),
        )
        .replace(
            "{{后端工程事实表}}",
            &backend_engineering_rows(root, request),
        )
        .replace("{{持久化事实表}}", &persistence_rows(root, request))
        .replace("{{异步集成事实表}}", &integration_rows(root, request))
        .replace("{{日志来源说明}}", &log_source(root))
        .replace("{{日志能力补充表}}", &log_capability_rows(root))
        .replace(
            "{{数据库证据说明}}",
            &format!(
                "{}；物理模型：`{docs_root}/latest/接口文档/物理模型总览.md`",
                selected_database(request)
            ),
        )
        .replace(
            "{{物理模型文档}}",
            &format!("{docs_root}/latest/接口文档/物理模型总览.md"),
        )
        .replace("{{第三方集成文档}}", "docs/项目需求与技术选型.md")
        .replace("{{本地日志}}", &log_source(root))
        .replace(
            "{{容器日志}}",
            "只有项目提供 Docker/容器访问方式后才使用对应容器日志命令",
        )
        .replace("{{集中日志}}", "只有项目配置并授权集中日志入口后才使用")
        .replace("{{日志规则路径}}", ".claude/rules/公共/事实与兜底边界.md")
        .replace("{{本地日志路径与检索命令}}", &log_source(root))
        .replace(
            "{{容器日志命令或不适用}}",
            "当前项目未配置可验证的容器日志命令，接入后按服务和时间窗口只读检索",
        )
        .replace(
            "{{context/namespace/服务/container 或未配置}}",
            "未配置；只有项目提供并授权 Kubernetes 上下文后才可使用",
        )
        .replace(
            "{{Loki/ELK/Grafana/云日志入口或未配置}}",
            "未配置；只有项目提供并授权集中日志入口后才可使用",
        )
        .replace(
            "{{SSH/API/CLI 入口或未配置}}",
            "未配置；不得假设可访问远程主机或生产环境",
        )
        .replace("{{源码路径}}", &project_evidence(root))
        .replace(
            "{{物理模型文档或不适用}}",
            &format!("{docs_root}/latest/接口文档/物理模型总览.md"),
        )
        .replace(
            "{{持久化规则路径}}",
            if adopted_decision(request, "persistence")
                && !request.recommendation.database.is_empty()
            {
                ".claude/rules/后端/持久化与迁移规则.md"
            } else {
                ".claude/rules/公共/事实与兜底边界.md"
            },
        )
        .replace(
            "{{配置文件/环境变量/配置中心键名，不写值}}",
            &markdown_paths(
                database_config_paths(root),
                "未发现数据库连接配置文件；仅有技术选型时仍需开发者补充",
            ),
        )
        .replace(
            "{{CLI/MCP/脚本/未配置}}",
            "未配置；初始化只登记配置证据，不读取或保存凭证",
        )
        .replace(
            "{{环境 → 可用/有证据但需配置/不适用}}",
            "有证据但需配置；配置键或技术选型尚未经过只读探测",
        )
        .replace(
            "{{数据库能力补充表}}",
            &database_capability_rows(root, request),
        )
        .replace("{{数据访问代码路径}}", &project_evidence(root))
        .replace(
            "{{Flyway/Liquibase/Prisma/其他真实路径}}",
            &project_evidence(root),
        )
        .replace("{{迁移或测试命令}}", &test)
        .replace(
            "{{第三方规则路径}}",
            ".claude/rules/后端/异步与第三方规则.md",
        )
        .replace("{{集成代码路径}}", &project_evidence(root))
        .replace(
            "{{官方文档或 SDK 版本}}",
            "以依赖清单与需求材料可证明的版本为准；未提供官方资料时不得猜测契约",
        )
        .replace(
            "{{初始化时写入当前项目 docs 路径与更新触发条件}}",
            &format!(
                "长期文档位于 `{docs_root}/latest/`；业务、接口、架构或公共能力变化时同步更新。"
            ),
        );
    reject_forbidden_material(&rendered, "渲染结果")?;
    Ok(rendered)
}

/// 将平台内置的 skill-designer 原样安装到项目中。
///
/// 初始化其他项目 skill 之前必须先完成这一步；调用方不得自行改写它的正文、references
/// 或 evals。该函数也供既有项目的 prepare 阶段使用，确保随后启动的 Agent 能立即加载它。
pub(super) fn write_skill_designer(root: &Path) -> Result<(), String> {
    let base = root.join(".claude/skills/skill-designer");
    write_file(&base.join("SKILL.md"), SKILL_DESIGNER)?;
    for (name, content) in [
        ("decision-tree.md", SKILL_DESIGNER_DECISION_TREE),
        ("generator-example.md", SKILL_DESIGNER_GENERATOR),
        ("inversion-example.md", SKILL_DESIGNER_INVERSION),
        ("pipeline-example.md", SKILL_DESIGNER_PIPELINE),
        ("reviewer-example.md", SKILL_DESIGNER_REVIEWER),
        ("tool-wrapper-example.md", SKILL_DESIGNER_TOOL_WRAPPER),
    ] {
        write_file(&base.join("references").join(name), content)?;
    }
    write_file(&base.join("evals/evals.json"), SKILL_DESIGNER_EVALS)
}

fn write_runtime_skills(root: &Path, request: &CreateProjectRequest) -> Result<(), String> {
    let layers = project_layers(root);
    write_skill_designer(root)?;
    for (name, content) in [
        ("detail-design-writer", DETAIL_DESIGN),
        ("review-feedback-handler", REVIEW_FEEDBACK),
        ("code-review", CODE_REVIEW),
        ("developer", DEVELOPER),
        ("problem-diagnose", PROBLEM_DIAGNOSE),
    ] {
        write_skill(
            root,
            name,
            &render_template(content, root, request, layers)?,
        )?;
    }
    if layers.frontend {
        write_skill(
            root,
            "frontend-self-test",
            &render_template_for(
                FRONTEND_SELF_TEST,
                root,
                request,
                layers,
                MaterialLayer::Frontend,
            )?,
        )?;
    }
    if layers.backend {
        write_skill(
            root,
            "backend-self-test",
            &render_template_for(
                BACKEND_SELF_TEST,
                root,
                request,
                layers,
                MaterialLayer::Backend,
            )?,
        )?;
        write_skill(
            root,
            "backend-log-diagnose",
            &render_template_for(
                BACKEND_LOG_DIAGNOSE,
                root,
                request,
                layers,
                MaterialLayer::Backend,
            )?,
        )?;
        if has_database_capability_evidence(root, request) {
            write_skill(
                root,
                "database-read-diagnose",
                &render_template_for(
                    DATABASE_READ_DIAGNOSE,
                    root,
                    request,
                    layers,
                    MaterialLayer::Backend,
                )?,
            )?;
        }
    }
    if layers.backend
        && adopted_decision(request, "persistence")
        && !request.recommendation.database.is_empty()
    {
        write_skill(
            root,
            "ddl-review",
            &render_template_for(DDL_REVIEW, root, request, layers, MaterialLayer::Backend)?,
        )?;
    }
    if layers.backend && adopted_decision(request, "integration") {
        write_skill(
            root,
            "external-integration",
            &render_template_for(
                EXTERNAL_INTEGRATION,
                root,
                request,
                layers,
                MaterialLayer::Backend,
            )?,
        )?;
    }
    Ok(())
}

fn write_runtime_rules(root: &Path, request: &CreateProjectRequest) -> Result<(), String> {
    let layers = project_layers(root);
    let rules = [
        ("公共/开发基线.md", DEVELOPMENT_BASELINE_RULE),
        ("公共/复用与影响面.md", REUSE_AND_IMPACT_RULE),
        ("公共/事实与兜底边界.md", FACT_AND_FALLBACK_RULE),
        ("公共/开发流程与文档同步.md", DEVELOPMENT_FLOW_RULE),
        ("公共/自测与交付.md", SELF_TEST_AND_DELIVERY_RULE),
    ];
    for (name, content) in rules {
        write_rule(
            root,
            name,
            &render_template(content, root, request, layers)?,
        )?;
    }
    if is_git_repository(root) {
        write_rule(
            root,
            "公共/Git协作与历史保护.md",
            &render_template(GIT_COLLABORATION_RULE, root, request, layers)?,
        )?;
    }
    if layers.frontend {
        write_rule(
            root,
            "前端/前端工程规则.md",
            &render_template_for(
                FRONTEND_ENGINEERING_RULE,
                root,
                request,
                layers,
                MaterialLayer::Frontend,
            )?,
        )?;
        write_rule(
            root,
            "前端/前端验证规则.md",
            &render_template_for(
                FRONTEND_VERIFICATION_RULE,
                root,
                request,
                layers,
                MaterialLayer::Frontend,
            )?,
        )?;
    }
    if layers.backend {
        write_rule(
            root,
            "后端/API与业务实现规则.md",
            &render_template_for(
                BACKEND_API_RULE,
                root,
                request,
                layers,
                MaterialLayer::Backend,
            )?,
        )?;
        if adopted_decision(request, "persistence") && !request.recommendation.database.is_empty() {
            write_rule(
                root,
                "后端/持久化与迁移规则.md",
                &render_template_for(
                    BACKEND_PERSISTENCE_RULE,
                    root,
                    request,
                    layers,
                    MaterialLayer::Backend,
                )?,
            )?;
        }
        if (adopted_decision(request, "messaging") && !request.recommendation.messaging.is_empty())
            || adopted_decision(request, "integration")
        {
            write_rule(
                root,
                "后端/异步与第三方规则.md",
                &render_template_for(
                    BACKEND_INTEGRATION_RULE,
                    root,
                    request,
                    layers,
                    MaterialLayer::Backend,
                )?,
            )?;
        }
    }
    let mut index = String::from("# 规则索引\n\n## 所有任务\n\n- `公共/开发基线.md`\n- `公共/复用与影响面.md`\n- `公共/事实与兜底边界.md`\n- `公共/开发流程与文档同步.md`\n- `公共/自测与交付.md`\n- `code/doc-sync-review.md`\n");
    if is_git_repository(root) {
        index.push_str("- `公共/Git协作与历史保护.md`\n");
    }
    if layers.frontend {
        index.push_str("\n## 前端任务\n\n- `前端/前端工程规则.md`\n- `前端/前端验证规则.md`\n");
    }
    if layers.backend {
        index.push_str("\n## 后端任务\n\n- `后端/API与业务实现规则.md`\n");
        if adopted_decision(request, "persistence") && !request.recommendation.database.is_empty() {
            index.push_str("- `后端/持久化与迁移规则.md`\n");
        }
        if (adopted_decision(request, "messaging") && !request.recommendation.messaging.is_empty())
            || adopted_decision(request, "integration")
        {
            index.push_str("- `后端/异步与第三方规则.md`\n");
        }
    }
    write_file(&root.join(".claude/rules/README.md"), &index)
}

fn project_structure(root: &Path, layers: ProjectLayers) -> String {
    let mut rows = Vec::new();
    if layers.frontend {
        rows.push("| 前端源码 | `src/` | 页面、组件与交互；以实际文件为准 |");
    }
    if layers.backend {
        rows.push("| 后端源码 | 当前运行时入口 | HTTP 与业务实现；以构建文件和源码为准 |");
    }
    if root.join("docs").is_dir() {
        rows.push("| 项目文档 | `docs/` | 长期真源、详设与开发进度 |");
    }
    format!("| 模块 | 路径 | 职责 |\n|---|---|---|\n{}", rows.join("\n"))
}

fn entry_document(root: &Path, request: &CreateProjectRequest) -> String {
    let layers = project_layers(root);
    let (test, lint, typecheck, build) = commands(root, layers);
    let mut docs = Vec::new();
    docs.push("- 项目需求与技术选型：`docs/项目需求与技术选型.md`");
    if layers.frontend {
        docs.push("- 前端文档：`docs/frontend/MOC.md`");
    }
    if layers.backend {
        docs.push("- 后端文档：`docs/backend/MOC.md`");
    }
    let mut skills = vec![
        "新需求/详设 → `detail-design-writer`",
        "按详设开发/修复 → `developer`",
        "问题定位/根因分析 → `problem-diagnose`",
        "代码审查 → `code-review`",
        "创建或维护 skill → `skill-designer`",
        "提交前长期文档一致性审核 → `doc-sync-review`",
    ];
    if layers.frontend {
        skills.push("前端自测 → `frontend-self-test`");
    }
    if layers.backend {
        skills.push("后端自测 → `backend-self-test`");
        skills.push("后端日志诊断 → `backend-log-diagnose`");
        if has_database_capability_evidence(root, request) {
            skills.push("数据库只读诊断 → `database-read-diagnose`");
        }
    }
    format!(
        "{PLATFORM_INIT_MARKER}\n# {} — AI 助手开发指南\n\n> {}。主要技术栈：{}。\n> `CLAUDE.md` 是唯一维护源，`AGENTS.md` 软链接到本文件；`.agents/rules`、`.agents/skills`、`.agents/scripts` 软链接到 `.claude/` 同名目录。\n\n## 工厂初始化边界\n\n本项目已完成工程骨架、项目文档、规则与 skills 的初始化。**项目工厂到此结束，不自动开发任何业务功能。** 后续由用户另开 Agent 会话并明确提出需求，再按本文件、rules 和对应 skill 生成详设、进度文档、代码与自测证据。\n\n## 项目结构与模块职责\n\n{}\n\n## 核心约束\n\n1. 改文件前读取目标文件、上游入口、下游调用方、同类实现和命中规则。\n2. 优先复用现有组件、工具、模型、错误处理和测试基座；不存在才新增。\n3. 只改当前任务直接相关内容，不覆盖用户改动，不顺手重构。\n4. 结论必须来自代码、配置、测试、数据或用户材料；证据不足时写“推测”。\n5. 不添加未经需求或项目证据确认的默认值、吞错、模拟成功或降级兜底。\n{}\n\n## 后续会话开发流程\n\n1. 读取“项目需求与技术选型”、业务总览、架构和 `.claude/rules/README.md`。\n2. 新需求先使用 `detail-design-writer` 生成详设与进度；用户确认前不改业务代码。\n3. 用户明确要求开发后才使用 `developer`，先写失败测试，再做最小实现。\n4. 覆盖正常、边界、异常、原 Bug 和相关回归，同步受影响长期文档。\n5. 提交前使用 `doc-sync-review` 审核暂存区并记录当前指纹；是否 commit/push 仍由用户选择。\n\n## 文档索引\n\n{}\n\n## 技能触发\n\n- {}\n\n## 构建与自测\n\n| 用途 | 真实命令 |\n|---|---|\n| 测试 | `{}` |\n| lint | `{}` |\n| 类型检查 | `{}` |\n| 构建 | `{}` |\n",
        request.project_name,
        request.profile.summary,
        stack_summary(request, layers),
        project_structure(root, layers),
        if is_git_repository(root) {
            "6. 提交和推送由用户选择，未经授权不执行。"
        } else {
            "6. 当前目录未检测到 Git 元数据，不生成或假设分支、commit、push 流程。"
        },
        docs.join("\n"),
        skills.join("\n- "),
        test,
        lint,
        typecheck,
        build
    )
}

fn remove_existing_link_or_copy(path: &Path) -> Result<(), String> {
    let Ok(metadata) = fs::symlink_metadata(path) else {
        return Ok(());
    };
    if metadata.file_type().is_dir() && !metadata.file_type().is_symlink() {
        fs::remove_dir_all(path).map_err(|error| error.to_string())
    } else {
        fs::remove_file(path).map_err(|error| error.to_string())
    }
}

fn write_shared_entrypoints(root: &Path, entry: &str) -> Result<String, String> {
    write_file(&root.join("CLAUDE.md"), entry)?;
    let agents_entry = root.join("AGENTS.md");
    remove_existing_link_or_copy(&agents_entry)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        if symlink("CLAUDE.md", &agents_entry).is_ok() {
            return Ok("symlink".to_string());
        }
    }
    write_file(&agents_entry, entry)?;
    Ok("copy".to_string())
}

fn copy_dir_contents(source: &Path, destination: &Path) -> Result<(), String> {
    fs::create_dir_all(destination).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(source).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let from = entry.path();
        let to = destination.join(entry.file_name());
        if from.is_dir() {
            copy_dir_contents(&from, &to)?;
        } else {
            fs::copy(&from, &to).map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

fn link_or_copy_shared_dir(root: &Path, dir: &str) -> Result<bool, String> {
    let agents_dir = root.join(".agents");
    fs::create_dir_all(&agents_dir).map_err(|error| error.to_string())?;
    let destination = agents_dir.join(dir);
    remove_existing_link_or_copy(&destination)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        if symlink(format!("../.claude/{dir}"), &destination).is_ok() {
            return Ok(true);
        }
    }
    copy_dir_contents(&root.join(".claude").join(dir), &destination)?;
    Ok(false)
}

pub fn write_ai_rules(root: &Path, request: &CreateProjectRequest) -> Result<String, String> {
    write_runtime_rules(root, request)?;
    write_runtime_skills(root, request)?;
    install_doc_sync_review(root, project_layers(root))?;
    validate_generated_materials(root)?;
    write_file(
        &root.join(".claude/scripts/README.md"),
        "# 项目脚本\n\n只保存当前项目反复使用且已验证的确定性脚本；没有脚本时保持本目录说明，不创建空工具。\n",
    )?;
    write_file(
        &root.join(".claude/settings.json"),
        "{\n  \"permissions\": {\n    \"defaultMode\": \"acceptEdits\"\n  }\n}\n",
    )?;

    let entry = entry_document(root, request);
    reject_forbidden_material(&entry, "CLAUDE.md")?;
    let entry_mode = write_shared_entrypoints(root, &entry)?;
    let rules_linked = link_or_copy_shared_dir(root, "rules")?;
    let skills_linked = link_or_copy_shared_dir(root, "skills")?;
    let scripts_linked = link_or_copy_shared_dir(root, "scripts")?;
    write_file(
        &root.join(".agents/CODEX.md"),
        "遵守根目录 `AGENTS.md`；规则、技能和脚本与 `.claude/` 共用同一来源。创建或修改 skill 必须先使用 `skill-designer`。\n",
    )?;

    if rules_linked && skills_linked && scripts_linked && entry_mode == "symlink" {
        Ok("shared-symlink".to_string())
    } else {
        Ok("shared-copy-fallback".to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};

    use super::{
        reject_forbidden_material, render_template_for, write_runtime_rules, write_runtime_skills,
        MaterialLayer, BACKEND_LOG_DIAGNOSE, BACKEND_SELF_TEST,
    };
    use crate::project_factory::{
        CreateProjectRequest, ProjectProfilePayload, StackRecommendationPayload, TechnologyDecision,
    };

    fn temporary_project(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "vibe-ai-rules-{name}-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        if root.exists() {
            fs::remove_dir_all(&root).expect("remove stale test project");
        }
        fs::create_dir_all(&root).expect("create test project");
        root
    }

    fn maven_backend_request(root: &Path) -> CreateProjectRequest {
        CreateProjectRequest {
            project_name: "demo-service".to_string(),
            parent_path: root.to_string_lossy().to_string(),
            recommendation: StackRecommendationPayload {
                backend: vec!["Java 21".to_string(), "Spring Boot".to_string()],
                database: vec!["MySQL".to_string()],
                decisions: vec![TechnologyDecision {
                    category: "persistence".to_string(),
                    title: "关系型持久化".to_string(),
                    status: "adopt".to_string(),
                    choices: vec!["MySQL".to_string()],
                    reason: "业务需要持久化".to_string(),
                    provision: "project".to_string(),
                    trigger: None,
                }],
                ..Default::default()
            },
            profile: ProjectProfilePayload {
                summary: "后端服务".to_string(),
                system_type: "backend-api".to_string(),
            },
            agent_choice: "both".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn generated_material_rejects_unresolved_placeholders() {
        let error = reject_forbidden_material("使用 {{测试命令}} 执行", "skill")
            .expect_err("unresolved token must fail initialization");
        assert!(error.contains("{{"));
    }

    #[test]
    fn generated_material_rejects_fake_fillers() {
        assert!(reject_forbidden_material("待填写", "rule").is_err());
        assert!(reject_forbidden_material("初始化扫描未发现对应证据", "rule").is_err());
    }

    #[test]
    fn generated_material_accepts_honest_unavailable_capability() {
        reject_forbidden_material("package.json 未定义测试脚本", "skill")
            .expect("honest unavailable capability is executable guidance");
    }

    #[test]
    fn maven_backend_self_test_handles_multi_module_targeted_tests() {
        let root = temporary_project("maven-self-test");
        fs::write(root.join("pom.xml"), "<project><modules/></project>").expect("write pom");
        let request = maven_backend_request(&root);
        let rendered = render_template_for(
            BACKEND_SELF_TEST,
            &root,
            &request,
            crate::project_factory::docs::project_layers(&root),
            MaterialLayer::Backend,
        )
        .expect("render backend self-test");

        assert!(rendered.contains("-Dsurefire.failIfNoSpecifiedTests=false"));
        fs::remove_dir_all(root).expect("cleanup test project");
    }

    #[test]
    fn backend_log_skill_requires_developer_completion_before_available() {
        assert!(BACKEND_LOG_DIAGNOSE.contains("## 待开发者补充"));
        assert!(BACKEND_LOG_DIAGNOSE.contains("## 完成 Gate"));
        assert!(BACKEND_LOG_DIAGNOSE.contains("最小只读探测"));
        assert!(BACKEND_LOG_DIAGNOSE.contains("配置键"));
    }

    #[test]
    fn database_skill_is_generated_but_not_available_without_probe() {
        let root = temporary_project("database-skill");
        fs::write(
            root.join("pom.xml"),
            "<project><dependencies><mysql-connector-j/></dependencies></project>",
        )
        .expect("write pom");
        fs::write(
            root.join("application.yml"),
            "spring:\n  datasource:\n    url: ${DB_URL}\n",
        )
        .expect("write datasource config");
        let request = maven_backend_request(&root);

        write_runtime_skills(&root, &request).expect("write backend skills");
        let skill = fs::read_to_string(root.join(".claude/skills/database-read-diagnose/SKILL.md"))
            .expect("database skill must be generated");

        assert!(skill.contains("## 待开发者补充"));
        assert!(skill.contains("## 完成 Gate"));
        assert!(skill.contains("有证据但需配置"));
        assert!(skill.contains("SELECT 1"));
        assert!(skill.contains("配置键不等于能力可用"));
        fs::remove_dir_all(root).expect("cleanup test project");
    }

    #[test]
    fn database_skill_does_not_reference_an_unselected_persistence_rule() {
        let root = temporary_project("database-rule-fallback");
        fs::write(
            root.join("pom.xml"),
            "<project><dependencies><mysql-connector-j/></dependencies></project>",
        )
        .expect("write pom");
        let mut request = maven_backend_request(&root);
        request.recommendation.decisions.clear();

        write_runtime_skills(&root, &request).expect("write backend skills");
        let skill = fs::read_to_string(root.join(".claude/skills/database-read-diagnose/SKILL.md"))
            .expect("database skill must be generated");

        assert!(skill.contains(".claude/rules/公共/事实与兜底边界.md"));
        assert!(!skill.contains(".claude/rules/后端/持久化与迁移规则.md"));
        fs::remove_dir_all(root).expect("cleanup test project");
    }

    #[test]
    fn generated_rules_reference_real_framework_and_shared_source_files() {
        let root = temporary_project("real-project-evidence");
        fs::create_dir_all(root.join("src/main/java/demo/common")).expect("create common dir");
        fs::create_dir_all(root.join("src/main/java/demo/domain")).expect("create domain dir");
        fs::create_dir_all(root.join("src/main/java/demo/web")).expect("create web dir");
        fs::write(
            root.join("pom.xml"),
            "<project><parent><artifactId>spring-boot-starter-parent</artifactId></parent></project>",
        )
        .expect("write pom");
        fs::write(root.join(".editorconfig"), "root = true\n").expect("write style config");
        fs::write(
            root.join("src/main/java/demo/common/MoneyUtils.java"),
            "final class MoneyUtils {}",
        )
        .expect("write utility");
        fs::write(
            root.join("src/main/java/demo/domain/OrderStatusEnum.java"),
            "enum OrderStatusEnum { CREATED }",
        )
        .expect("write enum");
        fs::write(
            root.join("src/main/java/demo/web/GlobalExceptionHandler.java"),
            "final class GlobalExceptionHandler {}",
        )
        .expect("write exception handler");
        let request = maven_backend_request(&root);

        write_runtime_rules(&root, &request).expect("write project rules");
        let reuse = fs::read_to_string(root.join(".claude/rules/公共/复用与影响面.md"))
            .expect("read reuse rule");
        let backend = fs::read_to_string(root.join(".claude/rules/后端/API与业务实现规则.md"))
            .expect("read backend rule");
        let baseline = fs::read_to_string(root.join(".claude/rules/公共/开发基线.md"))
            .expect("read baseline rule");

        assert!(reuse.contains("src/main/java/demo/common/MoneyUtils.java"));
        assert!(reuse.contains("src/main/java/demo/domain/OrderStatusEnum.java"));
        assert!(backend.contains("src/main/java/demo/web/GlobalExceptionHandler.java"));
        assert!(backend.contains("Spring Boot"));
        assert!(baseline.contains(".editorconfig"));
        fs::remove_dir_all(root).expect("cleanup test project");
    }

    #[test]
    fn frontend_rules_reference_real_routes_components_state_and_clients() {
        let root = temporary_project("frontend-evidence");
        for directory in ["src/router", "src/components", "src/stores", "src/api"] {
            fs::create_dir_all(root.join(directory)).expect("create frontend source dir");
        }
        fs::write(
            root.join("package.json"),
            r#"{"dependencies":{"vue":"^3.5.0","vue-router":"^4.0.0","pinia":"^3.0.0","axios":"^1.0.0"},"scripts":{"test":"vitest","build":"vite build"}}"#,
        )
        .expect("write package");
        fs::write(root.join("src/App.vue"), "<template />").expect("write app");
        fs::write(root.join("src/router/index.ts"), "export const router = {}")
            .expect("write router");
        fs::write(root.join("src/components/BaseButton.vue"), "<template />")
            .expect("write component");
        fs::write(
            root.join("src/stores/user.ts"),
            "export const useUserStore = () => ({})",
        )
        .expect("write store");
        fs::write(root.join("src/api/client.ts"), "export const client = {}")
            .expect("write client");
        let request = CreateProjectRequest {
            project_name: "demo-web".to_string(),
            parent_path: root.to_string_lossy().to_string(),
            recommendation: StackRecommendationPayload {
                frontend: vec!["Vue 3".to_string()],
                ..Default::default()
            },
            profile: ProjectProfilePayload {
                summary: "前端应用".to_string(),
                system_type: "web".to_string(),
            },
            agent_choice: "both".to_string(),
            ..Default::default()
        };

        write_runtime_rules(&root, &request).expect("write frontend rules");
        let frontend = fs::read_to_string(root.join(".claude/rules/前端/前端工程规则.md"))
            .expect("read frontend rule");

        for expected in [
            "src/router/index.ts",
            "src/components/BaseButton.vue",
            "src/stores/user.ts",
            "src/api/client.ts",
        ] {
            assert!(frontend.contains(expected), "missing {expected}");
        }
        assert!(frontend.contains("Vue 3"));
        fs::remove_dir_all(root).expect("cleanup test project");
    }
}
