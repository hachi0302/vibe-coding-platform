use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::types::{
    ArtifactKind, ArtifactPlan, ArtifactPlanItem, ArtifactTotals, ProjectInventory, ValidationIssue,
};

const PLAN_PATH: &str = ".vibe-coding-platform/artifact-plan.json";
const COMMON_DOCUMENT_IDS: &[&str] = &[
    "project-map",
    "architecture-boundaries",
    "reusable-assets",
    "verification-playbook",
    "known-risks",
];
const GENERIC_SKILL_PHRASES: &[&str] = &[
    "developer",
    "development-workflow",
    "problem-diagnose",
    "code-review",
    "review-feedback-handler",
    "worktree",
    "skill-designer",
];
const GENERIC_SKILL_WORDS: &[&str] = &[
    "code",
    "coding",
    "debug",
    "debugger",
    "debugging",
    "designer",
    "development",
    "diagnose",
    "diagnosis",
    "feedback",
    "general",
    "generic",
    "handler",
    "problem",
    "review",
    "skill",
    "troubleshooting",
    "workflow",
];
const GENERIC_RULE_PHRASES: &[&str] = &[
    "backend-engineering",
    "frontend-engineering",
    "backend-development",
    "frontend-development",
    "development-baseline",
    "coding-guidelines",
    "coding-rules",
    "engineering-standards",
    "general-rules",
    "general-best-practices",
];
const GENERIC_RULE_WORDS: &[&str] = &[
    "backend",
    "baseline",
    "best",
    "code",
    "coding",
    "development",
    "engineering",
    "frontend",
    "general",
    "guideline",
    "guidelines",
    "practice",
    "practices",
    "rule",
    "rules",
    "standard",
    "standards",
];
const FRONTEND_TOPIC_WORDS: &[&str] = &[
    "angular",
    "component",
    "components",
    "composable",
    "composables",
    "directive",
    "directives",
    "frontend",
    "layout",
    "layouts",
    "nextjs",
    "nuxt",
    "react",
    "svelte",
    "ui",
    "vue",
];
const FRONTEND_TOPIC_PHRASES: &[&str] = &["design-system", "state-flow"];
const BACKEND_TOPIC_WORDS: &[&str] = &[
    "backend",
    "controller",
    "database",
    "flyway",
    "migration",
    "migrations",
    "persistence",
    "repository",
    "server",
    "spring",
];

fn issue(
    code: impl Into<String>,
    detail: impl Into<String>,
    path: Option<&str>,
    stage: &str,
) -> ValidationIssue {
    ValidationIssue {
        code: code.into(),
        detail: detail.into(),
        path: path.map(str::to_string),
        stage: Some(stage.to_string()),
    }
}

fn normalized_relative_path(value: &str) -> Result<&Path, &'static str> {
    if value.is_empty() || value.contains('\\') {
        return Err("invalid");
    }
    let path = Path::new(value);
    if path.is_absolute() {
        return Err("traversal");
    }
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err("traversal");
        }
    }
    Ok(path)
}

fn is_kebab_component(component: &str) -> bool {
    if matches!(component, ".claude" | "README.md" | "SKILL.md") {
        return true;
    }
    if !component.is_ascii() {
        return false;
    }
    let stem = component.strip_suffix(".md").unwrap_or(component);
    !stem.is_empty()
        && !stem.starts_with('-')
        && !stem.ends_with('-')
        && !stem.contains("--")
        && stem
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn allowed_target(item: &ArtifactPlanItem) -> bool {
    let path = item.target_path.as_str();
    match item.kind {
        ArtifactKind::Document => path.starts_with("docs/ai/") && path.ends_with(".md"),
        ArtifactKind::Rule => path.starts_with(".claude/rules/project/") && path.ends_with(".md"),
        ArtifactKind::Skill => {
            path.starts_with(".claude/skills/")
                && path.ends_with("/SKILL.md")
                && path.split('/').count() >= 4
        }
    }
}

fn evidence_path_exists(workspace: &Path, inventory: &ProjectInventory, path: &str) -> bool {
    let Ok(relative) = normalized_relative_path(path) else {
        return false;
    };
    inventory.files.iter().any(|file| file.path == path)
        && fs::symlink_metadata(workspace.join(relative))
            .map(|metadata| metadata.file_type().is_file() && !metadata.file_type().is_symlink())
            .unwrap_or(false)
}

fn evidence_symbol_exists(workspace: &Path, path: &str, symbol: &str) -> bool {
    normalized_relative_path(path)
        .ok()
        .and_then(|relative| fs::read_to_string(workspace.join(relative)).ok())
        .map(|content| content.contains(symbol))
        .unwrap_or(false)
}

fn validate_evidence_reference(
    workspace: &Path,
    inventory: &ProjectInventory,
    evidence: &super::types::EvidenceReference,
    code_prefix: &str,
    target_path: Option<&str>,
    issues: &mut Vec<ValidationIssue>,
) -> bool {
    let mut valid = true;
    if evidence.path.trim().is_empty() {
        issues.push(issue(
            format!("{code_prefix}.path-empty"),
            "证据路径不能为空",
            target_path,
            "plan",
        ));
        valid = false;
    } else if !evidence_path_exists(workspace, inventory, &evidence.path) {
        issues.push(issue(
            format!("{code_prefix}.missing"),
            format!("证据路径不存在：{}", evidence.path),
            target_path,
            "plan",
        ));
        valid = false;
    }

    let symbol = evidence
        .symbol
        .as_deref()
        .map(str::trim)
        .unwrap_or_default();
    if symbol.is_empty() {
        issues.push(issue(
            format!("{code_prefix}.symbol-empty"),
            "证据必须声明非空源码符号或配置键",
            target_path,
            "plan",
        ));
        valid = false;
    } else if !evidence.path.trim().is_empty()
        && evidence_path_exists(workspace, inventory, &evidence.path)
        && !evidence_symbol_exists(workspace, &evidence.path, symbol)
    {
        issues.push(issue(
            format!("{code_prefix}.symbol-missing"),
            format!("证据符号不存在：{}#{symbol}", evidence.path),
            target_path,
            "plan",
        ));
        valid = false;
    }
    valid
}

fn path_is_within(parent: &str, child: &str) -> bool {
    parent == "." || child == parent || child.starts_with(&format!("{parent}/"))
}

fn coverage_target_known(inventory: &ProjectInventory, target: &str) -> bool {
    inventory
        .modules
        .iter()
        .any(|module| module.name == target || module.path == target)
        || inventory.source_roots.iter().any(|root| root == target)
}

fn evidence_relates_to_coverage(
    inventory: &ProjectInventory,
    evidence_path: &str,
    target: &str,
) -> bool {
    let module_match = inventory.modules.iter().any(|module| {
        if module.name != target && module.path != target {
            return false;
        }
        path_is_within(&module.path, evidence_path)
            || inventory.files.iter().any(|file| {
                file.path == evidence_path
                    && file
                        .module
                        .as_deref()
                        .is_some_and(|owner| owner == module.path || owner == module.name)
            })
    });
    module_match
        || inventory
            .source_roots
            .iter()
            .any(|root| root == target && path_is_within(root, evidence_path))
}

fn identifier_words(value: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut previous_lowercase = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            if character.is_ascii_uppercase() && previous_lowercase && !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            current.push(character.to_ascii_lowercase());
            previous_lowercase = character.is_ascii_lowercase();
        } else {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            previous_lowercase = false;
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    words
}

fn normalized_identifier(value: &str) -> String {
    identifier_words(value).join("-")
}

fn contains_identifier_phrase(value: &str, phrase: &str) -> bool {
    let normalized = normalized_identifier(value);
    normalized == phrase
        || normalized.starts_with(&format!("{phrase}-"))
        || normalized.ends_with(&format!("-{phrase}"))
        || normalized.contains(&format!("-{phrase}-"))
}

fn artifact_mentions_any(item: &ArtifactPlanItem, words: &[&str]) -> bool {
    [&item.id, &item.topic, &item.target_path]
        .into_iter()
        .flat_map(|value| identifier_words(value))
        .any(|word| words.contains(&word.as_str()))
}

fn artifact_mentions_phrase(item: &ArtifactPlanItem, phrases: &[&str]) -> bool {
    [
        item.id.as_str(),
        item.topic.as_str(),
        item.target_path.as_str(),
    ]
    .into_iter()
    .any(|value| {
        phrases
            .iter()
            .any(|phrase| contains_identifier_phrase(value, phrase))
    })
}

fn generic_rule(item: &ArtifactPlanItem) -> bool {
    let values = [item.id.as_str(), item.topic.as_str()];
    values.iter().any(|value| {
        GENERIC_RULE_PHRASES
            .iter()
            .any(|phrase| contains_identifier_phrase(value, phrase))
    }) || {
        let words = values
            .into_iter()
            .flat_map(identifier_words)
            .collect::<Vec<_>>();
        !words.is_empty()
            && words
                .iter()
                .all(|word| GENERIC_RULE_WORDS.contains(&word.as_str()))
    }
}

fn generic_skill(item: &ArtifactPlanItem) -> bool {
    let skill_name = item
        .target_path
        .strip_prefix(".claude/skills/")
        .and_then(|rest| rest.split('/').next())
        .unwrap_or_default();
    let values = [item.id.as_str(), item.topic.as_str(), skill_name];
    values.into_iter().any(|value| {
        GENERIC_SKILL_PHRASES
            .iter()
            .any(|phrase| contains_identifier_phrase(value, phrase))
    }) || {
        let words = values
            .into_iter()
            .flat_map(identifier_words)
            .collect::<Vec<_>>();
        !words.is_empty()
            && words
                .iter()
                .all(|word| GENERIC_SKILL_WORDS.contains(&word.as_str()))
    }
}

pub fn read_artifact_plan(workspace: &Path) -> Result<ArtifactPlan, Vec<ValidationIssue>> {
    let path = workspace.join(PLAN_PATH);
    let content = fs::read_to_string(&path).map_err(|error| {
        vec![issue(
            "plan.json.missing",
            format!("无法读取产物计划：{error}"),
            Some(PLAN_PATH),
            "plan",
        )]
    })?;
    serde_json::from_str(&content).map_err(|error| {
        vec![issue(
            "plan.json.invalid",
            format!("产物计划 JSON 无法解析：{error}"),
            Some(PLAN_PATH),
            "plan",
        )]
    })
}

pub fn validate_artifact_plan(
    workspace: &Path,
    inventory: &ProjectInventory,
    plan: &ArtifactPlan,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    if plan.schema_version != 1 {
        issues.push(issue(
            "plan.schema.unsupported",
            format!("只支持 schemaVersion=1，实际为 {}", plan.schema_version),
            None,
            "plan",
        ));
    }
    if plan.project_name != inventory.project_name {
        issues.push(issue(
            "plan.project-name.mismatch",
            "产物计划项目名与扫描结果不一致",
            None,
            "plan",
        ));
    }

    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for item in &plan.artifacts {
        if !ids.insert(item.id.clone()) {
            issues.push(issue(
                "plan.id.duplicate",
                format!("重复逻辑 ID：{}", item.id),
                Some(&item.target_path),
                "plan",
            ));
        }
        if !paths.insert(item.target_path.clone()) {
            issues.push(issue(
                "plan.path.duplicate",
                "多个产物使用了同一路径",
                Some(&item.target_path),
                "plan",
            ));
        }
        let path = match normalized_relative_path(&item.target_path) {
            Ok(path) => Some(path),
            Err("traversal") => {
                issues.push(issue(
                    "plan.path.traversal",
                    "产物路径包含绝对路径或目录穿越",
                    Some(&item.target_path),
                    "plan",
                ));
                None
            }
            Err(_) => {
                issues.push(issue(
                    "plan.path.invalid",
                    "产物路径格式无效",
                    Some(&item.target_path),
                    "plan",
                ));
                None
            }
        };
        if let Some(path) = path {
            if path
                .components()
                .filter_map(|component| component.as_os_str().to_str())
                .any(|component| !is_kebab_component(component))
            {
                issues.push(issue(
                    "plan.path.not-kebab-case",
                    "新产物路径必须使用 ASCII kebab-case",
                    Some(&item.target_path),
                    "plan",
                ));
            }
        }
        if !allowed_target(item) {
            issues.push(issue(
                "plan.path.outside-allowlist",
                "产物路径不在该类型允许的安装根目录",
                Some(&item.target_path),
                "plan",
            ));
        }
        if item.rationale.trim().chars().count() < 8 {
            issues.push(issue(
                "plan.rationale.insufficient",
                "产物缺少项目化生成理由",
                Some(&item.target_path),
                "plan",
            ));
        }
        if item.evidence.is_empty() {
            issues.push(issue(
                "plan.evidence.empty",
                "产物没有真实证据",
                Some(&item.target_path),
                "plan",
            ));
        }
        if item.required_sections.is_empty() {
            issues.push(issue(
                "plan.sections.empty",
                "产物计划必须声明可校验的必需章节",
                Some(&item.target_path),
                "plan",
            ));
        }
        let valid_evidence = item
            .evidence
            .iter()
            .filter(|evidence| {
                validate_evidence_reference(
                    workspace,
                    inventory,
                    evidence,
                    "plan.evidence",
                    Some(&item.target_path),
                    &mut issues,
                )
            })
            .collect::<Vec<_>>();
        for target in &item.covers {
            if !coverage_target_known(inventory, target) {
                issues.push(issue(
                    "plan.coverage.target.invalid",
                    format!("覆盖目标不在项目清单中：{target}"),
                    Some(&item.target_path),
                    "plan",
                ));
            } else if valid_evidence
                .iter()
                .any(|evidence| evidence_relates_to_coverage(inventory, &evidence.path, target))
            {
                covered.insert(target.clone());
            } else {
                issues.push(issue(
                    "plan.coverage.evidence-unrelated",
                    format!("覆盖目标缺少同模块或源码根证据：{target}"),
                    Some(&item.target_path),
                    "plan",
                ));
            }
        }

        let declared_layer_valid = match item.layer.as_str() {
            "common" | "contract" => true,
            "frontend" => inventory.layers.frontend,
            "backend" | "database" | "integration" => inventory.layers.backend,
            _ => false,
        };
        let frontend_topic = artifact_mentions_any(item, FRONTEND_TOPIC_WORDS)
            || artifact_mentions_phrase(item, FRONTEND_TOPIC_PHRASES);
        let topic_valid = (!frontend_topic || inventory.layers.frontend)
            && (!artifact_mentions_any(item, BACKEND_TOPIC_WORDS) || inventory.layers.backend);
        if !declared_layer_valid || !topic_valid {
            issues.push(issue(
                "plan.layer.mismatch",
                format!("产物层级与项目不匹配：{}", item.layer),
                Some(&item.target_path),
                "plan",
            ));
        }
        if item.kind == ArtifactKind::Rule && generic_rule(item) {
            issues.push(issue(
                "plan.rule.generic",
                "规则主题过于泛化，必须绑定真实项目责任或工作流",
                Some(&item.target_path),
                "plan",
            ));
        }
        if item.kind == ArtifactKind::Skill && generic_skill(item) {
            issues.push(issue(
                "plan.skill.generic",
                "通用平台能力不得复制为项目 skill",
                Some(&item.target_path),
                "plan",
            ));
        }
    }

    let mut exclusions = BTreeSet::new();
    for exclusion in &plan.exclusions {
        let mut valid = true;
        if !coverage_target_known(inventory, &exclusion.target) {
            issues.push(issue(
                "plan.exclusion.target.invalid",
                format!("排除目标不在项目清单中：{}", exclusion.target),
                None,
                "plan",
            ));
            valid = false;
        }
        if exclusion.reason.trim().chars().count() < 8 {
            issues.push(issue(
                "plan.exclusion.reason.insufficient",
                "覆盖排除必须说明可审查的项目化原因",
                None,
                "plan",
            ));
            valid = false;
        }
        if exclusion.evidence.is_empty() {
            issues.push(issue(
                "plan.exclusion.evidence.empty",
                "覆盖排除必须提供真实路径和符号证据",
                None,
                "plan",
            ));
            valid = false;
        }
        let valid_evidence = exclusion
            .evidence
            .iter()
            .filter(|evidence| {
                validate_evidence_reference(
                    workspace,
                    inventory,
                    evidence,
                    "plan.exclusion.evidence",
                    None,
                    &mut issues,
                )
            })
            .collect::<Vec<_>>();
        if valid_evidence.len() != exclusion.evidence.len() {
            valid = false;
        }
        if valid
            && !valid_evidence.iter().all(|evidence| {
                evidence_relates_to_coverage(inventory, &evidence.path, &exclusion.target)
            })
        {
            issues.push(issue(
                "plan.exclusion.evidence.unrelated",
                "覆盖排除证据不属于被排除的模块或源码根",
                None,
                "plan",
            ));
            valid = false;
        }
        if valid {
            exclusions.insert(exclusion.target.as_str());
        }
    }
    for module in &inventory.modules {
        if !covered.contains(&module.name)
            && !covered.contains(&module.path)
            && !exclusions.contains(module.name.as_str())
            && !exclusions.contains(module.path.as_str())
        {
            issues.push(issue(
                "plan.module.uncovered",
                format!("模块未被产物覆盖：{}", module.path),
                None,
                "plan",
            ));
        }
    }
    for source_root in &inventory.source_roots {
        if !covered.contains(source_root) && !exclusions.contains(source_root.as_str()) {
            issues.push(issue(
                "plan.source-root.uncovered",
                format!("源码根未被产物覆盖：{source_root}"),
                None,
                "plan",
            ));
        }
    }
    if !plan.artifacts.iter().any(|item| {
        item.kind == ArtifactKind::Rule && item.target_path == ".claude/rules/project/README.md"
    }) {
        issues.push(issue(
            "plan.rule-router.missing",
            "缺少项目规则触发路由 README.md",
            None,
            "plan",
        ));
    }
    let existing_ids = plan
        .artifacts
        .iter()
        .filter(|item| item.kind == ArtifactKind::Document)
        .map(|item| item.id.as_str())
        .collect::<BTreeSet<_>>();
    for required in COMMON_DOCUMENT_IDS {
        if !existing_ids.contains(required) {
            issues.push(issue(
                "plan.common-document.missing",
                format!("缺少基础项目文档：{required}"),
                None,
                "plan",
            ));
        }
    }
    issues
}

pub fn artifact_totals(plan: &ArtifactPlan) -> ArtifactTotals {
    let mut totals = ArtifactTotals::default();
    for artifact in &plan.artifacts {
        match artifact.kind {
            ArtifactKind::Document => totals.documents += 1,
            ArtifactKind::Rule => totals.rules += 1,
            ArtifactKind::Skill => totals.skills += 1,
        }
    }
    totals.total = totals.documents + totals.rules + totals.skills;
    totals
}

fn chinese_count(content: &str) -> usize {
    content
        .chars()
        .filter(|character| ('\u{4e00}'..='\u{9fff}').contains(character))
        .count()
}

fn markdown_links(content: &str) -> Vec<&str> {
    let mut links = Vec::new();
    let mut remaining = content;
    while let Some(start) = remaining.find("](") {
        remaining = &remaining[start + 2..];
        let Some(end) = remaining.find(')') else {
            break;
        };
        links.push(remaining[..end].trim());
        remaining = &remaining[end + 1..];
    }
    links
}

fn local_markdown_link(link: &str) -> Option<&str> {
    let link = link.trim().trim_matches(['<', '>']);
    if link.starts_with('#')
        || link.starts_with("http://")
        || link.starts_with("https://")
        || link.starts_with("mailto:")
    {
        return None;
    }
    let path = link.split(['#', '?']).next().unwrap_or_default().trim();
    (!path.is_empty() && path.ends_with(".md")).then_some(path)
}

fn resolve_artifact_relative_path(artifact_path: &str, target: &str) -> Option<PathBuf> {
    if target.contains('\\') || Path::new(target).is_absolute() {
        return None;
    }
    let mut resolved = PathBuf::new();
    for component in Path::new(artifact_path).parent()?.components() {
        let Component::Normal(component) = component else {
            return None;
        };
        resolved.push(component);
    }
    for component in Path::new(target).components() {
        match component {
            Component::Normal(component) => resolved.push(component),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return None;
                }
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(resolved)
}

fn link_exists(
    workspace: &Path,
    artifact_path: &str,
    link: &str,
    planned_paths: &BTreeSet<&str>,
) -> bool {
    let Some(link) = local_markdown_link(link) else {
        return true;
    };
    let Some(relative) = resolve_artifact_relative_path(artifact_path, link) else {
        return false;
    };
    workspace.join(&relative).is_file()
        || relative
            .to_str()
            .is_some_and(|path| planned_paths.contains(path))
}

fn contains_secret_assignment(content: &str) -> bool {
    const KEYS: &[&str] = &[
        "password",
        "passwd",
        "secret",
        "token",
        "api-key",
        "api_key",
        "private-key",
        "private_key",
        "authorization",
    ];
    content.lines().any(|line| {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.starts_with("//") {
            return false;
        }
        let lower = trimmed.to_ascii_lowercase();
        let Some(separator) = lower.find(['=', ':']) else {
            return false;
        };
        let key = lower[..separator].trim_matches(|character: char| {
            !character.is_ascii_alphanumeric() && character != '-' && character != '_'
        });
        let value = trimmed[separator + 1..].trim().trim_matches(['"', '\'']);
        KEYS.iter()
            .any(|candidate| key == *candidate || key.ends_with(candidate))
            && !value.is_empty()
            && !matches!(value, "[REDACTED]" | "<redacted>" | "***")
    })
}

fn command_candidate(value: &str) -> Option<&str> {
    let value = value.trim();
    let value = value
        .strip_prefix("$ ")
        .or_else(|| value.strip_prefix("> "))
        .unwrap_or(value)
        .trim();
    [
        "npm ", "pnpm ", "yarn ", "cargo ", "mvn ", "gradle ", "go ", "pytest", "python ",
    ]
    .iter()
    .any(|prefix| value.starts_with(prefix))
    .then_some(value)
}

fn inline_command_candidates<'a>(line: &'a str, commands: &mut Vec<&'a str>) {
    let mut remaining = line;
    while let Some(start) = remaining.find('`') {
        remaining = &remaining[start + 1..];
        let Some(end) = remaining.find('`') else {
            break;
        };
        if let Some(candidate) = command_candidate(&remaining[..end]) {
            commands.push(candidate);
        }
        remaining = &remaining[end + 1..];
    }
}

fn command_candidates(content: &str) -> Vec<&str> {
    let mut commands = Vec::new();
    let mut shell_fence = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if shell_fence.is_some() {
                shell_fence = None;
            } else {
                let language = trimmed.trim_start_matches('`').trim().to_ascii_lowercase();
                shell_fence = Some(matches!(
                    language.as_str(),
                    "" | "bash"
                        | "sh"
                        | "shell"
                        | "zsh"
                        | "console"
                        | "terminal"
                        | "powershell"
                        | "ps1"
                        | "cmd"
                ));
            }
            continue;
        }
        if let Some(is_shell) = shell_fence {
            if is_shell {
                if let Some(candidate) = command_candidate(trimmed) {
                    commands.push(candidate);
                }
            }
        } else {
            inline_command_candidates(line, &mut commands);
        }
    }
    commands
}

fn evidence_is_cited_together(content: &str, path: &str, symbol: &str) -> bool {
    let mut path_seen = false;
    let mut symbol_seen = false;
    for line in content.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            if path_seen && symbol_seen {
                return true;
            }
            path_seen = false;
            symbol_seen = false;
        } else {
            path_seen |= line.contains(path);
            symbol_seen |= line.replace(path, "").contains(symbol);
        }
    }
    false
}

fn content_has_rule_contract(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    (lower.contains("paths:") || content.contains("触发") || content.contains("关键词"))
        && content.contains("复用")
        && (content.contains("禁止") || content.contains("不得"))
        && content.contains("影响")
        && (content.contains("验证") || content.contains("自测"))
}

fn content_has_skill_contract(content: &str) -> bool {
    content.contains("---")
        && content.contains("name:")
        && content.contains("description:")
        && content.contains("项目资源")
        && content.contains("执行流程")
        && content.contains("完成 Gate")
        && content.contains("失败处理")
}

fn content_claims_to_be_generic(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    [
        "这是适用于所有项目",
        "本规则适用于所有项目",
        "本流程适用于所有项目",
        "this rule applies to every project",
        "this workflow applies to every project",
        "generic rule for all projects",
        "generic workflow for all projects",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

pub fn validate_staged_artifacts(
    workspace: &Path,
    inventory: &ProjectInventory,
    plan: &ArtifactPlan,
    kind: Option<ArtifactKind>,
) -> Vec<ValidationIssue> {
    let mut issues = Vec::new();
    let known_commands = inventory
        .commands
        .iter()
        .map(|command| command.command.trim())
        .collect::<BTreeSet<_>>();
    let planned_paths = plan
        .artifacts
        .iter()
        .map(|artifact| artifact.target_path.as_str())
        .collect::<BTreeSet<_>>();
    for artifact in &plan.artifacts {
        if kind.is_some() && kind != Some(artifact.kind) {
            continue;
        }
        let Ok(relative) = normalized_relative_path(&artifact.target_path) else {
            issues.push(issue(
                "artifact.path.invalid",
                "产物路径无效",
                Some(&artifact.target_path),
                "validate",
            ));
            continue;
        };
        let content = match fs::read_to_string(workspace.join(relative)) {
            Ok(content) => content,
            Err(error) => {
                issues.push(issue(
                    "artifact.file.missing",
                    format!("无法读取计划产物：{error}"),
                    Some(&artifact.target_path),
                    "validate",
                ));
                continue;
            }
        };
        let secret_detected = contains_secret_assignment(&content);
        if ["{{", "待填写", "TODO", "TBD", "以后补充"]
            .iter()
            .any(|token| content.contains(token))
        {
            issues.push(issue(
                "artifact.content.placeholder",
                "产物仍包含占位符或待办内容",
                Some(&artifact.target_path),
                "validate",
            ));
        }
        if chinese_count(&content) < 20
            || artifact.evidence.iter().any(|evidence| {
                evidence
                    .symbol
                    .as_deref()
                    .map(str::trim)
                    .filter(|symbol| !symbol.is_empty())
                    .is_none_or(|symbol| {
                        !evidence_is_cited_together(&content, &evidence.path, symbol)
                    })
            })
        {
            issues.push(issue(
                "artifact.content.not-project-specific",
                "产物缺少中文项目化说明或没有引用计划证据",
                Some(&artifact.target_path),
                "validate",
            ));
        }
        for section in &artifact.required_sections {
            if !content.contains(section) {
                issues.push(issue(
                    "artifact.section.missing",
                    format!("产物缺少必需章节：{section}"),
                    Some(&artifact.target_path),
                    "validate",
                ));
            }
        }
        for link in markdown_links(&content) {
            if local_markdown_link(link).is_some()
                && !link_exists(workspace, &artifact.target_path, link, &planned_paths)
            {
                issues.push(issue(
                    "artifact.link.dangling",
                    "Markdown 链接目标不存在或越出工作区",
                    Some(&artifact.target_path),
                    "validate",
                ));
            }
        }
        if secret_detected {
            issues.push(issue(
                "artifact.secret.detected",
                "产物疑似包含敏感配置值，已拒绝安装",
                Some(&artifact.target_path),
                "validate",
            ));
        }
        for command in command_candidates(&content) {
            if !known_commands.contains(command) {
                issues.push(issue(
                    "artifact.command.unknown",
                    "文档包含无法从项目清单确认的命令",
                    Some(&artifact.target_path),
                    "validate",
                ));
            }
        }
        if matches!(artifact.kind, ArtifactKind::Rule | ArtifactKind::Skill)
            && content_claims_to_be_generic(&content)
        {
            issues.push(issue(
                "artifact.content.generic",
                "项目 rule 或 skill 包含明确的跨项目通用模板内容",
                Some(&artifact.target_path),
                "validate",
            ));
        }
        if artifact.kind == ArtifactKind::Rule && !content_has_rule_contract(&content) {
            issues.push(issue(
                "artifact.rule.contract-missing",
                "项目规则缺少触发、复用、禁区、影响或验证约束",
                Some(&artifact.target_path),
                "validate",
            ));
        }
        if artifact.kind == ArtifactKind::Skill && !content_has_skill_contract(&content) {
            issues.push(issue(
                "artifact.skill.contract-missing",
                "项目 skill 缺少资源、流程、Gate 或失败处理",
                Some(&artifact.target_path),
                "validate",
            ));
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_factory::docs::ProjectLayers;
    use crate::project_factory::types::{
        CoverageExclusion, EvidenceReference, InventoryFile, ProjectInventory, ProjectModule,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "vibe-artifact-plan-{}-{sequence}",
                std::process::id()
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("fixture root");
            Self(root)
        }

        fn path(&self) -> &Path {
            &self.0
        }

        fn write(&self, relative: &str, content: &str) {
            let path = self.0.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("fixture parent");
            }
            fs::write(path, content).expect("fixture file");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn inventory(frontend: bool, backend: bool) -> ProjectInventory {
        ProjectInventory {
            schema_version: 1,
            project_name: "iam".into(),
            layers: ProjectLayers { frontend, backend },
            modules: vec![ProjectModule {
                name: "iam-service".into(),
                path: "iam-service".into(),
                kind: "maven".into(),
                manifests: vec!["iam-service/pom.xml".into()],
                source_roots: vec!["iam-service/src/main/java".into()],
            }],
            source_roots: vec!["iam-service/src/main/java".into()],
            files: vec![
                InventoryFile {
                    path: "iam-service/pom.xml".into(),
                    kind: "manifest".into(),
                    size: 8,
                    sha256: "hash".into(),
                    module: Some("iam-service".into()),
                },
                InventoryFile {
                    path: "iam-service/src/main/java/AuthService.java".into(),
                    kind: "source".into(),
                    size: 8,
                    sha256: "hash".into(),
                    module: Some("iam-service".into()),
                },
            ],
            commands: vec![],
            risk_keys: vec![],
        }
    }

    fn item(id: &str, kind: ArtifactKind, path: &str, topic: &str) -> ArtifactPlanItem {
        ArtifactPlanItem {
            id: id.into(),
            kind,
            layer: "backend".into(),
            topic: topic.into(),
            target_path: path.into(),
            rationale: "项目真实边界需要长期记录".into(),
            evidence: vec![EvidenceReference {
                path: "iam-service/src/main/java/AuthService.java".into(),
                symbol: Some("AuthService".into()),
            }],
            covers: vec!["iam-service".into(), "iam-service/src/main/java".into()],
            required_sections: vec!["真实证据".into(), "验证方式".into()],
        }
    }

    fn valid_plan() -> ArtifactPlan {
        ArtifactPlan {
            schema_version: 1,
            project_name: "iam".into(),
            artifacts: vec![
                item(
                    "project-map",
                    ArtifactKind::Document,
                    "docs/ai/project-map.md",
                    "project-map",
                ),
                item(
                    "architecture-boundaries",
                    ArtifactKind::Document,
                    "docs/ai/architecture-boundaries.md",
                    "architecture",
                ),
                item(
                    "reusable-assets",
                    ArtifactKind::Document,
                    "docs/ai/reusable-assets.md",
                    "reuse",
                ),
                item(
                    "verification-playbook",
                    ArtifactKind::Document,
                    "docs/ai/verification-playbook.md",
                    "verification",
                ),
                item(
                    "known-risks",
                    ArtifactKind::Document,
                    "docs/ai/known-risks-and-document-drift.md",
                    "known-risks",
                ),
                item(
                    "rule-router",
                    ArtifactKind::Rule,
                    ".claude/rules/project/README.md",
                    "rule-router",
                ),
                item(
                    "auth-lifecycle",
                    ArtifactKind::Rule,
                    ".claude/rules/project/backend/auth-lifecycle.md",
                    "authentication-lifecycle",
                ),
                item(
                    "auth-change-review",
                    ArtifactKind::Skill,
                    ".claude/skills/iam-auth-change-review/SKILL.md",
                    "authentication-change-review",
                ),
            ],
            exclusions: vec![],
        }
    }

    fn codes(issues: &[ValidationIssue]) -> Vec<&str> {
        issues.iter().map(|issue| issue.code.as_str()).collect()
    }

    #[test]
    fn rejects_unsafe_non_english_duplicate_and_outside_paths() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let mut plan = valid_plan();
        plan.artifacts.extend([
            item("中文", ArtifactKind::Document, "docs/ai/接口文档.md", "api"),
            item(
                "upper",
                ArtifactKind::Document,
                "docs/ai/API-Catalog.md",
                "api",
            ),
            item(
                "escape",
                ArtifactKind::Document,
                "docs/ai/../escape.md",
                "escape",
            ),
            item(
                "outside",
                ArtifactKind::Rule,
                ".claude/rules/global.md",
                "global",
            ),
            item(
                "project-map",
                ArtifactKind::Document,
                "docs/ai/duplicate.md",
                "duplicate",
            ),
            item(
                "same-path",
                ArtifactKind::Document,
                "docs/ai/project-map.md",
                "duplicate",
            ),
        ]);

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);
        let codes = codes(&issues);
        assert!(codes.contains(&"plan.path.not-kebab-case"));
        assert!(codes.contains(&"plan.path.traversal"));
        assert!(codes.contains(&"plan.path.outside-allowlist"));
        assert!(codes.contains(&"plan.id.duplicate"));
        assert!(codes.contains(&"plan.path.duplicate"));
    }

    #[test]
    fn aggregates_missing_evidence_coverage_router_common_docs_and_layer_errors() {
        let fixture = Fixture::new();
        let mut plan = valid_plan();
        plan.artifacts
            .retain(|artifact| !matches!(artifact.id.as_str(), "rule-router" | "reusable-assets"));
        plan.artifacts[0].evidence[0].path = "missing.java".into();
        for artifact in &mut plan.artifacts {
            artifact.covers.clear();
        }
        let mut frontend_rule = ArtifactPlanItem {
            layer: "frontend".into(),
            ..item(
                "vue-components",
                ArtifactKind::Rule,
                ".claude/rules/project/frontend/vue-components.md",
                "vue-components",
            )
        };
        frontend_rule.covers.clear();
        plan.artifacts.push(frontend_rule);

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);
        let codes = codes(&issues);
        assert!(codes.contains(&"plan.evidence.missing"));
        assert!(codes.contains(&"plan.module.uncovered"));
        assert!(codes.contains(&"plan.rule-router.missing"));
        assert!(codes.contains(&"plan.common-document.missing"));
        assert!(codes.contains(&"plan.layer.mismatch"));
    }

    #[test]
    fn rejects_generic_rules_and_generic_project_skills() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let mut plan = valid_plan();
        plan.artifacts.push(item(
            "backend-engineering",
            ArtifactKind::Rule,
            ".claude/rules/project/backend/backend-engineering.md",
            "backend-engineering",
        ));
        plan.artifacts.push(item(
            "developer",
            ArtifactKind::Skill,
            ".claude/skills/developer/SKILL.md",
            "developer",
        ));

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);
        let codes = codes(&issues);
        assert!(codes.contains(&"plan.rule.generic"));
        assert!(codes.contains(&"plan.skill.generic"));
    }

    #[test]
    fn rejects_artifacts_without_required_sections() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let mut plan = valid_plan();
        plan.artifacts[0].required_sections.clear();

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

        assert!(codes(&issues).contains(&"plan.sections.empty"));
    }

    #[test]
    fn accepts_backend_specific_plan_without_inventing_frontend_framework() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let plan = valid_plan();

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

        assert!(issues.is_empty(), "issues: {issues:#?}");
        assert_eq!(
            artifact_totals(&plan),
            ArtifactTotals {
                documents: 5,
                rules: 2,
                skills: 1,
                total: 8
            }
        );
    }

    #[test]
    fn reads_plan_and_aggregates_json_shape_errors() {
        let fixture = Fixture::new();
        fixture.write(".vibe-coding-platform/artifact-plan.json", "{not-json");
        let errors = read_artifact_plan(fixture.path()).expect_err("invalid plan");
        assert_eq!(errors[0].code, "plan.json.invalid");
    }

    #[test]
    fn staged_validation_rejects_placeholders_missing_sections_dangling_links_secrets_and_false_commands(
    ) {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let plan = valid_plan();
        for artifact in &plan.artifacts {
            fixture.write(
                &artifact.target_path,
                "# 示例\n\n待填写 TODO\n\n[缺失](docs/ai/no-such.md)\n\n`npm run imaginary`\npassword=secret-value\n",
            );
        }

        let issues =
            validate_staged_artifacts(fixture.path(), &inventory(false, true), &plan, None);
        let codes = codes(&issues);
        assert!(codes.contains(&"artifact.content.placeholder"));
        assert!(codes.contains(&"artifact.content.not-project-specific"));
        assert!(codes.contains(&"artifact.section.missing"));
        assert!(codes.contains(&"artifact.link.dangling"));
        assert!(codes.contains(&"artifact.secret.detected"));
        assert!(codes.contains(&"artifact.command.unknown"));
    }

    #[test]
    fn staged_rules_and_skills_require_executable_project_contracts() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let plan = valid_plan();
        for artifact in &plan.artifacts {
            let content = match artifact.kind {
                ArtifactKind::Document => format!(
                    "# 项目事实\n\n## 真实证据\n\n`iam-service/src/main/java/AuthService.java` 中的 `AuthService`。\n\n## 验证方式\n\n读取源码后验证。\n\n{}",
                    "项目边界与复用能力必须依据真实实现。".repeat(10)
                ),
                ArtifactKind::Rule => "# 认证规则\n\n真实项目规则但缺少完整执行字段。".repeat(20),
                ArtifactKind::Skill => "---\nname: iam-auth-change-review\ndescription: 项目认证变更。\n---\n\n# Skill\n\n只有概述。".repeat(10),
            };
            fixture.write(&artifact.target_path, &content);
        }

        let issues =
            validate_staged_artifacts(fixture.path(), &inventory(false, true), &plan, None);
        let codes = codes(&issues);
        assert!(codes.contains(&"artifact.rule.contract-missing"));
        assert!(codes.contains(&"artifact.skill.contract-missing"));
    }

    #[test]
    fn staged_content_must_cite_planned_symbols_not_only_paths() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &format!(
                "# 项目地图\n\n## 真实证据\n\n路径 `{}` 已读取，但这里故意没有写类名。\n\n## 验证方式\n\n按真实源码完成验证。\n\n{}",
                artifact.evidence[0].path,
                "当前项目结构、边界、复用入口和风险都必须保留真实证据。".repeat(8)
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert!(codes(&issues).contains(&"artifact.content.not-project-specific"));
    }

    #[test]
    fn coverage_requires_evidence_from_each_claimed_module_and_source_root() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        fixture.write(
            "billing-service/src/main/java/BillingService.java",
            "class BillingService {}",
        );
        let mut inventory = inventory(false, true);
        inventory.modules.push(ProjectModule {
            name: "billing-service".into(),
            path: "billing-service".into(),
            kind: "backend".into(),
            manifests: vec!["billing-service/pom.xml".into()],
            source_roots: vec!["billing-service/src/main/java".into()],
        });
        inventory
            .source_roots
            .push("billing-service/src/main/java".into());
        inventory.files.push(InventoryFile {
            path: "billing-service/src/main/java/BillingService.java".into(),
            kind: "source".into(),
            size: 8,
            sha256: "hash".into(),
            module: Some("billing-service".into()),
        });
        let mut plan = valid_plan();
        plan.artifacts[0].covers.extend([
            "billing-service".into(),
            "billing-service/src/main/java".into(),
        ]);

        let issues = validate_artifact_plan(fixture.path(), &inventory, &plan);
        let codes = codes(&issues);

        assert!(codes.contains(&"plan.coverage.evidence-unrelated"));
        assert!(codes.contains(&"plan.module.uncovered"));
        assert!(codes.contains(&"plan.source-root.uncovered"));
    }

    #[test]
    fn exclusions_require_valid_related_path_and_symbol_evidence() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let mut plan = valid_plan();
        for artifact in &mut plan.artifacts {
            artifact.covers.clear();
        }
        plan.exclusions.push(CoverageExclusion {
            target: "iam-service".into(),
            reason: "该模块由外部流程维护并单独验证".into(),
            evidence: vec![EvidenceReference {
                path: "missing/exclusion-proof.md".into(),
                symbol: Some("ExternalOwner".into()),
            }],
        });

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);
        let codes = codes(&issues);

        assert!(codes.contains(&"plan.exclusion.evidence.missing"));
        assert!(codes.contains(&"plan.module.uncovered"));
    }

    #[test]
    fn every_plan_evidence_requires_a_nonempty_symbol() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let mut plan = valid_plan();
        plan.artifacts[0].evidence[0].symbol = None;
        plan.artifacts[1].evidence[0].symbol = Some("   ".into());

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "plan.evidence.symbol-empty")
                .count(),
            2
        );
    }

    #[test]
    fn common_or_contract_labels_cannot_hide_opposite_layer_topics() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let mut plan = valid_plan();
        plan.artifacts.push(ArtifactPlanItem {
            layer: "contract".into(),
            ..item(
                "vue-components",
                ArtifactKind::Rule,
                ".claude/rules/project/frontend/vue-components.md",
                "vue-components",
            )
        });
        plan.artifacts.push(ArtifactPlanItem {
            layer: "common".into(),
            ..item(
                "design-system",
                ArtifactKind::Document,
                "docs/ai/design-system.md",
                "design-system",
            )
        });

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "plan.layer.mismatch")
                .count(),
            2
        );
    }

    #[test]
    fn rejects_prefixed_or_reworded_generic_rules_and_skills() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let mut plan = valid_plan();
        plan.artifacts.push(item(
            "backend-development",
            ArtifactKind::Rule,
            ".claude/rules/project/backend/backend-development.md",
            "coding-guidelines",
        ));
        plan.artifacts.push(item(
            "iam-developer",
            ArtifactKind::Skill,
            ".claude/skills/iam-developer/SKILL.md",
            "developer-workflow",
        ));
        plan.artifacts.push(item(
            "iam-engineering-standards",
            ArtifactKind::Rule,
            ".claude/rules/project/iam-engineering-standards.md",
            "general-best-practices",
        ));
        plan.artifacts.push(item(
            "general-debugging",
            ArtifactKind::Skill,
            ".claude/skills/general-debugging/SKILL.md",
            "problem-troubleshooting",
        ));

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);
        let codes = codes(&issues);

        assert_eq!(
            codes
                .iter()
                .filter(|code| **code == "plan.rule.generic")
                .count(),
            2
        );
        assert_eq!(
            codes
                .iter()
                .filter(|code| **code == "plan.skill.generic")
                .count(),
            2
        );
    }

    #[test]
    fn staged_evidence_path_and_symbol_must_be_cited_together() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &format!(
                "# 项目地图\n\n## 真实证据\n\n路径 `{}` 已核验。\n\n另一个无关段落提到 `AuthService`。\n\n## 验证方式\n\n{}",
                artifact.evidence[0].path,
                "项目边界、复用入口、风险与验证都必须依据真实实现。".repeat(8)
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert!(codes(&issues).contains(&"artifact.content.not-project-specific"));
    }

    #[test]
    fn secret_values_are_never_echoed_by_other_diagnostics() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        let secret = "do-not-echo-this-token";
        fixture.write(
            &artifact.target_path,
            &format!(
                "# 项目地图\n\n## 真实证据\n\n`{}` 与 `AuthService` 共同证明认证入口。\n\n## 验证方式\n\n{}\n\n`npm config set token={secret}`",
                artifact.evidence[0].path,
                "认证边界和验证步骤均来自当前项目源码。".repeat(8)
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert!(codes(&issues).contains(&"artifact.secret.detected"));
        assert!(issues.iter().all(|issue| !issue.detail.contains(secret)));
    }

    #[test]
    fn fenced_shell_commands_are_checked_against_inventory_commands() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &format!(
                "# 项目地图\n\n## 真实证据\n\n`{}` 与 `AuthService` 共同证明认证入口。\n\n## 验证方式\n\n{}\n\n```bash\nnpm run imaginary\n```",
                artifact.evidence[0].path,
                "认证边界和验证步骤均来自当前项目源码。".repeat(8)
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert!(codes(&issues).contains(&"artifact.command.unknown"));
    }

    #[test]
    fn markdown_links_resolve_only_from_the_containing_artifact() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        fixture.write("README.md", "# Root");
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &format!(
                "# 项目地图\n\n## 真实证据\n\n`{}` 与 `AuthService` 共同证明认证入口。\n\n## 验证方式\n\n{}\n\n[错误的同目录链接](README.md)",
                artifact.evidence[0].path,
                "认证边界和验证步骤均来自当前项目源码。".repeat(8)
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert!(codes(&issues).contains(&"artifact.link.dangling"));
    }

    #[test]
    fn markdown_links_may_walk_to_an_existing_file_inside_the_workspace() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        fixture.write("README.md", "# Root");
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &format!(
                "# 项目地图\n\n## 真实证据\n\n`{}` 与 `AuthService` 共同证明认证入口。\n\n## 验证方式\n\n{}\n\n[仓库说明](../../README.md)",
                artifact.evidence[0].path,
                "认证边界和验证步骤均来自当前项目源码。".repeat(8)
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert!(!codes(&issues).contains(&"artifact.link.dangling"));
    }

    #[test]
    fn staged_rules_and_skills_reject_explicitly_generic_template_content() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService {}",
        );
        let mut plan = valid_plan();
        plan.artifacts.retain(|artifact| {
            matches!(
                artifact.id.as_str(),
                "auth-lifecycle" | "auth-change-review"
            )
        });
        for artifact in &plan.artifacts {
            let evidence = &artifact.evidence[0];
            let content = match artifact.kind {
                ArtifactKind::Rule => format!(
                    "# 认证规则\n\n## 真实证据\n\n`{}` 与 `AuthService`。\n\n## 验证方式\n\npaths: iam-service/**\n\n触发：修改代码。复用：先搜索资产。禁止：不得重复实现。影响：检查调用方。验证：运行测试。\n\n{}",
                    evidence.path,
                    "这是适用于所有项目的通用开发规则。".repeat(8)
                ),
                ArtifactKind::Skill => format!(
                    "---\nname: iam-auth-change-review\ndescription: 认证变更检查。\n---\n\n# 认证变更\n\n## 真实证据\n\n`{}` 与 `AuthService`。\n\n## 验证方式\n\n## 项目资源\n读取源码。\n## 执行流程\n执行检查。\n## 完成 Gate\n运行验证。\n## 失败处理\n停止安装。\n\n{}",
                    evidence.path,
                    "这是适用于所有项目的通用调试流程。".repeat(8)
                ),
                ArtifactKind::Document => unreachable!("filtered to rule and skill"),
            };
            fixture.write(&artifact.target_path, &content);
        }

        let issues =
            validate_staged_artifacts(fixture.path(), &inventory(false, true), &plan, None);

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "artifact.content.generic")
                .count(),
            2
        );
    }
}
