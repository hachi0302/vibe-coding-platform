use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path};

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
const GENERIC_SKILLS: &[&str] = &[
    "developer",
    "problem-diagnose",
    "code-review",
    "review-feedback-handler",
    "worktree",
    "skill-designer",
];
const GENERIC_RULE_TOPICS: &[&str] = &[
    "backend-engineering",
    "frontend-engineering",
    "development-baseline",
    "coding-rules",
    "general-rules",
];

fn issue(
    code: &str,
    detail: impl Into<String>,
    path: Option<&str>,
    stage: &str,
) -> ValidationIssue {
    ValidationIssue {
        code: code.to_string(),
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
        for evidence in &item.evidence {
            if !evidence_path_exists(workspace, inventory, &evidence.path) {
                issues.push(issue(
                    "plan.evidence.missing",
                    format!("证据路径不存在：{}", evidence.path),
                    Some(&item.target_path),
                    "plan",
                ));
            } else if let Some(symbol) = evidence.symbol.as_deref() {
                if !evidence_symbol_exists(workspace, &evidence.path, symbol) {
                    issues.push(issue(
                        "plan.evidence.symbol-missing",
                        format!("证据符号不存在：{}#{}", evidence.path, symbol),
                        Some(&item.target_path),
                        "plan",
                    ));
                }
            }
        }
        covered.extend(item.covers.iter().cloned());

        let layer_valid = match item.layer.as_str() {
            "common" | "contract" => true,
            "frontend" => inventory.layers.frontend,
            "backend" | "database" | "integration" => inventory.layers.backend,
            _ => false,
        };
        if !layer_valid {
            issues.push(issue(
                "plan.layer.mismatch",
                format!("产物层级与项目不匹配：{}", item.layer),
                Some(&item.target_path),
                "plan",
            ));
        }
        if item.kind == ArtifactKind::Rule
            && GENERIC_RULE_TOPICS
                .iter()
                .any(|generic| item.topic == *generic || item.id == *generic)
        {
            issues.push(issue(
                "plan.rule.generic",
                "规则主题过于泛化，必须绑定真实项目责任或工作流",
                Some(&item.target_path),
                "plan",
            ));
        }
        if item.kind == ArtifactKind::Skill {
            let skill_name = item
                .target_path
                .strip_prefix(".claude/skills/")
                .and_then(|rest| rest.split('/').next())
                .unwrap_or_default();
            if GENERIC_SKILLS.iter().any(|generic| {
                skill_name == *generic || item.id == *generic || item.topic == *generic
            }) {
                issues.push(issue(
                    "plan.skill.generic",
                    "通用平台能力不得复制为项目 skill",
                    Some(&item.target_path),
                    "plan",
                ));
            }
        }
    }

    let exclusions = plan
        .exclusions
        .iter()
        .filter(|exclusion| exclusion.reason.trim().chars().count() >= 8)
        .map(|exclusion| exclusion.target.as_str())
        .collect::<BTreeSet<_>>();
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

fn link_exists(workspace: &Path, artifact_path: &str, link: &str) -> bool {
    let link = link.split('#').next().unwrap_or_default();
    if link.is_empty()
        || link.starts_with('#')
        || link.starts_with("http://")
        || link.starts_with("https://")
        || link.starts_with("mailto:")
    {
        return true;
    }
    let Ok(relative) = normalized_relative_path(link) else {
        return false;
    };
    if workspace.join(relative).is_file() {
        return true;
    }
    Path::new(artifact_path)
        .parent()
        .map(|parent| workspace.join(parent).join(relative).is_file())
        .unwrap_or(false)
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

fn command_candidates(content: &str) -> Vec<&str> {
    let mut commands = Vec::new();
    let mut remaining = content;
    while let Some(start) = remaining.find('`') {
        remaining = &remaining[start + 1..];
        let Some(end) = remaining.find('`') else {
            break;
        };
        let candidate = remaining[..end].trim();
        if [
            "npm ", "pnpm ", "yarn ", "cargo ", "mvn ", "gradle ", "go ", "pytest", "python ",
        ]
        .iter()
        .any(|prefix| candidate.starts_with(prefix))
        {
            commands.push(candidate);
        }
        remaining = &remaining[end + 1..];
    }
    commands
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
                !content.contains(&evidence.path)
                    || evidence
                        .symbol
                        .as_deref()
                        .is_some_and(|symbol| !content.replace(&evidence.path, "").contains(symbol))
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
            if link.ends_with(".md")
                && !link_exists(workspace, &artifact.target_path, link)
                && !planned_paths.contains(link)
            {
                issues.push(issue(
                    "artifact.link.dangling",
                    format!("Markdown 链接目标不存在：{link}"),
                    Some(&artifact.target_path),
                    "validate",
                ));
            }
        }
        if contains_secret_assignment(&content) {
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
                    format!("文档命令无法从项目脚本中确认：{command}"),
                    Some(&artifact.target_path),
                    "validate",
                ));
            }
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
        EvidenceReference, InventoryFile, ProjectInventory, ProjectModule,
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
}
