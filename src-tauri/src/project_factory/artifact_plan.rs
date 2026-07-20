use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::inventory::{content_sha256, read_project_bytes_handle_safe};
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
    "clean-code",
    "bug-fix",
    "refactor",
    "refactor-workflow",
    "debugging",
    "debug-workflow",
    "coding-assistant",
    "feature-implementation",
    "maintenance",
    "coding-style",
    "style-guide",
];
const GENERIC_SKILL_WORDS: &[&str] = &[
    "code",
    "coding",
    "clean",
    "bug",
    "fix",
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
    "refactor",
    "refactoring",
    "review",
    "skill",
    "troubleshooting",
    "workflow",
    "assistant",
    "feature",
    "implementation",
    "implementing",
    "maintenance",
    "maintain",
    "style",
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
    "clean-code",
    "bug-fix",
    "refactor",
    "feature-implementation",
    "maintenance",
    "coding-style",
    "style-guide",
];
const GENERIC_RULE_WORDS: &[&str] = &[
    "backend",
    "baseline",
    "best",
    "code",
    "coding",
    "clean",
    "bug",
    "fix",
    "development",
    "engineering",
    "frontend",
    "general",
    "guideline",
    "guidelines",
    "practice",
    "practices",
    "refactor",
    "refactoring",
    "rule",
    "rules",
    "standard",
    "standards",
    "feature",
    "implementation",
    "maintenance",
    "style",
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
            let components = path.split('/').collect::<Vec<_>>();
            components.len() == 4
                && components[0] == ".claude"
                && components[1] == "skills"
                && !components[2].is_empty()
                && components[3] == "SKILL.md"
        }
    }
}

#[derive(Debug)]
enum SnapshotError {
    Missing,
    Unsafe,
    Mismatch,
}

fn read_without_symlinks(workspace: &Path, relative: &Path) -> Result<Vec<u8>, SnapshotError> {
    match read_project_bytes_handle_safe(workspace, relative) {
        Ok(Some(bytes)) => Ok(bytes),
        Ok(None) => Err(SnapshotError::Unsafe),
        Err(_) if !workspace.join(relative).try_exists().unwrap_or(false) => {
            Err(SnapshotError::Missing)
        }
        Err(_) => Err(SnapshotError::Unsafe),
    }
}

fn inventory_snapshot<'a>(
    workspace: &Path,
    inventory: &'a ProjectInventory,
    path: &str,
) -> Result<(&'a super::types::InventoryFile, Vec<u8>), SnapshotError> {
    let relative = normalized_relative_path(path).map_err(|_| SnapshotError::Unsafe)?;
    let file = inventory
        .files
        .iter()
        .find(|file| file.path == path)
        .ok_or(SnapshotError::Missing)?;
    let bytes = read_without_symlinks(workspace, relative)?;
    if bytes.len() as u64 != file.size || content_sha256(&bytes) != file.sha256 {
        return Err(SnapshotError::Mismatch);
    }
    Ok((file, bytes))
}

fn symbol_looks_real(symbol: &str) -> bool {
    const RESERVED: &[&str] = &[
        "api",
        "app",
        "application",
        "async",
        "auth",
        "await",
        "class",
        "client",
        "component",
        "config",
        "configuration",
        "const",
        "controller",
        "def",
        "enum",
        "export",
        "false",
        "fn",
        "function",
        "handler",
        "helper",
        "impl",
        "import",
        "index",
        "interface",
        "let",
        "main",
        "manager",
        "model",
        "module",
        "namespace",
        "new",
        "none",
        "null",
        "package",
        "private",
        "protected",
        "public",
        "repository",
        "return",
        "security",
        "self",
        "server",
        "service",
        "static",
        "struct",
        "this",
        "trait",
        "true",
        "type",
        "undefined",
        "util",
        "utils",
        "var",
    ];
    let symbol = symbol.trim();
    !symbol.is_empty()
        && symbol.len() <= 200
        && symbol
            .chars()
            .next()
            .is_some_and(|character| character.is_alphanumeric() || character == '_')
        && symbol
            .chars()
            .last()
            .is_some_and(|character| character.is_alphanumeric() || character == '_')
        && symbol.chars().any(|character| character.is_alphabetic())
        && symbol.chars().all(|character| {
            character.is_alphanumeric()
                || matches!(character, '_' | '-' | '.' | ':' | '#' | '$' | '[' | ']')
        })
        && !symbol.contains("..")
        && !symbol.contains(":::")
        && !RESERVED.contains(&symbol.to_ascii_lowercase().as_str())
}

fn identifier_character(character: char) -> bool {
    character.is_alphanumeric() || matches!(character, '_' | '$')
}

fn content_has_symbol(content: &str, symbol: &str) -> bool {
    content.match_indices(symbol).any(|(start, matched)| {
        let before = content[..start].chars().next_back();
        let after = content[start + matched.len()..].chars().next();
        before.is_none_or(|character| !identifier_character(character))
            && after.is_none_or(|character| !identifier_character(character))
    })
}

#[derive(Debug, PartialEq, Eq)]
enum DeclarationToken {
    Identifier(String),
    Punctuation(char),
}

fn declaration_code(line: &str) -> String {
    let mut visible = String::with_capacity(line.len());
    let mut characters = line.chars().peekable();
    let mut quote = None;
    let mut escaped = false;
    while let Some(character) = characters.next() {
        if let Some(marker) = quote {
            visible.push(character);
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == marker {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'' | '`') {
            quote = Some(character);
            visible.push(character);
        } else if character == '/' && characters.peek() == Some(&'/') {
            break;
        } else if character == '/' && characters.peek() == Some(&'*') {
            characters.next();
            let mut previous = '\0';
            for comment_character in characters.by_ref() {
                if previous == '*' && comment_character == '/' {
                    break;
                }
                previous = comment_character;
            }
            visible.push(' ');
        } else {
            visible.push(character);
        }
    }
    visible
}

fn declaration_tokens(line: &str) -> Vec<DeclarationToken> {
    let mut tokens = Vec::new();
    let mut identifier = String::new();
    let mut quote = None;
    let mut escaped = false;
    for character in line.chars() {
        if let Some(marker) = quote {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == marker {
                quote = None;
            }
            continue;
        }
        if matches!(character, '"' | '\'' | '`') {
            if !identifier.is_empty() {
                tokens.push(DeclarationToken::Identifier(std::mem::take(
                    &mut identifier,
                )));
            }
            quote = Some(character);
        } else if identifier_character(character) {
            identifier.push(character);
        } else {
            if !identifier.is_empty() {
                tokens.push(DeclarationToken::Identifier(std::mem::take(
                    &mut identifier,
                )));
            }
            if !character.is_whitespace() {
                tokens.push(DeclarationToken::Punctuation(character));
            }
        }
    }
    if !identifier.is_empty() {
        tokens.push(DeclarationToken::Identifier(identifier));
    }
    tokens
}

fn tokens_declare_symbol(tokens: &[DeclarationToken], symbol: &str) -> bool {
    const DECLARATION_KEYWORDS: &[&str] = &[
        "class",
        "interface",
        "enum",
        "struct",
        "record",
        "trait",
        "type",
        "fn",
        "function",
        "def",
        "const",
        "let",
        "var",
        "module",
        "namespace",
        "table",
        "view",
        "procedure",
    ];
    const NON_DECLARATION_PREFIXES: &[&str] = &[
        "return", "new", "throw", "await", "yield", "if", "while", "for", "switch",
    ];
    tokens.iter().enumerate().any(|(index, token)| {
        let DeclarationToken::Identifier(identifier) = token else {
            return false;
        };
        if identifier != symbol {
            return false;
        }
        if index > 0
            && matches!(
                &tokens[index - 1],
                DeclarationToken::Identifier(previous)
                    if DECLARATION_KEYWORDS.contains(&previous.to_ascii_lowercase().as_str())
            )
        {
            return true;
        }
        if !matches!(
            tokens.get(index + 1),
            Some(DeclarationToken::Punctuation('('))
        ) || matches!(
            tokens.get(index.wrapping_sub(1)),
            Some(DeclarationToken::Punctuation('.'))
        ) {
            return false;
        }
        let prefix = &tokens[..index];
        if prefix.iter().any(|token| {
            matches!(token, DeclarationToken::Punctuation('='))
                || matches!(
                    token,
                    DeclarationToken::Identifier(identifier)
                        if NON_DECLARATION_PREFIXES
                            .contains(&identifier.to_ascii_lowercase().as_str())
                )
        }) {
            return false;
        }
        let mut depth = 0usize;
        let mut closing = None;
        for (offset, token) in tokens[index + 1..].iter().enumerate() {
            match token {
                DeclarationToken::Punctuation('(') => depth += 1,
                DeclarationToken::Punctuation(')') => {
                    depth = depth.saturating_sub(1);
                    if depth == 0 {
                        closing = Some(index + 1 + offset);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(closing) = closing else {
            return false;
        };
        let declaration_tail = tokens[closing + 1..].iter().find_map(|token| match token {
            DeclarationToken::Punctuation(character @ ('{' | ';')) => Some(*character),
            _ => None,
        });
        let typed_prefix = prefix
            .iter()
            .any(|token| matches!(token, DeclarationToken::Identifier(_)));
        match declaration_tail {
            Some('{') => typed_prefix || index == 0,
            Some(';') => typed_prefix,
            _ => false,
        }
    })
}

fn line_declares_symbol(line: &str, symbol: &str) -> bool {
    let line = declaration_code(line);
    let line = line.as_str();
    let mut search_from = 0;
    while let Some(relative) = line[search_from..].find(symbol) {
        let start = search_from + relative;
        let end = start + symbol.len();
        let before = line[..start].chars().next_back();
        let after = line[end..].chars().next();
        if before.is_none_or(|character| !identifier_character(character))
            && after.is_none_or(|character| !identifier_character(character))
        {
            let prefix = &line[..start];
            let suffix = &line[end..];
            let suffix_after_quote = suffix.trim_start_matches(['"', '\'', '`']).trim_start();
            let prefix_trimmed = prefix.trim_end();
            let configuration_key = matches!(suffix_after_quote.chars().next(), Some(':' | '='))
                && (prefix_trimmed.is_empty()
                    || prefix_trimmed.ends_with(['"', '\'', '`'])
                    || prefix_trimmed == "-");
            let xml_key = prefix_trimmed.ends_with('<') && suffix_after_quote.starts_with('>');
            if configuration_key || xml_key {
                return true;
            }
        }
        search_from = end;
    }
    tokens_declare_symbol(&declaration_tokens(line), symbol)
}

#[derive(Clone, Copy)]
enum DeclarationMultilineLiteral {
    Template,
    TripleSingle,
    TripleDouble,
}

fn unescaped_delimiter_end(value: &str, delimiter: &str) -> Option<usize> {
    let mut search_from = 0;
    while let Some(relative) = value[search_from..].find(delimiter) {
        let start = search_from + relative;
        let backslashes = value[..start]
            .bytes()
            .rev()
            .take_while(|byte| *byte == b'\\')
            .count();
        if backslashes % 2 == 0 {
            return Some(start + delimiter.len());
        }
        search_from = start + delimiter.len();
    }
    None
}

fn declaration_visible_line(
    line: &str,
    in_block_comment: &mut bool,
    multiline_literal: &mut Option<DeclarationMultilineLiteral>,
) -> String {
    let mut visible = String::with_capacity(line.len());
    let mut index = 0;
    while index < line.len() {
        let remaining = &line[index..];
        if *in_block_comment {
            let Some(end) = remaining.find("*/") else {
                break;
            };
            index += end + 2;
            *in_block_comment = false;
            visible.push(' ');
            continue;
        }
        if let Some(literal) = *multiline_literal {
            let delimiter = match literal {
                DeclarationMultilineLiteral::Template => "`",
                DeclarationMultilineLiteral::TripleSingle => "'''",
                DeclarationMultilineLiteral::TripleDouble => "\"\"\"",
            };
            let Some(end) = unescaped_delimiter_end(remaining, delimiter) else {
                break;
            };
            index += end;
            *multiline_literal = None;
            visible.push(' ');
            continue;
        }
        if remaining.starts_with("//") {
            break;
        }
        if remaining.starts_with("/*") {
            index += 2;
            *in_block_comment = true;
            continue;
        }
        if remaining.starts_with("'''") {
            index += 3;
            *multiline_literal = Some(DeclarationMultilineLiteral::TripleSingle);
            visible.push(' ');
            continue;
        }
        if remaining.starts_with("\"\"\"") {
            index += 3;
            *multiline_literal = Some(DeclarationMultilineLiteral::TripleDouble);
            visible.push(' ');
            continue;
        }
        let character = remaining
            .chars()
            .next()
            .expect("index remains on a character boundary");
        let character_len = character.len_utf8();
        if character == '`' {
            index += character_len;
            *multiline_literal = Some(DeclarationMultilineLiteral::Template);
            visible.push(' ');
            continue;
        }
        if matches!(character, '\'' | '"') {
            visible.push(character);
            index += character_len;
            let remaining = &line[index..];
            let delimiter = if character == '\'' { "'" } else { "\"" };
            let Some(end) = unescaped_delimiter_end(remaining, delimiter) else {
                visible.push_str(remaining);
                break;
            };
            visible.push_str(&remaining[..end]);
            index += end;
            continue;
        }
        visible.push(character);
        index += character_len;
    }
    visible
}

fn content_declares_symbol(content: &str, symbol: &str) -> bool {
    let mut in_block_comment = false;
    let mut multiline_literal = None;
    content.lines().any(|line| {
        let visible = declaration_visible_line(line, &mut in_block_comment, &mut multiline_literal);
        let visible = visible.trim();
        !visible.starts_with('#')
            && !visible.starts_with('*')
            && line_declares_symbol(visible, symbol)
    })
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
    let mut snapshot = None;
    if evidence.path.trim().is_empty() {
        issues.push(issue(
            format!("{code_prefix}.path-empty"),
            "证据路径不能为空",
            target_path,
            "plan",
        ));
        valid = false;
    } else {
        match inventory_snapshot(workspace, inventory, &evidence.path) {
            Ok((_, bytes)) => snapshot = Some(bytes),
            Err(SnapshotError::Missing) => {
                issues.push(issue(
                    format!("{code_prefix}.missing"),
                    "证据路径不在可信项目快照中",
                    target_path,
                    "plan",
                ));
                valid = false;
            }
            Err(SnapshotError::Unsafe) => {
                issues.push(issue(
                    format!("{code_prefix}.unsafe"),
                    "证据路径包含链接、越界或非普通文件",
                    target_path,
                    "plan",
                ));
                valid = false;
            }
            Err(SnapshotError::Mismatch) => {
                issues.push(issue(
                    format!("{code_prefix}.snapshot-mismatch"),
                    "证据文件与项目清单的大小或内容哈希不一致",
                    target_path,
                    "plan",
                ));
                valid = false;
            }
        }
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
    } else if !symbol_looks_real(symbol) {
        issues.push(issue(
            format!("{code_prefix}.symbol-invalid"),
            "证据符号必须是标识符、限定名或配置键",
            target_path,
            "plan",
        ));
        valid = false;
    } else if snapshot
        .as_deref()
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .is_some_and(|content| !content_declares_symbol(content, symbol))
    {
        issues.push(issue(
            format!("{code_prefix}.symbol-missing"),
            "证据符号未在可信项目文件中找到",
            target_path,
            "plan",
        ));
        valid = false;
    }
    valid
}

fn workspace_integrity_issues(
    workspace: &Path,
    inventory: &ProjectInventory,
    stage: &str,
) -> Vec<ValidationIssue> {
    inventory
        .files
        .iter()
        .filter(|file| {
            matches!(
                file.kind.as_str(),
                "source" | "test" | "database" | "config"
            )
        })
        .filter_map(|file| {
            inventory_snapshot(workspace, inventory, &file.path)
                .err()
                .map(|_| {
                    issue(
                        "workspace.source.modified",
                        "项目源文件已缺失、被链接替换或偏离扫描快照",
                        Some(&file.path),
                        stage,
                    )
                })
        })
        .collect()
}

fn plan_strings(plan: &ArtifactPlan) -> Vec<&str> {
    let mut values = vec![plan.project_name.as_str()];
    for artifact in &plan.artifacts {
        values.extend([
            artifact.id.as_str(),
            artifact.layer.as_str(),
            artifact.topic.as_str(),
            artifact.target_path.as_str(),
            artifact.rationale.as_str(),
        ]);
        values.extend(artifact.covers.iter().map(String::as_str));
        values.extend(artifact.required_sections.iter().map(String::as_str));
        for evidence in &artifact.evidence {
            values.push(evidence.path.as_str());
            if let Some(symbol) = evidence.symbol.as_deref() {
                values.push(symbol);
            }
        }
    }
    for exclusion in &plan.exclusions {
        values.extend([exclusion.target.as_str(), exclusion.reason.as_str()]);
        for evidence in &exclusion.evidence {
            values.push(evidence.path.as_str());
            if let Some(symbol) = evidence.symbol.as_deref() {
                values.push(symbol);
            }
        }
    }
    values
}

fn plan_contains_secret(plan: &ArtifactPlan) -> bool {
    plan_strings(plan).into_iter().any(contains_secret_material)
}

fn redact_plan_diagnostics(issues: &mut [ValidationIssue]) {
    for issue in issues {
        issue.detail = "产物计划验证失败；潜在敏感值已隐藏".to_string();
        issue.path = None;
    }
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

#[derive(Default)]
struct EvidenceSignals {
    frontend: bool,
    backend: bool,
    database: bool,
    integration: bool,
    contract_boundary: bool,
}

fn has_any_word(value: &str, expected: &[&str]) -> bool {
    identifier_words(value)
        .iter()
        .any(|word| expected.contains(&word.as_str()))
}

fn evidence_signals(
    inventory: &ProjectInventory,
    evidence: &[&super::types::EvidenceReference],
) -> EvidenceSignals {
    let mut signals = EvidenceSignals::default();
    for evidence in evidence {
        let Some(file) = inventory
            .files
            .iter()
            .find(|file| file.path == evidence.path)
        else {
            continue;
        };
        let module = file.module.as_deref().and_then(|owner| {
            inventory
                .modules
                .iter()
                .find(|module| module.path == owner || module.name == owner)
        });
        let module_kind = module
            .map(|module| module.kind.to_ascii_lowercase())
            .unwrap_or_default();
        let extension = Path::new(&file.path)
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let frontend_file = module_kind == "frontend"
            || (inventory.layers.frontend
                && !inventory.layers.backend
                && matches!(file.kind.as_str(), "source" | "test"))
            || (inventory.layers.frontend
                && matches!(extension.as_str(), "vue" | "svelte" | "tsx" | "jsx"));
        let path_components = file
            .path
            .split('/')
            .map(|component| component.to_ascii_lowercase())
            .collect::<Vec<_>>();
        let node_api_boundary = path_components.windows(2).any(|components| {
            matches!(
                (components[0].as_str(), components[1].as_str()),
                ("pages", "api") | ("src", "api") | ("app", "api")
            )
        });
        let frontend_router = path_components
            .iter()
            .any(|component| matches!(component.as_str(), "router" | "routers"));
        let node_server_path = path_components
            .iter()
            .any(|component| matches!(component.as_str(), "server" | "backend"));
        let node_server_symbol = has_any_word(
            evidence.symbol.as_deref().unwrap_or_default(),
            &[
                "controller",
                "gateway",
                "filter",
                "middleware",
                "resolver",
                "handler",
            ],
        );
        let node_fullstack_backend = inventory.layers.frontend
            && inventory.layers.backend
            && module.is_some_and(|module| {
                module
                    .manifests
                    .iter()
                    .any(|manifest| manifest.ends_with("package.json"))
            })
            && matches!(
                extension.as_str(),
                "ts" | "js" | "mts" | "cts" | "mjs" | "cjs"
            )
            && !frontend_router
            && (node_api_boundary || node_server_path || node_server_symbol);
        let backend_file = module_kind == "backend"
            || (inventory.layers.backend
                && !inventory.layers.frontend
                && matches!(file.kind.as_str(), "source" | "test"))
            || (inventory.layers.backend
                && !frontend_file
                && matches!(
                    extension.as_str(),
                    "java" | "kt" | "scala" | "go" | "rs" | "py" | "cs" | "rb" | "php"
                ))
            || node_fullstack_backend;
        signals.frontend |= frontend_file;
        signals.backend |= backend_file;
        signals.database |= file.kind == "database"
            || has_any_word(
                &file.path,
                &[
                    "database",
                    "db",
                    "migration",
                    "migrations",
                    "flyway",
                    "liquibase",
                ],
            );
        signals.integration |= has_any_word(
            &format!(
                "{} {}",
                file.path,
                evidence.symbol.as_deref().unwrap_or_default()
            ),
            &[
                "api",
                "client",
                "sdk",
                "integration",
                "adapter",
                "gateway",
                "callback",
                "webhook",
                "remote",
            ],
        );
        signals.contract_boundary |= has_any_word(
            &format!(
                "{} {}",
                file.path,
                evidence.symbol.as_deref().unwrap_or_default()
            ),
            &[
                "api", "client", "sdk", "contract", "dto", "openapi", "proto",
            ],
        );
    }
    signals
}

fn artifact_layer_valid(
    inventory: &ProjectInventory,
    item: &ArtifactPlanItem,
    evidence: &[&super::types::EvidenceReference],
) -> bool {
    let signals = evidence_signals(inventory, evidence);
    match item.layer.as_str() {
        "common" => {
            item.kind == ArtifactKind::Document
                && (COMMON_DOCUMENT_IDS.contains(&item.id.as_str())
                    || (signals.frontend && signals.backend))
        }
        "contract" => signals.contract_boundary || (signals.frontend && signals.backend),
        "frontend" => inventory.layers.frontend && signals.frontend,
        "backend" => inventory.layers.backend && signals.backend,
        "database" => signals.database,
        "integration" => signals.integration,
        _ => false,
    }
}

fn normalized_theme_word(word: &str) -> String {
    match word {
        "authentication" | "authorization" | "authenticated" => "auth".to_string(),
        _ if word.len() > 4 && word.ends_with("ies") => {
            format!("{}y", &word[..word.len() - 3])
        }
        _ if word.len() > 4 && word.ends_with('s') => word[..word.len() - 1].to_string(),
        _ => word.to_string(),
    }
}

fn theme_tokens<'a>(
    values: impl IntoIterator<Item = &'a str>,
    ignored: &BTreeSet<String>,
) -> BTreeSet<String> {
    const STRUCTURAL: &[&str] = &[
        "src",
        "main",
        "test",
        "tests",
        "java",
        "kotlin",
        "rust",
        "docs",
        "claude",
        "rule",
        "rules",
        "skill",
        "skills",
        "project",
        "frontend",
        "backend",
        "common",
        "contract",
        "integration",
        "document",
        "change",
        "review",
        "workflow",
        "lifecycle",
    ];
    values
        .into_iter()
        .flat_map(identifier_words)
        .map(|word| normalized_theme_word(&word))
        .filter(|word| {
            word.len() >= 3 && !STRUCTURAL.contains(&word.as_str()) && !ignored.contains(word)
        })
        .collect()
}

fn artifact_topic_supported(
    inventory: &ProjectInventory,
    item: &ArtifactPlanItem,
    evidence: &[&super::types::EvidenceReference],
) -> bool {
    if (item.kind == ArtifactKind::Document && COMMON_DOCUMENT_IDS.contains(&item.id.as_str()))
        || item.id == "rule-router"
    {
        return true;
    }
    let ignored = theme_tokens(
        std::iter::once(inventory.project_name.as_str()),
        &BTreeSet::new(),
    );
    let artifact_tokens = theme_tokens(
        [
            item.id.as_str(),
            item.topic.as_str(),
            item.target_path.as_str(),
        ],
        &ignored,
    );
    let evidence_tokens = theme_tokens(
        evidence.iter().flat_map(|evidence| {
            [
                evidence.path.as_str(),
                evidence.symbol.as_deref().unwrap_or_default(),
            ]
        }),
        &ignored,
    );
    if !artifact_tokens.is_empty() && !artifact_tokens.is_disjoint(&evidence_tokens) {
        return true;
    }
    let rationale_tokens = identifier_words(&item.rationale)
        .into_iter()
        .map(|word| normalized_theme_word(&word))
        .collect::<BTreeSet<_>>();
    let rationale_names_a_concept = artifact_tokens
        .iter()
        .any(|token| rationale_tokens.contains(token));
    let rationale_names_verified_evidence = evidence.iter().any(|evidence| {
        evidence
            .symbol
            .as_deref()
            .is_some_and(|symbol| content_has_symbol(&item.rationale, symbol))
            || item.rationale.contains(&evidence.path)
    });
    rationale_names_a_concept && rationale_names_verified_evidence
}

fn generic_engineering_capability<'a>(values: impl IntoIterator<Item = &'a str>) -> bool {
    let words = values
        .into_iter()
        .flat_map(identifier_words)
        .collect::<BTreeSet<_>>();
    let broad_engineering_work = [
        "programming",
        "programmer",
        "development",
        "developer",
        "engineering",
        "engineer",
    ]
    .iter()
    .any(|category| words.contains(*category));
    let generic_packaging = [
        "helper",
        "guide",
        "assistant",
        "workflow",
        "playbook",
        "handbook",
    ]
    .iter()
    .any(|category| words.contains(*category));
    broad_engineering_work || (generic_packaging && words.len() <= 2)
}

fn generic_rule(item: &ArtifactPlanItem) -> bool {
    let values = [item.id.as_str(), item.topic.as_str()];
    generic_engineering_capability(values)
        || values.iter().any(|value| {
            GENERIC_RULE_PHRASES
                .iter()
                .any(|phrase| contains_identifier_phrase(value, phrase))
        })
        || {
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
    generic_engineering_capability(values)
        || values.into_iter().any(|value| {
            GENERIC_SKILL_PHRASES
                .iter()
                .any(|phrase| contains_identifier_phrase(value, phrase))
        })
        || {
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

fn skill_declares_inline_resources(item: &ArtifactPlanItem) -> bool {
    let rationale = item.rationale.to_ascii_lowercase();
    let explains_inline =
        (rationale.contains("内嵌") || rationale.contains("内联") || rationale.contains("inline"))
            && rationale.contains("skill.md");
    let requires_resource_section = item
        .required_sections
        .iter()
        .any(|section| heading_slug(section) == heading_slug("项目资源"));
    explains_inline && requires_resource_section
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
    let plan_secret = plan_contains_secret(plan);
    let mut issues = workspace_integrity_issues(workspace, inventory, "plan");
    if plan_secret {
        issues.push(issue(
            "plan.secret.detected",
            "产物计划包含疑似敏感配置值，已拒绝",
            None,
            "plan",
        ));
    }
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

        if !artifact_layer_valid(inventory, item, &valid_evidence) {
            issues.push(issue(
                "plan.layer.mismatch",
                "产物层级缺少匹配的 inventory 路径、模块或边界证据",
                Some(&item.target_path),
                "plan",
            ));
        }
        if !artifact_topic_supported(inventory, item, &valid_evidence) {
            issues.push(issue(
                "plan.topic.unsupported",
                "产物主题无法由计划中的项目路径与符号证据支持",
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
        if item.kind == ArtifactKind::Skill && !skill_declares_inline_resources(item) {
            issues.push(issue(
                "plan.skill.resources-inline-required",
                "项目 skill 必须声明项目资源章节，并说明资源全部内嵌在 SKILL.md 中",
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
    if plan_secret {
        redact_plan_diagnostics(&mut issues);
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

#[derive(Debug)]
struct MarkdownHeading {
    level: usize,
    title: String,
    slug: String,
    line: usize,
}

#[derive(Debug)]
struct MarkdownLink {
    destination: Option<String>,
}

fn markdown_destination(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(value) = value.strip_prefix('<') {
        return value.find('>').map(|end| value[..end].to_string());
    }
    let end = value
        .char_indices()
        .find_map(|(index, character)| character.is_whitespace().then_some(index))
        .unwrap_or(value.len());
    (!value[..end].is_empty()).then(|| value[..end].to_string())
}

fn mask_inline_code(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut remaining = line;
    while let Some(start) = remaining.find('`') {
        output.push_str(&remaining[..start]);
        let marker_length = remaining[start..]
            .chars()
            .take_while(|character| *character == '`')
            .count();
        let marker = "`".repeat(marker_length);
        let code = &remaining[start + marker_length..];
        let Some(end) = code.find(&marker) else {
            output.push_str(&remaining[start..]);
            return output;
        };
        output.push_str(&" ".repeat(marker_length + end + marker_length));
        remaining = &code[end + marker_length..];
    }
    output.push_str(remaining);
    output
}

fn markdown_visible_lines(content: &str) -> Vec<String> {
    let mut visible = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some((marker, length)) = fence {
            if trimmed
                .chars()
                .take_while(|character| *character == marker)
                .count()
                >= length
            {
                fence = None;
            }
            continue;
        }
        if let Some((marker, length, _)) = shell_fence(trimmed) {
            fence = Some((marker, length));
            continue;
        }
        visible.push(mask_inline_code(line));
    }
    visible
}

fn reference_definition(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    let rest = line.strip_prefix('[')?;
    let (label, destination) = rest.split_once("]:")?;
    markdown_destination(destination)
        .map(|destination| (label.trim().to_ascii_lowercase(), destination))
}

fn closing_parenthesis(value: &str) -> Option<usize> {
    let mut depth = 1usize;
    let mut escaped = false;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return Some(index);
                }
            }
            _ => {}
        }
    }
    None
}

fn markdown_links(content: &str) -> Vec<MarkdownLink> {
    let lines = markdown_visible_lines(content);
    let definitions = lines
        .iter()
        .filter_map(|line| reference_definition(line))
        .collect::<BTreeMap<_, _>>();
    let mut links = Vec::new();
    for line in &lines {
        if reference_definition(line).is_some() {
            continue;
        }
        let mut cursor = 0usize;
        while let Some(relative_start) = line[cursor..].find('[') {
            let start = cursor + relative_start;
            if start > 0 && line.as_bytes()[start - 1] == b'\\' {
                cursor = start + 1;
                continue;
            }
            let Some(relative_end) = line[start + 1..].find(']') else {
                break;
            };
            let label_end = start + 1 + relative_end;
            let label = line[start + 1..label_end].trim();
            let after = label_end + 1;
            if line[after..].starts_with('(') {
                let destination = &line[after + 1..];
                let Some(end) = closing_parenthesis(destination) else {
                    break;
                };
                links.push(MarkdownLink {
                    destination: markdown_destination(&destination[..end]),
                });
                cursor = after + 1 + end + 1;
            } else if line[after..].starts_with('[') {
                let reference = &line[after + 1..];
                let Some(end) = reference.find(']') else {
                    break;
                };
                let reference_label = reference[..end].trim();
                let reference_label = if reference_label.is_empty() {
                    label
                } else {
                    reference_label
                };
                links.push(MarkdownLink {
                    destination: definitions
                        .get(&reference_label.to_ascii_lowercase())
                        .cloned(),
                });
                cursor = after + 1 + end + 1;
            } else {
                if let Some(destination) = definitions.get(&label.to_ascii_lowercase()) {
                    links.push(MarkdownLink {
                        destination: Some(destination.clone()),
                    });
                }
                cursor = after;
            }
        }
    }
    links
}

fn heading_slug(value: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in value.trim().chars() {
        if character.is_alphanumeric() || character == '_' {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            separator = false;
            slug.extend(character.to_lowercase());
        } else if character.is_whitespace() || character == '-' {
            separator = true;
        }
    }
    slug
}

fn markdown_headings(content: &str) -> Vec<MarkdownHeading> {
    let mut headings = Vec::new();
    let mut fence: Option<(char, usize)> = None;
    for (line_index, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if let Some((marker, length)) = fence {
            if trimmed
                .chars()
                .take_while(|character| *character == marker)
                .count()
                >= length
            {
                fence = None;
            }
            continue;
        }
        if let Some((marker, length, _)) = shell_fence(trimmed) {
            fence = Some((marker, length));
            continue;
        }
        let level = trimmed
            .chars()
            .take_while(|character| *character == '#')
            .count();
        if !(1..=6).contains(&level) || !trimmed[level..].starts_with(char::is_whitespace) {
            continue;
        }
        let title = trimmed[level..]
            .trim()
            .trim_end_matches('#')
            .trim()
            .to_string();
        headings.push(MarkdownHeading {
            level,
            slug: heading_slug(&title),
            title,
            line: line_index,
        });
    }
    headings
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
    content: &str,
    link: &MarkdownLink,
    planned_paths: &BTreeSet<&str>,
) -> bool {
    let Some(destination) = link.destination.as_deref() else {
        return false;
    };
    let lower = destination.to_ascii_lowercase();
    if lower.starts_with("http://") || lower.starts_with("https://") || lower.starts_with("mailto:")
    {
        return true;
    }
    if lower.contains("%2e")
        || lower.contains("%2f")
        || lower.contains("%5c")
        || destination.contains('\\')
    {
        return false;
    }
    if let Some(colon) = destination.find(':') {
        let scheme = &destination[..colon];
        if scheme
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphabetic())
            && scheme.chars().all(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '+' | '-' | '.')
            })
        {
            return false;
        }
    }
    let (path, fragment) = destination
        .split_once('#')
        .map(|(path, fragment)| (path, Some(fragment)))
        .unwrap_or((destination, None));
    let path = path.split('?').next().unwrap_or_default();
    let relative = if path.is_empty() {
        PathBuf::from(artifact_path)
    } else if let Some(relative) = resolve_artifact_relative_path(artifact_path, path) {
        relative
    } else {
        return false;
    };
    let target_bytes = if relative == Path::new(artifact_path) {
        Some(content.as_bytes().to_vec())
    } else {
        match read_without_symlinks(workspace, &relative) {
            Ok(bytes) => Some(bytes),
            Err(SnapshotError::Missing) => None,
            Err(SnapshotError::Unsafe | SnapshotError::Mismatch) => return false,
        }
    };
    let exists = target_bytes.is_some()
        || relative
            .to_str()
            .is_some_and(|path| planned_paths.contains(path));
    if !exists {
        return false;
    }
    let Some(fragment) = fragment.filter(|fragment| !fragment.is_empty()) else {
        return true;
    };
    if !path.is_empty() && !path.to_ascii_lowercase().ends_with(".md") {
        return true;
    }
    target_bytes
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .is_some_and(|target| {
            let expected = fragment.trim_start_matches('#').to_ascii_lowercase();
            markdown_headings(&target).iter().any(|heading| {
                heading.slug == expected || heading.title.to_ascii_lowercase() == expected
            })
        })
}

fn skill_link_targets_sidecar(artifact_path: &str, link: &MarkdownLink) -> bool {
    let Some(destination) = link.destination.as_deref() else {
        return false;
    };
    if destination.starts_with("http://")
        || destination.starts_with("https://")
        || destination.starts_with("mailto:")
        || destination.starts_with('#')
    {
        return false;
    }
    let target = destination
        .split('#')
        .next()
        .unwrap_or_default()
        .split('?')
        .next()
        .unwrap_or_default();
    let Some(resolved) = resolve_artifact_relative_path(artifact_path, target) else {
        return false;
    };
    let Some(skill_directory) = Path::new(artifact_path).parent() else {
        return false;
    };
    resolved != Path::new(artifact_path) && resolved.starts_with(skill_directory)
}

fn skill_directory_has_sidecars(workspace: &Path, artifact_path: &str) -> bool {
    let Some(skill_directory) = Path::new(artifact_path).parent() else {
        return true;
    };
    let target = workspace.join(artifact_path);
    fs::read_dir(workspace.join(skill_directory))
        .ok()
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .any(|entry| entry.path() != target)
}

enum SectionState {
    Missing,
    Empty,
    Present,
}

fn required_section_state(content: &str, required: &str) -> SectionState {
    let lines = content.lines().collect::<Vec<_>>();
    let headings = markdown_headings(content);
    let Some((index, heading)) = headings.iter().enumerate().find(|(_, heading)| {
        heading.title.trim() == required.trim() || heading.slug == heading_slug(required)
    }) else {
        return SectionState::Missing;
    };
    let end = headings[index + 1..]
        .iter()
        .find(|next| next.level <= heading.level)
        .map(|next| next.line)
        .unwrap_or(lines.len());
    let mut fence: Option<(char, usize)> = None;
    let mut html_comment = false;
    for line in &lines[heading.line + 1..end] {
        let mut line = line.trim();
        if let Some((marker, length)) = fence {
            if line
                .chars()
                .take_while(|character| *character == marker)
                .count()
                >= length
            {
                fence = None;
            } else if !line.is_empty() {
                return SectionState::Present;
            }
            continue;
        }
        if let Some((marker, length, _)) = shell_fence(line) {
            fence = Some((marker, length));
            continue;
        }
        if html_comment {
            if let Some(end) = line.find("-->") {
                html_comment = false;
                line = line[end + 3..].trim();
            } else {
                continue;
            }
        }
        while let Some(start) = line.find("<!--") {
            let before = line[..start].trim();
            let comment = &line[start + 4..];
            if !before.is_empty() {
                return SectionState::Present;
            }
            if let Some(end) = comment.find("-->") {
                line = comment[end + 3..].trim();
            } else {
                html_comment = true;
                line = "";
                break;
            }
        }
        let horizontal_rule = {
            let compact = line
                .chars()
                .filter(|character| !character.is_whitespace())
                .collect::<String>();
            let mut characters = compact.chars();
            let marker = characters.next();
            compact.len() >= 3
                && marker.is_some_and(|marker| matches!(marker, '-' | '*' | '_'))
                && characters.all(|character| Some(character) == marker)
        };
        if !line.is_empty()
            && !line.starts_with('#')
            && !horizontal_rule
            && !matches!(line, "-" | "*" | "+" | ">")
        {
            return SectionState::Present;
        }
    }
    SectionState::Empty
}

fn normalized_secret_key(value: &str) -> String {
    value
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn sensitive_key(value: &str) -> bool {
    matches!(
        normalized_secret_key(value).as_str(),
        "password"
            | "passwd"
            | "pwd"
            | "secret"
            | "clientsecret"
            | "token"
            | "accesstoken"
            | "refreshtoken"
            | "apikey"
            | "privatekey"
            | "authorization"
    )
}

fn safe_secret_placeholder(value: &str) -> bool {
    let value = value
        .trim()
        .trim_matches(|character: char| matches!(character, '"' | '\'' | '`' | ',' | ';'));
    if value.is_empty()
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "[redacted]" | "<redacted>" | "***" | "redacted"
        )
    {
        return true;
    }
    let ascii_identifier = |candidate: &str| {
        !candidate.is_empty()
            && candidate
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    };
    if value.starts_with("${") && value.ends_with('}') {
        return ascii_identifier(&value[2..value.len() - 1]);
    }
    if let Some(name) = value.strip_prefix('$') {
        return ascii_identifier(name);
    }
    if value.starts_with('%') && value.ends_with('%') && value.len() > 2 {
        return ascii_identifier(&value[1..value.len() - 1]);
    }
    if value.starts_with("{{") && value.ends_with("}}") {
        let expression = value[2..value.len() - 2].trim().to_ascii_lowercase();
        return expression.starts_with("env.")
            || expression.starts_with("secrets.")
            || expression.starts_with("process.env.");
    }
    let lower = value.to_ascii_lowercase();
    lower.starts_with("process.env.")
        || lower.starts_with("env:")
        || lower.starts_with("os.environ[")
}

fn explanatory_secret_value(value: &str) -> bool {
    let lower = value
        .trim()
        .trim_matches(['"', '\'', '`', ',', ';', '.'])
        .to_ascii_lowercase();
    [
        "environment variable",
        "from environment",
        "from the environment",
        "from an environment",
        "read from environment",
        "read from the environment",
        "read from an environment",
        "from env",
        "managed by a secret manager",
        "managed by the secret manager",
        "stored in a secret manager",
        "stored in the secret manager",
        "secret manager",
        "secrets manager",
        "credential manager",
        "key vault",
        "runtime injection",
        "环境变量",
        "从环境变量",
        "由环境变量",
        "通过环境变量",
        "密钥管理",
        "从密钥管理",
        "由密钥管理",
        "凭据管理",
        "从凭据管理",
        "由凭据管理",
        "运行时注入",
        "不得写入",
        "不要写入",
        "禁止写入",
    ]
    .iter()
    .any(|marker| lower.starts_with(marker))
}

fn json_value_contains_secret(value: &serde_json::Value) -> bool {
    match value {
        serde_json::Value::Object(object) => object.iter().any(|(key, value)| {
            let secret_value = sensitive_key(key)
                && match value {
                    serde_json::Value::Null => false,
                    serde_json::Value::String(value) => {
                        !safe_secret_placeholder(value) && !explanatory_secret_value(value)
                    }
                    serde_json::Value::Array(values) => values.iter().any(|value| {
                        value.as_str().is_none_or(|value| {
                            !safe_secret_placeholder(value) && !explanatory_secret_value(value)
                        })
                    }),
                    serde_json::Value::Bool(_) | serde_json::Value::Number(_) => true,
                    serde_json::Value::Object(_) => json_value_contains_secret(value),
                };
            secret_value || json_value_contains_secret(value)
        }),
        serde_json::Value::Array(values) => values.iter().any(json_value_contains_secret),
        _ => false,
    }
}

fn json_candidate_end(content: &str, start: usize) -> Option<usize> {
    let mut expected = Vec::new();
    let mut in_string = false;
    let mut escaped = false;
    for (relative, character) in content[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if character == '\\' {
                escaped = true;
            } else if character == '"' {
                in_string = false;
            }
            continue;
        }
        match character {
            '"' => in_string = true,
            '{' => expected.push('}'),
            '[' => expected.push(']'),
            '}' | ']' => {
                if expected.pop() != Some(character) {
                    return None;
                }
                if expected.is_empty() {
                    return Some(start + relative + character.len_utf8());
                }
            }
            _ => {}
        }
    }
    None
}

fn compact_json_contains_secret(content: &str) -> bool {
    let mut cursor = 0usize;
    while let Some(relative) = content[cursor..].find(['{', '[']) {
        let start = cursor + relative;
        if let Some(end) = json_candidate_end(content, start) {
            if serde_json::from_str::<serde_json::Value>(&content[start..end])
                .is_ok_and(|value| json_value_contains_secret(&value))
            {
                return true;
            }
        }
        cursor = start + 1;
    }
    false
}

fn xml_contains_secret(line: &str) -> bool {
    let mut remaining = line;
    while let Some(open) = remaining.find('<') {
        remaining = &remaining[open + 1..];
        if remaining.starts_with('/') || remaining.starts_with('!') || remaining.starts_with('?') {
            continue;
        }
        let name_end = remaining
            .find(|character: char| character.is_whitespace() || matches!(character, '>' | '/'))
            .unwrap_or(remaining.len());
        let name = &remaining[..name_end];
        let Some(tag_end) = remaining.find('>') else {
            return false;
        };
        if sensitive_key(name) && !remaining[..tag_end].trim_end().ends_with('/') {
            let value_and_rest = &remaining[tag_end + 1..];
            let value = value_and_rest.split('<').next().unwrap_or_default();
            if !safe_secret_placeholder(value) {
                return true;
            }
        }
        remaining = &remaining[tag_end + 1..];
    }
    false
}

fn url_contains_secret(line: &str) -> bool {
    let Some(scheme) = line.find("://") else {
        return false;
    };
    let authority = line[scheme + 3..]
        .split(['/', '?', '#'])
        .next()
        .unwrap_or_default();
    let Some((userinfo, _)) = authority.rsplit_once('@') else {
        return false;
    };
    userinfo
        .split_once(':')
        .is_some_and(|(_, password)| !safe_secret_placeholder(password))
}

fn token_looks_like_secret(value: &str) -> bool {
    let value = value.trim_matches(|character: char| {
        !character.is_ascii_alphanumeric() && character != '_' && character != '-'
    });
    (value.starts_with("AKIA") && value.len() >= 20)
        || ["ghp_", "github_pat_", "sk-", "xoxb-", "xoxp-"]
            .iter()
            .any(|prefix| value.starts_with(prefix) && value.len() >= prefix.len() + 12)
}

fn assignment_contains_secret(line: &str) -> bool {
    let tokens = line.split_whitespace().collect::<Vec<_>>();
    for (index, raw) in tokens.iter().enumerate() {
        let raw = raw.trim_matches(|character: char| {
            matches!(character, '"' | '\'' | '{' | '[' | '(' | ',')
        });
        let separator = raw.find(['=', ':']);
        let (key, inline_value) = separator
            .map(|position| (&raw[..position], Some(&raw[position + 1..])))
            .unwrap_or((raw, None));
        let cli_key = key.trim_start_matches('-');
        if !sensitive_key(cli_key) {
            continue;
        }
        let has_assignment = separator.is_some() || key.starts_with("--");
        if !has_assignment {
            continue;
        }
        let joined_tail;
        let mut value = if let Some(value) = inline_value.filter(|value| !value.is_empty()) {
            Some(value)
        } else if separator.is_some() {
            joined_tail = tokens[index + 1..].join(" ");
            let comment = joined_tail
                .find(" #")
                .or_else(|| joined_tail.find(" //"))
                .unwrap_or(joined_tail.len());
            Some(joined_tail[..comment].trim())
        } else {
            tokens
                .get(index + 1)
                .map(|value| value.trim_matches(['"', '\'', ',']))
        };
        if normalized_secret_key(cli_key) == "authorization"
            && value.is_some_and(|value| {
                matches!(value.to_ascii_lowercase().as_str(), "bearer" | "basic")
            })
        {
            value = tokens.get(index + 2).copied();
        }
        if value.is_some_and(|value| {
            !safe_secret_placeholder(value) && !explanatory_secret_value(value)
        }) {
            return true;
        }
    }
    false
}

fn mask_safe_template_expressions(line: &str) -> String {
    let mut output = String::with_capacity(line.len());
    let mut remaining = line;
    while let Some(start) = remaining.find("{{") {
        output.push_str(&remaining[..start]);
        let candidate = &remaining[start..];
        let Some(end) = candidate.find("}}") else {
            output.push_str(candidate);
            return output;
        };
        let expression = &candidate[..end + 2];
        if safe_secret_placeholder(expression) {
            output.push_str("$SAFE_ENV");
        } else {
            output.push_str(expression);
        }
        remaining = &candidate[end + 2..];
    }
    output.push_str(remaining);
    output
}

fn contains_secret_material(content: &str) -> bool {
    let lower = content.to_ascii_lowercase();
    if lower.lines().any(|line| {
        (line.contains("-----begin ") && line.contains(" private key-----"))
            || line.contains("putty-user-key-file")
    }) || compact_json_contains_secret(content)
    {
        return true;
    }
    content.lines().any(|line| {
        let line = mask_safe_template_expressions(line);
        let line = line.as_str();
        xml_contains_secret(line)
            || url_contains_secret(line)
            || assignment_contains_secret(line)
            || line.split_whitespace().any(token_looks_like_secret)
    })
}

fn contains_unresolved_placeholder(content: &str) -> bool {
    if ["待填写", "TODO", "TBD", "以后补充"]
        .iter()
        .any(|token| content.contains(token))
    {
        return true;
    }
    let mut remaining = content;
    while let Some(start) = remaining.find("{{") {
        let candidate = &remaining[start..];
        let Some(end) = candidate.find("}}") else {
            return true;
        };
        let expression = &candidate[..end + 2];
        if !safe_secret_placeholder(expression) {
            return true;
        }
        remaining = &candidate[end + 2..];
    }
    false
}

fn shell_assignment(value: &str) -> bool {
    value.split_once('=').is_some_and(|(name, _)| {
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    })
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct CommandReference {
    cwd: String,
    command: String,
}

fn unquote(value: &str) -> &str {
    let value = value.trim();
    if value.len() >= 2
        && ((value.starts_with('"') && value.ends_with('"'))
            || (value.starts_with('\'') && value.ends_with('\'')))
    {
        &value[1..value.len() - 1]
    } else {
        value
    }
}

fn normalize_command_cwd(base: &str, target: &str) -> Option<String> {
    let target = unquote(target).replace('\\', "/");
    if target.starts_with('/')
        || target.starts_with("//")
        || target.as_bytes().get(1).is_some_and(|byte| *byte == b':')
    {
        return None;
    }
    let mut components = if base == "." {
        Vec::new()
    } else {
        base.split('/').map(str::to_string).collect::<Vec<_>>()
    };
    for component in target.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop()?;
            }
            component => components.push(component.to_string()),
        }
    }
    Some(if components.is_empty() {
        ".".to_string()
    } else {
        components.join("/")
    })
}

fn canonical_executable(executable: &str) -> Option<String> {
    let normalized = unquote(executable).replace('\\', "/");
    let trimmed = normalized.strip_prefix("./").unwrap_or(&normalized);
    let lower = trimmed.to_ascii_lowercase();
    let canonical = match lower.as_str() {
        "npm" | "npm.cmd" | "npm.exe" => "npm",
        "pnpm" | "pnpm.cmd" | "pnpm.exe" => "pnpm",
        "yarn" | "yarn.cmd" | "yarn.exe" => "yarn",
        "mvn" | "mvnw" | "mvnw.cmd" | "mvnw.bat" => "mvn",
        "gradle" | "gradlew" | "gradlew.cmd" | "gradlew.bat" => "gradle",
        "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe" => "pwsh",
        "cargo" | "go" | "pytest" | "python" | "python3" | "make" | "npx" | "bun" | "deno"
        | "dotnet" | "bash" | "sh" | "zsh" | "node" => lower.as_str(),
        _ if normalized.starts_with("./")
            || [".sh", ".bash", ".zsh", ".ps1", ".cmd", ".bat"]
                .iter()
                .any(|extension| lower.ends_with(extension)) =>
        {
            trimmed
        }
        _ => return None,
    };
    Some(canonical.to_string())
}

fn canonicalize_command(value: &str) -> Option<String> {
    let mut tokens = value.split_whitespace();
    let executable = canonical_executable(tokens.next()?)?;
    let mut result = vec![executable];
    result.extend(tokens.map(|token| {
        let normalized = unquote(token).replace('\\', "/");
        normalized
            .strip_prefix("./")
            .unwrap_or(&normalized)
            .to_string()
    }));
    Some(result.join(" "))
}

fn normalize_command_line(value: &str) -> Option<String> {
    let mut value = value.trim();
    value = value
        .strip_prefix("$ ")
        .or_else(|| value.strip_prefix("> "))
        .unwrap_or(value)
        .trim();
    loop {
        let (first, rest) = value
            .split_once(char::is_whitespace)
            .map(|(first, rest)| (first, rest.trim_start()))
            .unwrap_or((value, ""));
        if shell_assignment(first) {
            value = rest;
            continue;
        }
        if matches!(first, "env" | "command" | "exec") {
            value = rest;
            continue;
        }
        if first == "sudo" {
            value = rest;
            while let Some(option) = value.strip_prefix('-') {
                let (flag, remainder) = option.split_once(char::is_whitespace)?;
                value = remainder.trim_start();
                if matches!(flag, "u" | "g" | "h" | "p" | "C" | "T") {
                    value = value
                        .split_once(char::is_whitespace)
                        .map(|(_, remainder)| remainder.trim_start())
                        .unwrap_or_default();
                }
            }
            continue;
        }
        break;
    }
    let (executable, arguments) = value
        .split_once(char::is_whitespace)
        .map(|(executable, arguments)| (executable, arguments.trim_start()))
        .unwrap_or((value, ""));
    let executable_lower = unquote(executable).to_ascii_lowercase();
    if matches!(executable_lower.as_str(), "cmd" | "cmd.exe") {
        let mut remaining = arguments;
        loop {
            let (option, rest) = remaining
                .split_once(char::is_whitespace)
                .map(|(option, rest)| (option, rest.trim_start()))
                .unwrap_or((remaining, ""));
            if matches!(option.to_ascii_lowercase().as_str(), "/c" | "/k") {
                return normalize_command_line(unquote(rest));
            }
            if matches!(option.to_ascii_lowercase().as_str(), "/d" | "/s" | "/q") {
                remaining = rest;
                continue;
            }
            return None;
        }
    }
    if matches!(
        executable_lower.as_str(),
        "powershell" | "powershell.exe" | "pwsh" | "pwsh.exe"
    ) {
        let (option, rest) = arguments
            .split_once(char::is_whitespace)
            .map(|(option, rest)| (option, rest.trim_start()))
            .unwrap_or((arguments, ""));
        if matches!(option.to_ascii_lowercase().as_str(), "-command" | "-c") {
            return normalize_command_line(unquote(rest));
        }
    }
    canonicalize_command(value)
}

fn change_directory(line: &str, cwd: &mut String) -> bool {
    let mut tokens = line.split_whitespace();
    let command = tokens.next().unwrap_or_default().to_ascii_lowercase();
    if !matches!(
        command.as_str(),
        "cd" | "chdir" | "set-location" | "push-location"
    ) {
        return false;
    }
    let first = tokens.next().unwrap_or_default();
    let target = if matches!(first.to_ascii_lowercase().as_str(), "/d" | "-path") {
        tokens.next().unwrap_or_default()
    } else {
        first
    };
    if let Some(resolved) = normalize_command_cwd(cwd, target) {
        *cwd = resolved;
    } else {
        *cwd = "<unsafe-cwd>".to_string();
    }
    true
}

fn collect_command_line(line: &str, cwd: &mut String, commands: &mut Vec<CommandReference>) {
    if contains_shell_composition(line) {
        commands.push(CommandReference {
            cwd: cwd.clone(),
            command: line.trim().to_string(),
        });
        return;
    }
    for part in line.split("&&").flat_map(|part| part.split(';')) {
        let part = part.trim();
        if part.is_empty() || part.starts_with('#') || part.starts_with("REM ") {
            continue;
        }
        if change_directory(part, cwd) {
            continue;
        }
        if let Some(command) = normalize_command_line(part) {
            commands.push(CommandReference {
                cwd: cwd.clone(),
                command,
            });
        }
    }
}

fn inline_command_candidates(line: &str, commands: &mut Vec<CommandReference>) {
    let mut remaining = line;
    while let Some(start) = remaining.find('`') {
        remaining = &remaining[start + 1..];
        let Some(end) = remaining.find('`') else {
            break;
        };
        let mut cwd = ".".to_string();
        collect_command_line(&remaining[..end], &mut cwd, commands);
        remaining = &remaining[end + 1..];
    }
}

fn shell_fence(trimmed: &str) -> Option<(char, usize, bool)> {
    let marker = trimmed.chars().next()?;
    if !matches!(marker, '`' | '~') {
        return None;
    }
    let length = trimmed
        .chars()
        .take_while(|character| *character == marker)
        .count();
    if length < 3 {
        return None;
    }
    let info = trimmed[length..].trim();
    let shell = info.is_empty()
        || info
            .trim_matches(['{', '}'])
            .split_whitespace()
            .map(|token| token.trim_matches(['{', '}', '.']).to_ascii_lowercase())
            .any(|token| {
                matches!(
                    token.as_str(),
                    "bash"
                        | "sh"
                        | "shell"
                        | "zsh"
                        | "console"
                        | "terminal"
                        | "powershell"
                        | "ps1"
                        | "pwsh"
                        | "cmd"
                )
            });
    Some((marker, length, shell))
}

fn command_candidates(content: &str) -> Vec<CommandReference> {
    let mut commands = Vec::new();
    let mut active_fence: Option<(char, usize, bool, String)> = None;
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some((marker, length, shell, mut cwd)) = active_fence.take() {
            if trimmed
                .chars()
                .take_while(|character| *character == marker)
                .count()
                >= length
            {
                active_fence = None;
            } else if shell {
                collect_command_line(trimmed, &mut cwd, &mut commands);
                active_fence = Some((marker, length, shell, cwd));
            } else {
                active_fence = Some((marker, length, shell, cwd));
            }
            continue;
        }
        if let Some(fence) = shell_fence(trimmed) {
            active_fence = Some((fence.0, fence.1, fence.2, ".".to_string()));
            continue;
        }
        inline_command_candidates(line, &mut commands);
    }
    commands
}

fn local_script_executable(command: &str) -> Option<&str> {
    let executable = command.split_whitespace().next()?;
    let extension = Path::new(executable)
        .extension()
        .and_then(|extension| extension.to_str())?;
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "sh" | "bash" | "zsh" | "ps1" | "cmd" | "bat" | "py" | "rb"
    )
    .then_some(executable)
}

fn contains_shell_composition(command: &str) -> bool {
    command.contains("$(")
        || command
            .chars()
            .any(|character| matches!(character, ';' | '&' | '|' | '<' | '>' | '`' | '\n' | '\r'))
}

fn repository_local_script_allowed(workspace: &Path, command: &CommandReference) -> bool {
    if contains_shell_composition(&command.command) {
        return false;
    }
    let Some(executable) = local_script_executable(&command.command) else {
        return false;
    };
    let Some(relative) = normalize_command_cwd(&command.cwd, executable) else {
        return false;
    };
    let Ok(relative) = normalized_relative_path(&relative) else {
        return false;
    };
    read_project_bytes_handle_safe(workspace, relative).is_ok_and(|bytes| bytes.is_some())
}

fn evidence_is_cited_together(content: &str, path: &str, symbol: &str) -> bool {
    fn inline_code_contains(block: &str, expected: &str) -> bool {
        let mut remaining = block;
        while let Some(start) = remaining.find('`') {
            remaining = &remaining[start + 1..];
            let Some(end) = remaining.find('`') else {
                return false;
            };
            if remaining[..end].trim() == expected {
                return true;
            }
            remaining = &remaining[end + 1..];
        }
        false
    }

    let mut block = String::new();
    for line in content.lines().chain(std::iter::once("")) {
        if line.trim().is_empty() {
            if inline_code_contains(&block, path) && inline_code_contains(&block, symbol) {
                return true;
            }
            block.clear();
        } else {
            block.push_str(line);
            block.push('\n');
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
    let plan_secret = plan_contains_secret(plan);
    let mut issues = workspace_integrity_issues(workspace, inventory, "validate");
    if plan_secret {
        issues.push(issue(
            "plan.secret.detected",
            "产物计划包含疑似敏感配置值，已拒绝",
            None,
            "validate",
        ));
    }
    let known_commands = inventory
        .commands
        .iter()
        .filter_map(|command| {
            Some(CommandReference {
                cwd: normalize_command_cwd(".", &command.cwd)?,
                command: normalize_command_line(&command.command)?,
            })
        })
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
        let content = match read_without_symlinks(workspace, relative) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(content) => content,
                Err(_) => {
                    issues.push(issue(
                        "artifact.file.invalid-text",
                        "计划产物不是有效的 UTF-8 文本",
                        Some(&artifact.target_path),
                        "validate",
                    ));
                    continue;
                }
            },
            Err(SnapshotError::Missing) => {
                issues.push(issue(
                    "artifact.file.missing",
                    "无法读取计划产物",
                    Some(&artifact.target_path),
                    "validate",
                ));
                continue;
            }
            Err(SnapshotError::Unsafe | SnapshotError::Mismatch) => {
                issues.push(issue(
                    "artifact.file.unsafe",
                    "计划产物包含链接、特殊文件或不安全路径",
                    Some(&artifact.target_path),
                    "validate",
                ));
                continue;
            }
        };
        let secret_detected = contains_secret_material(&content);
        if contains_unresolved_placeholder(&content) {
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
            match required_section_state(&content, section) {
                SectionState::Missing => issues.push(issue(
                    "artifact.section.missing",
                    format!("产物缺少必需章节：{section}"),
                    Some(&artifact.target_path),
                    "validate",
                )),
                SectionState::Empty => issues.push(issue(
                    "artifact.section.empty",
                    format!("产物必需章节没有真实内容：{section}"),
                    Some(&artifact.target_path),
                    "validate",
                )),
                SectionState::Present => {}
            }
        }
        let links = markdown_links(&content);
        if artifact.kind == ArtifactKind::Skill
            && (skill_directory_has_sidecars(workspace, &artifact.target_path)
                || links
                    .iter()
                    .any(|link| skill_link_targets_sidecar(&artifact.target_path, link)))
        {
            issues.push(issue(
                "artifact.skill.resource-external",
                "项目 skill 的资源必须直接内嵌在 SKILL.md，不能生成或引用旁路资源文件",
                Some(&artifact.target_path),
                "validate",
            ));
        }
        for link in links {
            if !link_exists(
                workspace,
                &artifact.target_path,
                &content,
                &link,
                &planned_paths,
            ) {
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
            if !known_commands.contains(&command)
                && !repository_local_script_allowed(workspace, &command)
            {
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
    if plan_secret {
        redact_plan_diagnostics(&mut issues);
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project_factory::docs::ProjectLayers;
    use crate::project_factory::types::{
        CoverageExclusion, EvidenceReference, InventoryFile, ProjectCommand, ProjectInventory,
        ProjectModule,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    const AUTH_SOURCE: &str = "class AuthService {}";

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
                    size: AUTH_SOURCE.len() as u64,
                    sha256: crate::project_factory::inventory::content_sha256(
                        AUTH_SOURCE.as_bytes(),
                    ),
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
        let mut plan = ArtifactPlan {
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
        };
        let skill = plan
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.kind == ArtifactKind::Skill)
            .expect("skill");
        skill.rationale = "项目资源全部内嵌在 SKILL.md 中，避免未受计划约束的旁路文件".into();
        skill.required_sections.push("项目资源".into());
        plan
    }

    fn codes(issues: &[ValidationIssue]) -> Vec<&str> {
        issues.iter().map(|issue| issue.code.as_str()).collect()
    }

    fn valid_document_content(artifact: &ArtifactPlanItem, extra: &str) -> String {
        format!(
            "# 项目事实\n\n## 真实证据\n\n`{}` 与 `{}` 共同证明当前项目实现。\n\n## 验证方式\n\n按清单执行源码核验与项目测试。\n\n{}\n\n{extra}",
            artifact.evidence[0].path,
            artifact.evidence[0].symbol.as_deref().expect("symbol"),
            "当前项目的边界、复用入口、风险和验证方式都必须依据真实实现。".repeat(6),
        )
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

    #[cfg(unix)]
    #[test]
    fn evidence_rejects_a_symlinked_parent_even_when_the_final_file_is_regular() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        fixture.write("actual/java/AuthService.java", AUTH_SOURCE);
        fs::create_dir_all(fixture.path().join("iam-service/src")).expect("source parent");
        symlink(
            fixture.path().join("actual"),
            fixture.path().join("iam-service/src/main"),
        )
        .expect("parent symlink");

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &valid_plan());

        assert!(codes(&issues).contains(&"plan.evidence.unsafe"));
    }

    #[test]
    fn evidence_and_workspace_sources_must_match_the_inventory_snapshot() {
        let fixture = Fixture::new();
        fixture.write(
            "iam-service/src/main/java/AuthService.java",
            "class AuthService { int changed; }",
        );
        let plan = valid_plan();

        let plan_issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);
        assert!(codes(&plan_issues).contains(&"plan.evidence.snapshot-mismatch"));
        assert!(codes(&plan_issues).contains(&"workspace.source.modified"));

        let artifact = &plan.artifacts[0];
        fixture.write(&artifact.target_path, &valid_document_content(artifact, ""));
        let staged_issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );
        assert!(codes(&staged_issues).contains(&"workspace.source.modified"));
    }

    #[test]
    fn evidence_symbols_must_look_like_real_identifiers_or_configuration_keys() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        plan.artifacts[0].evidence[0].symbol = Some("class AuthService {}".into());

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

        assert!(codes(&issues).contains(&"plan.evidence.symbol-invalid"));
    }

    #[test]
    fn staged_evidence_requires_associated_inline_code_path_and_symbol() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &format!(
                "# 项目地图\n\n## 真实证据\n\n路径 {} 与符号 AuthService 共同证明入口。\n\n## 验证方式\n\n{}",
                artifact.evidence[0].path,
                "当前项目边界、复用入口、风险和验证都来自真实实现。".repeat(8)
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
    fn plan_secret_formats_are_rejected_without_echoing_values() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let secret = "never-print-plan-secret";
        let mut plan = valid_plan();
        plan.artifacts[0].rationale = format!("Authorization: Bearer {secret}");
        plan.artifacts[1].evidence[0].symbol = Some(format!("password={secret}"));

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

        assert!(codes(&issues).contains(&"plan.secret.detected"));
        assert!(issues.iter().all(|issue| !issue.detail.contains(secret)));
    }

    #[test]
    fn artifact_secret_formats_are_rejected_without_echoing_values() {
        let cases = [
            ("xml-secret-value", "<password>xml-secret-value</password>"),
            ("cli-secret-value", "tool --token cli-secret-value"),
            (
                "url-secret-value",
                "https://user:url-secret-value@example.test/api",
            ),
            (
                "bearer-secret-value",
                "Authorization: Bearer bearer-secret-value",
            ),
            ("yaml-secret-value", "- apiKey: yaml-secret-value"),
        ];
        for (secret, declaration) in cases {
            let fixture = Fixture::new();
            fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
            let mut plan = valid_plan();
            plan.artifacts.truncate(1);
            let artifact = &plan.artifacts[0];
            fixture.write(
                &artifact.target_path,
                &valid_document_content(artifact, declaration),
            );

            let issues = validate_staged_artifacts(
                fixture.path(),
                &inventory(false, true),
                &plan,
                Some(ArtifactKind::Document),
            );

            assert!(
                codes(&issues).contains(&"artifact.secret.detected"),
                "declaration was accepted"
            );
            assert!(issues.iter().all(|issue| !issue.detail.contains(secret)));
        }
    }

    #[test]
    fn environment_secret_placeholders_are_allowed() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "password=${DB_PASSWORD}\ntoken=$API_TOKEN\nsecret={{ env.SECRET }}\napiKey=%API_KEY%",
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );
        let codes = codes(&issues);

        assert!(!codes.contains(&"artifact.secret.detected"));
        assert!(!codes.contains(&"artifact.content.placeholder"));
    }

    #[test]
    fn pure_frontend_api_client_evidence_can_support_an_integration_artifact() {
        const CLIENT_SOURCE: &str = "export function callPaymentApi() {}";
        let fixture = Fixture::new();
        fixture.write("web/src/api/payment-client.ts", CLIENT_SOURCE);
        let inventory = ProjectInventory {
            schema_version: 1,
            project_name: "iam".into(),
            layers: ProjectLayers {
                frontend: true,
                backend: false,
            },
            modules: vec![ProjectModule {
                name: "web".into(),
                path: "web".into(),
                kind: "frontend".into(),
                manifests: vec!["web/package.json".into()],
                source_roots: vec!["web/src".into()],
            }],
            source_roots: vec!["web/src".into()],
            files: vec![InventoryFile {
                path: "web/src/api/payment-client.ts".into(),
                kind: "source".into(),
                size: CLIENT_SOURCE.len() as u64,
                sha256: crate::project_factory::inventory::content_sha256(CLIENT_SOURCE.as_bytes()),
                module: Some("web".into()),
            }],
            commands: vec![],
            risk_keys: vec![],
        };
        let mut artifact = item(
            "payment-api-integration",
            ArtifactKind::Document,
            "docs/ai/payment-api-integration.md",
            "payment-api-client",
        );
        artifact.layer = "integration".into();
        artifact.evidence = vec![EvidenceReference {
            path: "web/src/api/payment-client.ts".into(),
            symbol: Some("callPaymentApi".into()),
        }];
        artifact.covers = vec!["web".into(), "web/src".into()];
        let plan = ArtifactPlan {
            schema_version: 1,
            project_name: "iam".into(),
            artifacts: vec![artifact],
            exclusions: vec![],
        };

        let issues = validate_artifact_plan(fixture.path(), &inventory, &plan);
        let codes = codes(&issues);

        assert!(!codes.contains(&"plan.layer.mismatch"));
        assert!(!codes.contains(&"plan.topic.unsupported"));
    }

    #[test]
    fn ambiguous_component_terms_follow_backend_evidence_instead_of_a_word_list() {
        const REGISTRY_SOURCE: &str = "class ComponentRegistry {}";
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        fixture.write(
            "iam-service/src/main/java/ComponentRegistry.java",
            REGISTRY_SOURCE,
        );
        let mut inventory = inventory(false, true);
        inventory.files.push(InventoryFile {
            path: "iam-service/src/main/java/ComponentRegistry.java".into(),
            kind: "source".into(),
            size: REGISTRY_SOURCE.len() as u64,
            sha256: crate::project_factory::inventory::content_sha256(REGISTRY_SOURCE.as_bytes()),
            module: Some("iam-service".into()),
        });
        let mut plan = valid_plan();
        let mut artifact = item(
            "component-registry",
            ArtifactKind::Rule,
            ".claude/rules/project/component-registry.md",
            "component-registry",
        );
        artifact.evidence = vec![EvidenceReference {
            path: "iam-service/src/main/java/ComponentRegistry.java".into(),
            symbol: Some("ComponentRegistry".into()),
        }];
        plan.artifacts.push(artifact);

        let issues = validate_artifact_plan(fixture.path(), &inventory, &plan);
        let codes = codes(&issues);

        assert!(!codes.contains(&"plan.layer.mismatch"));
        assert!(!codes.contains(&"plan.topic.unsupported"));
    }

    #[test]
    fn common_and_contract_layers_require_real_scope_evidence() {
        const CONTRACT_SOURCE: &str = "interface AuthContract {}";
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        fixture.write(
            "iam-service/src/main/java/api/AuthContract.java",
            CONTRACT_SOURCE,
        );
        let mut inventory = inventory(false, true);
        inventory.files.push(InventoryFile {
            path: "iam-service/src/main/java/api/AuthContract.java".into(),
            kind: "source".into(),
            size: CONTRACT_SOURCE.len() as u64,
            sha256: crate::project_factory::inventory::content_sha256(CONTRACT_SOURCE.as_bytes()),
            module: Some("iam-service".into()),
        });
        let mut plan = valid_plan();
        plan.artifacts
            .iter_mut()
            .find(|artifact| artifact.id == "auth-lifecycle")
            .expect("auth rule")
            .layer = "common".into();
        let mut contract = item(
            "auth-contract",
            ArtifactKind::Document,
            "docs/ai/auth-contract.md",
            "auth-contract",
        );
        contract.layer = "contract".into();
        contract.evidence = vec![EvidenceReference {
            path: "iam-service/src/main/java/api/AuthContract.java".into(),
            symbol: Some("AuthContract".into()),
        }];
        plan.artifacts.push(contract);

        let issues = validate_artifact_plan(fixture.path(), &inventory, &plan);

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "plan.layer.mismatch")
                .count(),
            1,
            "only the common rule should be rejected: {issues:#?}"
        );
    }

    #[test]
    fn adaptive_topics_must_share_project_tokens_with_their_evidence() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        plan.artifacts.push(item(
            "vue-state-flow",
            ArtifactKind::Document,
            "docs/ai/vue-state-flow.md",
            "vue-state-flow",
        ));

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

        assert!(codes(&issues).contains(&"plan.topic.unsupported"));
    }

    #[test]
    fn generic_debug_clean_code_bug_fix_and_refactor_packs_are_rejected() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        for (id, topic) in [
            ("iam-debugging", "debug-workflow"),
            ("iam-clean-code", "clean-code"),
            ("iam-bug-fix", "bug-fix"),
            ("iam-refactor", "refactor-workflow"),
        ] {
            plan.artifacts.push(item(
                id,
                ArtifactKind::Skill,
                &format!(".claude/skills/{id}/SKILL.md"),
                topic,
            ));
        }

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "plan.skill.generic")
                .count(),
            4
        );
    }

    #[test]
    fn command_validation_handles_tilde_fences_attributes_env_and_wrappers() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "~~~{.bash title=\"operations\"}\nFEATURE=1 npx imaginary-tool\nenv MODE=test bun run missing\ndeno task absent\ndotnet test Missing.csproj\npwsh -File missing.ps1\nsudo -E make imaginary\n~~~",
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "artifact.command.unknown")
                .count(),
            6
        );
    }

    #[test]
    fn environment_prefixes_do_not_hide_a_known_command() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut inventory = inventory(false, true);
        inventory.commands.push(ProjectCommand {
            name: "test".into(),
            command: "npm run test".into(),
            cwd: ".".into(),
        });
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "```bash title=\"test\"\nNODE_ENV=$NODE_ENV npm run test\n```",
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory,
            &plan,
            Some(ArtifactKind::Document),
        );

        assert!(!codes(&issues).contains(&"artifact.command.unknown"));
    }

    #[test]
    fn markdown_links_validate_titles_references_and_heading_fragments() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        fixture.write(
            "docs/ai/architecture-boundaries.md",
            "# 架构边界\n\n## 认证边界\n\n真实内容。",
        );
        fixture.write(
            "docs/ai/known-risks-and-document-drift.md",
            "# 风险\n\n## 已知风险\n\n真实内容。",
        );
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "[边界](architecture-boundaries.md#认证边界 \"架构说明\")\n\n[风险][risk-ref]\n\n[risk-ref]: known-risks-and-document-drift.md#已知风险 \"风险说明\"\n\n[本节](#验证方式)",
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
    fn markdown_links_reject_missing_inline_and_reference_fragments() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        fixture.write(
            "docs/ai/architecture-boundaries.md",
            "# 架构边界\n\n正文。\n",
        );
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "[缺失](architecture-boundaries.md#不存在 \"说明\")\n\n[引用缺失][missing-ref]\n\n[missing-ref]: architecture-boundaries.md#仍不存在",
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "artifact.link.dangling")
                .count(),
            2
        );
    }

    #[test]
    fn required_sections_must_be_real_nonempty_markdown_headings() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        plan.artifacts.truncate(2);
        let missing_heading = &plan.artifacts[0];
        fixture.write(
            &missing_heading.target_path,
            &format!(
                "# 项目地图\n\n`{}` 与 `AuthService`。真实证据和验证方式只是正文词语。\n\n{}",
                missing_heading.evidence[0].path,
                "当前项目内容必须完整且来自真实实现。".repeat(8)
            ),
        );
        let empty_heading = &plan.artifacts[1];
        fixture.write(
            &empty_heading.target_path,
            &format!(
                "# 边界\n\n`{}` 与 `AuthService` 共同证明项目边界。\n\n{}\n\n## 真实证据\n\n## 验证方式\n",
                empty_heading.evidence[0].path,
                "当前项目内容必须完整且来自真实实现。".repeat(8)
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );
        let codes = codes(&issues);

        assert!(codes.contains(&"artifact.section.missing"));
        assert!(codes.contains(&"artifact.section.empty"));
    }

    #[test]
    fn project_skills_must_declare_and_use_inline_resources_only() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        let skill = plan
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.kind == ArtifactKind::Skill)
            .expect("skill");
        skill.rationale = "项目资源全部内嵌在 SKILL.md 中，避免未受计划约束的旁路文件".into();
        skill.required_sections.push("项目资源".into());
        let skill_path = skill.target_path.clone();
        fixture.write(
            ".claude/skills/iam-auth-change-review/references/checklist.md",
            "# 外置清单",
        );
        fixture.write(
            &skill_path,
            &format!(
                "---\nname: iam-auth-change-review\ndescription: 认证变更检查。\n---\n\n# 认证变更\n\n## 真实证据\n\n`{}` 与 `AuthService`。\n\n## 验证方式\n运行项目验证。\n\n## 项目资源\n[外置清单](references/checklist.md)\n\n## 执行流程\n执行认证检查。\n\n## 完成 Gate\n验证认证结果。\n\n## 失败处理\n失败时停止。\n\n{}",
                skill.evidence[0].path,
                "认证资源和流程均来自当前项目实现。".repeat(8)
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Skill),
        );

        assert!(codes(&issues).contains(&"artifact.skill.resource-external"));
    }

    #[test]
    fn project_skill_plan_requires_an_inline_resource_reason_and_section() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        let skill = plan
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.kind == ArtifactKind::Skill)
            .expect("skill");
        skill.rationale = "项目真实边界需要长期记录".into();
        skill
            .required_sections
            .retain(|section| section != "项目资源");

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

        assert!(codes(&issues).contains(&"plan.skill.resources-inline-required"));
    }

    #[test]
    fn evidence_symbols_reject_language_keywords_and_common_role_tokens() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        plan.artifacts[0].evidence[0].symbol = Some("class".into());
        plan.artifacts[1].evidence[0].symbol = Some("Service".into());

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "plan.evidence.symbol-invalid")
                .count(),
            2
        );
    }

    #[test]
    fn evidence_symbols_must_be_declarations_not_comment_mentions() {
        const COMMENT_ONLY: &str = "// AuthService is discussed here, but never declared.";
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", COMMENT_ONLY);
        let mut inventory = inventory(false, true);
        inventory.files[1].size = COMMENT_ONLY.len() as u64;
        inventory.files[1].sha256 = content_sha256(COMMENT_ONLY.as_bytes());

        let issues = validate_artifact_plan(fixture.path(), &inventory, &valid_plan());

        assert!(codes(&issues).contains(&"plan.evidence.symbol-missing"));
    }

    #[test]
    fn configuration_key_symbols_are_accepted_when_actually_declared() {
        const CONFIG: &str = "security.tenant-id: ${TENANT_ID}";
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        fixture.write("iam-service/config/application.yml", CONFIG);
        let mut inventory = inventory(false, true);
        inventory.files.push(InventoryFile {
            path: "iam-service/config/application.yml".into(),
            kind: "config".into(),
            size: CONFIG.len() as u64,
            sha256: content_sha256(CONFIG.as_bytes()),
            module: Some("iam-service".into()),
        });
        let mut plan = valid_plan();
        plan.artifacts[0].evidence = vec![EvidenceReference {
            path: "iam-service/config/application.yml".into(),
            symbol: Some("security.tenant-id".into()),
        }];

        let issues = validate_artifact_plan(fixture.path(), &inventory, &plan);

        assert!(!codes(&issues).contains(&"plan.evidence.symbol-invalid"));
        assert!(!codes(&issues).contains(&"plan.evidence.symbol-missing"));
    }

    #[test]
    fn rationale_can_bind_project_concepts_to_verified_evidence_symbols() {
        const ORGANIZATION: &str = "class OrganizationService {}";
        const FILTER: &str = "class InnerApiFilter {}";
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        fixture.write(
            "iam-service/src/main/java/OrganizationService.java",
            ORGANIZATION,
        );
        fixture.write("iam-service/src/main/java/InnerApiFilter.java", FILTER);
        let mut inventory = inventory(false, true);
        for (path, source) in [
            (
                "iam-service/src/main/java/OrganizationService.java",
                ORGANIZATION,
            ),
            ("iam-service/src/main/java/InnerApiFilter.java", FILTER),
        ] {
            inventory.files.push(InventoryFile {
                path: path.into(),
                kind: "source".into(),
                size: source.len() as u64,
                sha256: content_sha256(source.as_bytes()),
                module: Some("iam-service".into()),
            });
        }
        let mut plan = valid_plan();
        for (id, topic, path, symbol, rationale) in [
            (
                "iam-tenant-boundary",
                "iam-tenant",
                "iam-service/src/main/java/OrganizationService.java",
                "OrganizationService",
                "IAM tenant 边界由 OrganizationService 的真实声明负责",
            ),
            (
                "security-filter",
                "security",
                "iam-service/src/main/java/InnerApiFilter.java",
                "InnerApiFilter",
                "security 入口由 InnerApiFilter 的真实声明负责",
            ),
        ] {
            let mut artifact = item(
                id,
                ArtifactKind::Rule,
                &format!(".claude/rules/project/backend/{id}.md"),
                topic,
            );
            artifact.rationale = rationale.into();
            artifact.evidence = vec![EvidenceReference {
                path: path.into(),
                symbol: Some(symbol.into()),
            }];
            plan.artifacts.push(artifact);
        }

        let issues = validate_artifact_plan(fixture.path(), &inventory, &plan);

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "plan.topic.unsupported")
                .count(),
            0,
            "verified rationale relationships should be accepted: {issues:#?}"
        );
    }

    #[test]
    fn node_fullstack_single_package_can_prove_a_backend_layer() {
        const SERVER: &str = "export class ServerGateway {}";
        let fixture = Fixture::new();
        fixture.write("src/server/ServerGateway.ts", SERVER);
        let inventory = ProjectInventory {
            schema_version: 1,
            project_name: "iam".into(),
            layers: ProjectLayers {
                frontend: true,
                backend: true,
            },
            modules: vec![ProjectModule {
                name: "root".into(),
                path: ".".into(),
                kind: "frontend".into(),
                manifests: vec!["package.json".into()],
                source_roots: vec!["src".into()],
            }],
            source_roots: vec!["src".into()],
            files: vec![InventoryFile {
                path: "src/server/ServerGateway.ts".into(),
                kind: "source".into(),
                size: SERVER.len() as u64,
                sha256: content_sha256(SERVER.as_bytes()),
                module: Some("root".into()),
            }],
            commands: vec![],
            risk_keys: vec![],
        };
        let mut artifact = item(
            "server-gateway",
            ArtifactKind::Rule,
            ".claude/rules/project/backend/server-gateway.md",
            "server-gateway",
        );
        artifact.evidence = vec![EvidenceReference {
            path: "src/server/ServerGateway.ts".into(),
            symbol: Some("ServerGateway".into()),
        }];
        artifact.covers = vec!["root".into(), "src".into()];
        let plan = ArtifactPlan {
            schema_version: 1,
            project_name: "iam".into(),
            artifacts: vec![artifact],
            exclusions: vec![],
        };

        let issues = validate_artifact_plan(fixture.path(), &inventory, &plan);

        assert!(!codes(&issues).contains(&"plan.layer.mismatch"));
    }

    #[test]
    fn project_skills_reject_assistant_feature_maintenance_and_style_packs() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        for id in [
            "auth-coding-assistant",
            "feature-implementation",
            "maintenance",
            "coding-style",
        ] {
            plan.artifacts.push(item(
                id,
                ArtifactKind::Skill,
                &format!(".claude/skills/{id}/SKILL.md"),
                id,
            ));
        }

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "plan.skill.generic")
                .count(),
            4
        );
    }

    #[test]
    fn skill_target_must_have_exactly_one_directory_below_skills() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        let skill = plan
            .artifacts
            .iter_mut()
            .find(|artifact| artifact.kind == ArtifactKind::Skill)
            .expect("skill");
        skill.target_path = ".claude/skills/iam-auth-change-review/nested/SKILL.md".into();

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

        assert!(codes(&issues).contains(&"plan.path.outside-allowlist"));
    }

    #[cfg(unix)]
    #[test]
    fn staged_artifact_reads_reject_a_symlinked_final_file() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            "staged-real.md",
            &valid_document_content(artifact, "真实产物不得经由链接读取。"),
        );
        fs::create_dir_all(fixture.path().join("docs/ai")).expect("artifact parent");
        symlink(
            "../../staged-real.md",
            fixture.path().join(&artifact.target_path),
        )
        .expect("staged symlink");

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert!(codes(&issues).contains(&"artifact.file.unsafe"));
    }

    #[cfg(unix)]
    #[test]
    fn markdown_fragment_reads_reject_symlink_targets() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        fixture.write("docs/real.md", "# 真实文档\n\n## 目标章节\n\n内容。\n");
        fs::create_dir_all(fixture.path().join("docs/ai")).expect("docs parent");
        symlink("../real.md", fixture.path().join("docs/ai/linked.md")).expect("fragment symlink");
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(artifact, "[链接目标](linked.md#目标章节)"),
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
    fn compact_json_secrets_are_detected_in_every_nested_field_without_echo() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let secret = "compact-json-secret-value";
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                &format!(
                    r#"{{"ordinary":"value","nested":{{"region":"cn","clientSecret":"{secret}"}},"tail":"kept"}}"#
                ),
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
    fn all_private_key_headers_are_rejected_and_plan_diagnostics_are_redacted() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let markers = [
            "-----BEGIN EC PRIVATE KEY-----",
            "-----BEGIN DSA PRIVATE KEY-----",
            "-----BEGIN ENCRYPTED PRIVATE KEY-----",
        ];
        for marker in markers {
            let mut plan = valid_plan();
            plan.artifacts[0].rationale = format!("项目证据 {marker} forbidden-plan-value");

            let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

            assert!(codes(&issues).contains(&"plan.secret.detected"));
            assert!(issues.iter().all(|issue| !issue.detail.contains(marker)));
            assert!(issues
                .iter()
                .all(|issue| !issue.detail.contains("forbidden-plan-value")));
        }
    }

    #[test]
    fn secret_manager_and_environment_explanations_are_not_secret_values() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let explanation = "password: 从环境变量读取，不得写入文档。\nsecret: managed by a secret manager.\ntoken: read from an environment variable.";
        for line in explanation.lines() {
            assert!(
                !contains_secret_material(line),
                "explanatory line was treated as a literal secret: {line}"
            );
        }
        let mut plan = valid_plan();
        plan.artifacts[0].rationale = format!("项目配置说明：{explanation}");
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(artifact, explanation),
        );

        let plan_issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);
        let staged_issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert!(!codes(&plan_issues).contains(&"plan.secret.detected"));
        assert!(!codes(&staged_issues).contains(&"artifact.secret.detected"));
    }

    #[test]
    fn command_validation_requires_the_exact_inventory_cwd() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut inventory = inventory(false, true);
        inventory.commands.push(ProjectCommand {
            name: "test".into(),
            command: "npm run test".into(),
            cwd: "packages/console".into(),
        });
        let mut plan = valid_plan();
        plan.artifacts.truncate(2);
        fixture.write(
            &plan.artifacts[0].target_path,
            &valid_document_content(&plan.artifacts[0], "```bash\nnpm run test\n```"),
        );
        fixture.write(
            &plan.artifacts[1].target_path,
            &valid_document_content(
                &plan.artifacts[1],
                "```bash\ncd packages/admin\nnpm run test\n```",
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory,
            &plan,
            Some(ArtifactKind::Document),
        );

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "artifact.command.unknown")
                .count(),
            2
        );
    }

    #[test]
    fn command_validation_understands_windows_wrappers_and_local_scripts() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut inventory = inventory(false, true);
        inventory.commands = vec![
            ProjectCommand {
                name: "test".into(),
                command: "npm run test".into(),
                cwd: "packages/console".into(),
            },
            ProjectCommand {
                name: "gradle-test".into(),
                command: "gradle test".into(),
                cwd: ".".into(),
            },
            ProjectCommand {
                name: "powershell-check".into(),
                command: "pwsh -File scripts/check.ps1".into(),
                cwd: ".".into(),
            },
            ProjectCommand {
                name: "shell-check".into(),
                command: "bash scripts/check.sh".into(),
                cwd: ".".into(),
            },
        ];
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "~~~powershell\ncd /d packages\\console\ncmd.exe /c npm.cmd run test\ncd /d ..\\..\n.\\gradlew.bat test\npowershell.exe -File scripts\\check.ps1\nbash ./scripts/check.sh\n./scripts/missing.sh\n~~~",
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory,
            &plan,
            Some(ArtifactKind::Document),
        );

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "artifact.command.unknown")
                .count(),
            1,
            "only the missing local script should be rejected: {issues:#?}"
        );
    }

    #[test]
    fn every_local_link_requires_a_safe_existing_target_and_supported_protocol() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        fixture.write("docs/ai/existing.png", "png-placeholder");
        fixture.write("outside.png", "outside-placeholder");
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "[存在图片](existing.png)\n\n[缺失图片](missing.png)\n\n[越界图片](../../../outside.png)\n\n[危险协议](javascript:alert(1))",
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "artifact.link.dangling")
                .count(),
            3
        );
    }

    #[test]
    fn markdown_shortcut_and_collapsed_references_are_validated() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        fixture.write(
            "docs/ai/architecture-boundaries.md",
            "# 架构边界\n\n## 认证边界\n\n真实内容。",
        );
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "[边界][] 与 [认证]。\n\n[边界]: architecture-boundaries.md#认证边界\n[认证]: architecture-boundaries.md#认证边界",
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
    fn missing_shortcut_reference_targets_are_rejected() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "参见 [缺失资源]。\n\n[缺失资源]: missing-resource.png",
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "artifact.link.dangling")
                .count(),
            1
        );
    }

    #[test]
    fn markdown_links_inside_code_examples_are_ignored() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "示例语法 `[标签](missing-inline.md)` 不是真实链接。\n\n```markdown\n[标签](missing-fenced.md)\n```",
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
    fn empty_fences_and_horizontal_rules_do_not_fill_required_sections() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &format!(
                "# 项目事实\n\n`{}` 与 `AuthService` 共同证明当前项目。\n\n{}\n\n## 真实证据\n\n```bash\n```\n\n## 验证方式\n\n---",
                artifact.evidence[0].path,
                "当前项目边界与验证均来自真实实现。".repeat(8)
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "artifact.section.empty")
                .count(),
            2
        );
    }

    #[test]
    fn evidence_symbols_reject_calls_inside_strings_and_comments() {
        const STRING_AND_COMMENT: &str = r#"class Holder {
    String example = "DangerousFeature(";
    // DangerousFeature(input) is documentation, not a declaration.
}"#;
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        fixture.write("iam-service/src/main/java/Holder.java", STRING_AND_COMMENT);
        let mut inventory = inventory(false, true);
        inventory.files.push(InventoryFile {
            path: "iam-service/src/main/java/Holder.java".into(),
            kind: "source".into(),
            size: STRING_AND_COMMENT.len() as u64,
            sha256: content_sha256(STRING_AND_COMMENT.as_bytes()),
            module: Some("iam-service".into()),
        });
        let mut plan = valid_plan();
        plan.artifacts[0].evidence = vec![EvidenceReference {
            path: "iam-service/src/main/java/Holder.java".into(),
            symbol: Some("DangerousFeature".into()),
        }];

        let issues = validate_artifact_plan(fixture.path(), &inventory, &plan);

        assert!(codes(&issues).contains(&"plan.evidence.symbol-missing"));
    }

    #[test]
    fn declaration_lines_ignore_comment_bodies_with_declaration_shapes() {
        assert!(!line_declares_symbol(
            "// public void DangerousFeature() {}",
            "DangerousFeature"
        ));
        assert!(!line_declares_symbol(
            "/* public void DangerousFeature() {} */",
            "DangerousFeature"
        ));
        assert!(line_declares_symbol(
            "public void DangerousFeature() {}",
            "DangerousFeature"
        ));
        assert!(!content_declares_symbol(
            "/*\npublic void DangerousFeature() {}\n*/",
            "DangerousFeature"
        ));
        assert!(content_declares_symbol(
            "/* public void OtherFeature() {} */\npublic void DangerousFeature() {}",
            "DangerousFeature"
        ));
    }

    #[test]
    fn evidence_symbols_reject_bare_calls_and_multiline_string_declarations() {
        fn symbol_is_missing(source: &str, symbol: &str) -> bool {
            let fixture = Fixture::new();
            fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
            fixture.write("iam-service/src/main/java/Holder.java", source);
            let mut inventory = inventory(false, true);
            inventory.files.push(InventoryFile {
                path: "iam-service/src/main/java/Holder.java".into(),
                kind: "source".into(),
                size: source.len() as u64,
                sha256: content_sha256(source.as_bytes()),
                module: Some("iam-service".into()),
            });
            let mut plan = valid_plan();
            plan.artifacts[0].evidence = vec![EvidenceReference {
                path: "iam-service/src/main/java/Holder.java".into(),
                symbol: Some(symbol.into()),
            }];

            codes(&validate_artifact_plan(fixture.path(), &inventory, &plan))
                .contains(&"plan.evidence.symbol-missing")
        }

        assert_eq!(
            [
                symbol_is_missing("DangerousFeature();", "DangerousFeature"),
                symbol_is_missing(
                    "const docs = `\nfunction DangerousFeature() {}\n`;\nfunction RealFeature() {}",
                    "DangerousFeature"
                ),
                symbol_is_missing(
                    "DOC = \"\"\"\ndef DangerousFeature():\n    pass\n\"\"\"\ndef RealFeature():\n    pass",
                    "DangerousFeature"
                ),
                symbol_is_missing(
                    "const docs = `\nfunction DangerousFeature() {}\n`;\nfunction RealFeature() {}",
                    "RealFeature"
                ),
                symbol_is_missing(
                    "DOC = '''\ndef DangerousFeature():\n    pass\n'''\ndef RealFeature():\n    pass",
                    "RealFeature"
                ),
            ],
            [true, true, true, false, false]
        );
    }

    #[test]
    fn fullstack_pages_api_and_src_api_handlers_prove_backend_layers() {
        const PAGES_HANDLER: &str = "export function getUsers() { return []; }";
        const SRC_HANDLER: &str = "export const sessionHandler = () => ({ ok: true });";
        let fixture = Fixture::new();
        fixture.write("pages/api/users.ts", PAGES_HANDLER);
        fixture.write("src/api/session.ts", SRC_HANDLER);
        let inventory = ProjectInventory {
            schema_version: 1,
            project_name: "iam".into(),
            layers: ProjectLayers {
                frontend: true,
                backend: true,
            },
            modules: vec![ProjectModule {
                name: "root".into(),
                path: ".".into(),
                kind: "frontend".into(),
                manifests: vec!["package.json".into()],
                source_roots: vec!["pages".into(), "src".into()],
            }],
            source_roots: vec!["pages".into(), "src".into()],
            files: vec![
                InventoryFile {
                    path: "pages/api/users.ts".into(),
                    kind: "source".into(),
                    size: PAGES_HANDLER.len() as u64,
                    sha256: content_sha256(PAGES_HANDLER.as_bytes()),
                    module: Some("root".into()),
                },
                InventoryFile {
                    path: "src/api/session.ts".into(),
                    kind: "source".into(),
                    size: SRC_HANDLER.len() as u64,
                    sha256: content_sha256(SRC_HANDLER.as_bytes()),
                    module: Some("root".into()),
                },
            ],
            commands: vec![],
            risk_keys: vec![],
        };
        let artifacts = [
            ("users-api", "pages/api/users.ts", "getUsers", "pages"),
            ("session-api", "src/api/session.ts", "sessionHandler", "src"),
        ]
        .into_iter()
        .map(|(id, path, symbol, root)| {
            let mut artifact = item(
                id,
                ArtifactKind::Rule,
                &format!(".claude/rules/project/backend/{id}.md"),
                id,
            );
            artifact.evidence = vec![EvidenceReference {
                path: path.into(),
                symbol: Some(symbol.into()),
            }];
            artifact.covers = vec!["root".into(), root.into()];
            artifact
        })
        .collect();
        let plan = ArtifactPlan {
            schema_version: 1,
            project_name: "iam".into(),
            artifacts,
            exclusions: vec![],
        };

        let issues = validate_artifact_plan(fixture.path(), &inventory, &plan);

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "plan.layer.mismatch")
                .count(),
            0
        );
    }

    #[test]
    fn frontend_router_routes_do_not_prove_a_backend_layer() {
        const ROUTES: &str = "export const accountRoute = { path: '/account' };";
        let fixture = Fixture::new();
        fixture.write("src/router/route.ts", ROUTES);
        let inventory = ProjectInventory {
            schema_version: 1,
            project_name: "iam".into(),
            layers: ProjectLayers {
                frontend: true,
                backend: true,
            },
            modules: vec![ProjectModule {
                name: "root".into(),
                path: ".".into(),
                kind: "frontend".into(),
                manifests: vec!["package.json".into()],
                source_roots: vec!["src".into()],
            }],
            source_roots: vec!["src".into()],
            files: vec![InventoryFile {
                path: "src/router/route.ts".into(),
                kind: "source".into(),
                size: ROUTES.len() as u64,
                sha256: content_sha256(ROUTES.as_bytes()),
                module: Some("root".into()),
            }],
            commands: vec![],
            risk_keys: vec![],
        };
        let mut artifact = item(
            "account-route",
            ArtifactKind::Rule,
            ".claude/rules/project/backend/account-route.md",
            "account-route",
        );
        artifact.evidence = vec![EvidenceReference {
            path: "src/router/route.ts".into(),
            symbol: Some("accountRoute".into()),
        }];
        artifact.covers = vec!["root".into(), "src".into()];
        let plan = ArtifactPlan {
            schema_version: 1,
            project_name: "iam".into(),
            artifacts: vec![artifact],
            exclusions: vec![],
        };

        let issues = validate_artifact_plan(fixture.path(), &inventory, &plan);

        assert!(codes(&issues).contains(&"plan.layer.mismatch"));
    }

    #[test]
    fn generic_engineering_capability_categories_are_rejected_with_project_prefixes() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        for id in [
            "auth-programming-helper",
            "iam-development-guide",
            "billing-engineering-assistant",
        ] {
            plan.artifacts.push(item(
                id,
                ArtifactKind::Skill,
                &format!(".claude/skills/{id}/SKILL.md"),
                id,
            ));
        }
        plan.artifacts.push(item(
            "auth-programming-guide",
            ArtifactKind::Rule,
            ".claude/rules/project/auth-programming-guide.md",
            "auth-programming-guide",
        ));
        plan.artifacts.push(item(
            "tenant-import-guide",
            ArtifactKind::Skill,
            ".claude/skills/tenant-import-guide/SKILL.md",
            "tenant-import-guide",
        ));

        let issues = validate_artifact_plan(fixture.path(), &inventory(false, true), &plan);

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "plan.skill.generic")
                .count(),
            3
        );
        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "plan.rule.generic")
                .count(),
            1
        );
        assert!(!issues.iter().any(|issue| {
            issue.code == "plan.skill.generic"
                && issue.path.as_deref() == Some(".claude/skills/tenant-import-guide/SKILL.md")
        }));
    }

    #[test]
    fn embedded_json_secrets_are_found_inside_curl_and_arbitrary_text() {
        let cases = [
            "curl -d '{\"user\":\"iam\",\"password\":\"curl-secret-value\"}' https://example.test",
            "request payload={\"safe\":\"value\",\"nested\":{\"apiKey\":\"embedded-secret-value\"}} follows",
        ];
        for declaration in cases {
            let fixture = Fixture::new();
            fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
            let mut plan = valid_plan();
            plan.artifacts.truncate(1);
            let artifact = &plan.artifacts[0];
            fixture.write(
                &artifact.target_path,
                &valid_document_content(artifact, declaration),
            );

            let issues = validate_staged_artifacts(
                fixture.path(),
                &inventory(false, true),
                &plan,
                Some(ArtifactKind::Document),
            );

            assert!(
                codes(&issues).contains(&"artifact.secret.detected"),
                "embedded JSON was accepted: {declaration}"
            );
        }
    }

    #[test]
    fn explanatory_tail_does_not_hide_a_real_assignment_value() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "password: real-secret-value # 后续改为环境变量注入",
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert!(codes(&issues).contains(&"artifact.secret.detected"));
    }

    #[cfg(unix)]
    #[test]
    fn repository_local_scripts_are_resolved_handle_safely_from_command_cwd() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        fixture.write("scripts/verify.sh", "#!/bin/sh\nexit 0\n");
        symlink("verify.sh", fixture.path().join("scripts/linked-verify.sh"))
            .expect("script symlink");
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "```bash\n./scripts/verify.sh --root\ncd iam-service\n../scripts/verify.sh --module\n./scripts/verify.sh --wrong-cwd\ncd ..\n./scripts/linked-verify.sh\n```",
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "artifact.command.unknown")
                .count(),
            2,
            "only the wrong-cwd and symlink scripts should be rejected: {issues:#?}"
        );
    }

    #[test]
    fn repository_local_script_fallback_rejects_shell_composition() {
        let fixture = Fixture::new();
        fixture.write("scripts/verify.sh", "#!/bin/sh\nexit 0\n");
        let unsafe_commands = [
            "./scripts/verify.sh ; echo injected",
            "./scripts/verify.sh && echo injected",
            "./scripts/verify.sh || echo injected",
            "./scripts/verify.sh | sh",
            "./scripts/verify.sh > result.txt",
            "./scripts/verify.sh < input.txt",
            "./scripts/verify.sh `echo injected`",
            "./scripts/verify.sh $(echo injected)",
        ];

        for command in unsafe_commands {
            assert!(
                !repository_local_script_allowed(
                    fixture.path(),
                    &CommandReference {
                        cwd: ".".into(),
                        command: command.into(),
                    }
                ),
                "shell composition was accepted: {command}"
            );
        }
        assert!(repository_local_script_allowed(
            fixture.path(),
            &CommandReference {
                cwd: ".".into(),
                command: "./scripts/verify.sh --root".into(),
            }
        ));
    }

    #[test]
    fn staged_local_script_commands_reject_shell_composition() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        fixture.write("scripts/verify.sh", "#!/bin/sh\nexit 0\n");
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "```bash
./scripts/verify.sh ; echo injected
./scripts/verify.sh && echo injected
./scripts/verify.sh || echo injected
./scripts/verify.sh | sh
./scripts/verify.sh > result.txt
./scripts/verify.sh < input.txt
./scripts/verify.sh `echo injected`
./scripts/verify.sh $(echo injected)
```",
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "artifact.command.unknown")
                .count(),
            8,
            "every composed local-script command should be rejected: {issues:#?}"
        );
    }

    #[test]
    fn staged_local_scripts_reject_composition_before_the_script() {
        let fixture = Fixture::new();
        fixture.write("iam-service/src/main/java/AuthService.java", AUTH_SOURCE);
        fixture.write("scripts/verify.sh", "#!/bin/sh\nexit 0\n");
        let mut plan = valid_plan();
        plan.artifacts.truncate(1);
        let artifact = &plan.artifacts[0];
        fixture.write(
            &artifact.target_path,
            &valid_document_content(
                artifact,
                "```bash
echo injected && ./scripts/verify.sh
TOKEN=$(whoami) ./scripts/verify.sh
```",
            ),
        );

        let issues = validate_staged_artifacts(
            fixture.path(),
            &inventory(false, true),
            &plan,
            Some(ArtifactKind::Document),
        );

        assert_eq!(
            issues
                .iter()
                .filter(|issue| issue.code == "artifact.command.unknown")
                .count(),
            2,
            "composition before a local script must stay visible: {issues:#?}"
        );
    }
}
