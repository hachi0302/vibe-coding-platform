use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use super::docs::ProjectLayers;
use super::types::{
    InventoryFile, ProjectCommand, ProjectInventory, ProjectModule, SensitiveFinding,
};

const INVENTORY_SCHEMA_VERSION: u32 = 1;
const MAX_FILE_SIZE_BYTES: u64 = 1024 * 1024;
const MAX_SCAN_DEPTH: usize = 128;
const REDACTED: &str = "[REDACTED]";

struct ScannedFile {
    path: String,
    kind: String,
    bytes: Vec<u8>,
}

pub fn content_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn inspect_project(root: &Path) -> Result<ProjectInventory, String> {
    validate_root(root)?;
    let mut paths = collect_files(root)?;
    paths.sort();

    let mut scanned = Vec::new();
    let mut risk_keys = Vec::new();
    for path in paths {
        let relative = safe_relative(root, &path)?;
        let metadata = fs::symlink_metadata(&path)
            .map_err(|error| format!("Cannot inspect {relative}: {error}"))?;
        if !metadata.file_type().is_file()
            || metadata.len() > MAX_FILE_SIZE_BYTES
            || excluded_file(&path)
        {
            continue;
        }
        let bytes = fs::read(&path).map_err(|error| format!("Cannot read {relative}: {error}"))?;
        if binary(&bytes) || private_key_content(&bytes) {
            continue;
        }
        let (bytes, keys) = if configuration_file(&path) {
            redact_configuration(&bytes)
        } else {
            (bytes, Vec::new())
        };
        risk_keys.extend(keys.into_iter().map(|key| SensitiveFinding {
            path: relative.clone(),
            key,
            kind: "redacted-config-value".to_string(),
        }));
        scanned.push(ScannedFile {
            path: relative,
            kind: file_kind(&path),
            bytes,
        });
    }
    scanned.sort_by(|left, right| left.path.cmp(&right.path));
    risk_keys.sort_by(|left, right| (&left.path, &left.key).cmp(&(&right.path, &right.key)));
    risk_keys.dedup_by(|left, right| left.path == right.path && left.key == right.key);

    let source_roots = source_roots(&scanned);
    let modules = modules(&scanned, &source_roots);
    let files = scanned
        .iter()
        .map(|file| InventoryFile {
            path: file.path.clone(),
            kind: file.kind.clone(),
            size: file.bytes.len() as u64,
            sha256: content_sha256(&file.bytes),
            module: owner(&file.path, &modules),
        })
        .collect();

    Ok(ProjectInventory {
        schema_version: INVENTORY_SCHEMA_VERSION,
        project_name: root
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project")
            .to_string(),
        layers: layers(&scanned),
        modules,
        source_roots,
        files,
        commands: commands(&scanned, root),
        risk_keys,
    })
}

pub fn create_filtered_workspace(
    root: &Path,
    workspace: &Path,
    inventory: &ProjectInventory,
) -> Result<(), String> {
    validate_root(root)?;
    if fs::symlink_metadata(workspace).is_ok() {
        return Err(format!("Workspace already exists: {}", workspace.display()));
    }
    if let Ok(relative) = workspace.strip_prefix(root) {
        no_workspace_parent_symlinks(root, relative)?;
    }
    fs::create_dir_all(workspace)
        .map_err(|error| format!("Cannot create workspace {}: {error}", workspace.display()))?;
    let result = copy_inventory(root, workspace, inventory);
    if result.is_err() {
        let _ = fs::remove_dir_all(workspace);
    }
    result
}

fn copy_inventory(
    root: &Path,
    workspace: &Path,
    inventory: &ProjectInventory,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for file in &inventory.files {
        if !seen.insert(file.path.as_str()) {
            return Err(format!("Duplicate inventory path: {}", file.path));
        }
        let relative = inventory_path(&file.path)?;
        no_source_symlinks(root, &relative)?;
        let source = root.join(&relative);
        let metadata = fs::symlink_metadata(&source)
            .map_err(|error| format!("Cannot inspect {}: {error}", file.path))?;
        if !metadata.file_type().is_file()
            || metadata.len() > MAX_FILE_SIZE_BYTES
            || excluded_file(&source)
        {
            return Err(format!("Unsafe inventory source: {}", file.path));
        }
        let bytes =
            fs::read(&source).map_err(|error| format!("Cannot read {}: {error}", file.path))?;
        if binary(&bytes) || private_key_content(&bytes) {
            return Err(format!("Inventory source became excluded: {}", file.path));
        }
        let safe_bytes = if configuration_file(&source) {
            redact_configuration(&bytes).0
        } else {
            bytes
        };
        if content_sha256(&safe_bytes) != file.sha256 {
            return Err(format!(
                "Project file changed after inventory: {}",
                file.path
            ));
        }
        let destination = workspace.join(&relative);
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Cannot create {}: {error}", parent.display()))?;
        }
        fs::write(&destination, safe_bytes)
            .map_err(|error| format!("Cannot write {}: {error}", destination.display()))?;
        fs::set_permissions(&destination, metadata.permissions()).map_err(|error| {
            format!(
                "Cannot preserve permissions for {}: {error}",
                destination.display()
            )
        })?;
    }
    Ok(())
}

fn validate_root(root: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| format!("Cannot inspect root {}: {error}", root.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(format!(
            "Project root is not a real directory: {}",
            root.display()
        ));
    }
    Ok(())
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut result = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0usize)];
    while let Some((directory, depth)) = pending.pop() {
        if depth > MAX_SCAN_DEPTH {
            continue;
        }
        let entries = fs::read_dir(&directory)
            .map_err(|error| format!("Cannot scan {}: {error}", directory.display()))?;
        for entry in entries {
            let entry =
                entry.map_err(|error| format!("Cannot scan {}: {error}", directory.display()))?;
            let file_type = entry.file_type().map_err(|error| error.to_string())?;
            if file_type.is_symlink() {
                continue;
            }
            if file_type.is_dir() {
                if !excluded_directory(&entry.file_name().to_string_lossy()) {
                    pending.push((entry.path(), depth + 1));
                }
            } else if file_type.is_file() {
                result.push(entry.path());
            }
        }
    }
    Ok(result)
}

fn excluded_directory(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        ".git"
            | ".vibe-coding-platform"
            | "node_modules"
            | "target"
            | "dist"
            | "dist-ssr"
            | "build"
            | "out"
            | "coverage"
            | ".next"
            | ".nuxt"
            | ".svelte-kit"
            | ".cache"
            | ".gradle"
            | "vendor"
            | "__pycache__"
            | ".venv"
            | "venv"
    )
}

fn excluded_file(path: &Path) -> bool {
    let name = filename(path);
    if matches!(
        name.as_str(),
        "id_rsa"
            | "id_dsa"
            | "id_ecdsa"
            | "id_ed25519"
            | ".netrc"
            | "credentials"
            | "credentials.json"
    ) {
        return true;
    }
    matches!(
        extension(path).as_str(),
        "pem"
            | "key"
            | "p12"
            | "pfx"
            | "jks"
            | "keystore"
            | "der"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "ico"
            | "bmp"
            | "tiff"
            | "mp3"
            | "wav"
            | "flac"
            | "mp4"
            | "mov"
            | "avi"
            | "mkv"
            | "pdf"
            | "zip"
            | "gz"
            | "bz2"
            | "xz"
            | "7z"
            | "jar"
            | "war"
            | "class"
            | "o"
            | "a"
            | "so"
            | "dylib"
            | "dll"
            | "exe"
    )
}

fn binary(bytes: &[u8]) -> bool {
    bytes.contains(&0) || std::str::from_utf8(bytes).is_err()
}

fn private_key_content(bytes: &[u8]) -> bool {
    let text = String::from_utf8_lossy(bytes);
    [
        "-----BEGIN PRIVATE KEY-----",
        "-----BEGIN RSA PRIVATE KEY-----",
        "-----BEGIN EC PRIVATE KEY-----",
        "-----BEGIN OPENSSH PRIVATE KEY-----",
    ]
    .iter()
    .any(|header| text.contains(header))
}

fn configuration_file(path: &Path) -> bool {
    let name = filename(path);
    name == ".env"
        || name.starts_with(".env.")
        || matches!(
            extension(path).as_str(),
            "yaml" | "yml" | "toml" | "json" | "properties" | "ini" | "conf" | "config"
        )
}

fn redact_configuration(bytes: &[u8]) -> (Vec<u8>, Vec<String>) {
    let text = String::from_utf8_lossy(bytes);
    let mut result = String::with_capacity(text.len());
    let mut keys = Vec::new();
    for line in text.split_inclusive('\n') {
        let (body, newline) = match line.strip_suffix('\n') {
            Some(body) => (
                body.strip_suffix('\r').unwrap_or(body),
                if body.ends_with('\r') { "\r\n" } else { "\n" },
            ),
            None => (line, ""),
        };
        if let Some((redacted, key)) = redact_line(body) {
            result.push_str(&redacted);
            keys.push(key);
        } else {
            result.push_str(body);
        }
        result.push_str(newline);
    }
    (result.into_bytes(), keys)
}

fn redact_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    if trimmed.is_empty()
        || trimmed.starts_with('#')
        || trimmed.starts_with("//")
        || trimmed.starts_with(';')
    {
        return None;
    }
    let delimiter = match (line.find('='), line.find(':')) {
        (Some(left), Some(right)) => left.min(right),
        (Some(index), None) | (None, Some(index)) => index,
        (None, None) => return None,
    };
    let raw_key = line[..delimiter].trim();
    let key = raw_key
        .trim_matches(['"', '\''])
        .rsplit('.')
        .next()
        .unwrap_or(raw_key)
        .trim()
        .to_string();
    let raw_value = &line[delimiter + 1..];
    let value = raw_value.trim();
    if value.is_empty() || matches!(value, "{" | "[" | "}" | "]") {
        return None;
    }
    if !sensitive_key(&key) && !connection_string(value) {
        return None;
    }
    let whitespace = &raw_value[..raw_value.len() - raw_value.trim_start().len()];
    let suffix = if value.ends_with(',') { "," } else { "" };
    let quote = value
        .chars()
        .next()
        .filter(|character| matches!(character, '"' | '\''));
    let replacement = quote
        .map(|quote| format!("{quote}{REDACTED}{quote}{suffix}"))
        .unwrap_or_else(|| format!("{REDACTED}{suffix}"));
    Some((
        format!("{}{}{}", &line[..delimiter + 1], whitespace, replacement),
        key,
    ))
}

fn sensitive_key(key: &str) -> bool {
    let key = key
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect::<String>();
    [
        "password",
        "passwd",
        "pwd",
        "secret",
        "token",
        "apikey",
        "privatekey",
        "credential",
        "accesskey",
        "clientsecret",
        "connectionstring",
        "databaseurl",
    ]
    .iter()
    .any(|candidate| key.contains(candidate))
}

fn connection_string(value: &str) -> bool {
    let value = value.trim_matches(['"', '\'']).to_ascii_lowercase();
    [
        "jdbc:",
        "mongodb://",
        "mongodb+srv://",
        "redis://",
        "rediss://",
        "postgres://",
        "postgresql://",
        "mysql://",
        "mariadb://",
        "amqp://",
        "amqps://",
    ]
    .iter()
    .any(|prefix| value.starts_with(prefix))
        || value.split_once("://").is_some_and(|(_, authority)| {
            authority
                .split('/')
                .next()
                .is_some_and(|part| part.contains('@'))
        })
}

fn file_kind(path: &Path) -> String {
    let name = filename(path);
    if manifest(&name) {
        "manifest"
    } else if path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some("test" | "tests" | "__tests__")
        )
    }) || name.contains(".test.")
        || name.contains(".spec.")
        || name.ends_with("_test.rs")
    {
        "test"
    } else if configuration_file(path) {
        "config"
    } else if matches!(extension(path).as_str(), "md" | "mdx" | "rst" | "adoc") {
        "document"
    } else if extension(path) == "sql" {
        "database"
    } else if matches!(
        extension(path).as_str(),
        "rs" | "java"
            | "kt"
            | "kts"
            | "scala"
            | "go"
            | "py"
            | "js"
            | "jsx"
            | "ts"
            | "tsx"
            | "vue"
            | "svelte"
            | "c"
            | "cc"
            | "cpp"
            | "h"
            | "hpp"
            | "cs"
            | "rb"
            | "php"
            | "swift"
    ) {
        "source"
    } else {
        "other"
    }
    .to_string()
}

fn manifest(name: &str) -> bool {
    matches!(
        name,
        "pom.xml"
            | "build.gradle"
            | "build.gradle.kts"
            | "settings.gradle"
            | "settings.gradle.kts"
            | "package.json"
            | "cargo.toml"
            | "go.mod"
            | "pyproject.toml"
            | "requirements.txt"
            | "setup.py"
            | "composer.json"
            | "gemfile"
            | "build.sbt"
    )
}

fn source_roots(files: &[ScannedFile]) -> Vec<String> {
    let mut roots = BTreeSet::new();
    for file in files
        .iter()
        .filter(|file| matches!(file.kind.as_str(), "source" | "test"))
    {
        let parts = file.path.split('/').collect::<Vec<_>>();
        for index in 0..parts.len().saturating_sub(1) {
            let end = if parts[index] == "src"
                && parts.get(index + 1) == Some(&"main")
                && matches!(
                    parts.get(index + 2),
                    Some(&"java" | &"kotlin" | &"scala" | &"groovy")
                ) {
                index + 3
            } else if matches!(
                parts[index],
                "src" | "app" | "pages" | "lib" | "test" | "tests"
            ) {
                index + 1
            } else {
                continue;
            };
            roots.insert(parts[..end].join("/"));
            break;
        }
    }
    roots.into_iter().collect()
}

fn modules(files: &[ScannedFile], roots: &[String]) -> Vec<ProjectModule> {
    let mut grouped: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for file in files {
        if manifest(&filename(Path::new(&file.path))) {
            grouped
                .entry(parent(&file.path))
                .or_default()
                .push(file.path.clone());
        }
    }
    grouped
        .into_iter()
        .map(|(path, mut manifests)| {
            manifests.sort();
            let kind = module_kind(&manifests, files);
            let name = if path == "." {
                "root".to_string()
            } else {
                Path::new(&path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("module")
                    .to_string()
            };
            let source_roots = roots
                .iter()
                .filter(|root| contains_path(&path, root))
                .cloned()
                .collect();
            ProjectModule {
                name,
                path,
                kind,
                manifests,
                source_roots,
            }
        })
        .collect()
}

fn module_kind(manifests: &[String], files: &[ScannedFile]) -> String {
    if manifests.iter().any(|path| {
        path.ends_with("package.json")
            && files
                .iter()
                .find(|file| file.path == *path)
                .is_some_and(|file| frontend_package(&file.bytes))
    }) {
        "frontend"
    } else if manifests.iter().any(|path| {
        path.ends_with("pom.xml")
            || path.ends_with("build.gradle")
            || path.ends_with("build.gradle.kts")
    }) {
        "backend"
    } else if manifests.iter().any(|path| path.ends_with("Cargo.toml")) {
        "rust"
    } else if manifests.iter().any(|path| path.ends_with("go.mod")) {
        "go"
    } else if manifests.iter().any(|path| {
        path.ends_with("pyproject.toml")
            || path.ends_with("requirements.txt")
            || path.ends_with("setup.py")
    }) {
        "python"
    } else {
        "package"
    }
    .to_string()
}

fn layers(files: &[ScannedFile]) -> ProjectLayers {
    let frontend = files.iter().any(|file| {
        let name = filename(Path::new(&file.path));
        matches!(
            name.as_str(),
            "app.vue" | "app.tsx" | "page.tsx" | "tauri.conf.json"
        ) || (name == "package.json" && frontend_package(&file.bytes))
    });
    let backend = files.iter().any(|file| {
        let name = filename(Path::new(&file.path));
        let content = String::from_utf8_lossy(&file.bytes).to_ascii_lowercase();
        name == "pom.xml"
            || name == "app.module.ts"
            || name == "manage.py"
            || (matches!(name.as_str(), "build.gradle" | "build.gradle.kts")
                && [
                    "org.springframework.boot",
                    "io.ktor",
                    "io.micronaut",
                    "io.quarkus",
                ]
                .iter()
                .any(|value| content.contains(value)))
            || (matches!(
                name.as_str(),
                "pyproject.toml" | "requirements.txt" | "setup.py"
            ) && ["fastapi", "flask", "django", "starlette"]
                .iter()
                .any(|value| content.contains(value)))
            || (name == "package.json"
                && ["@nestjs", "\"express\"", "\"fastify\"", "\"koa\""]
                    .iter()
                    .any(|value| content.contains(value)))
            || (name == "cargo.toml"
                && [
                    "axum",
                    "actix-web",
                    "rocket",
                    "warp",
                    "tonic",
                    "sqlx",
                    "diesel",
                    "sea-orm",
                ]
                .iter()
                .any(|value| content.contains(value)))
    });
    ProjectLayers { frontend, backend }
}

fn frontend_package(bytes: &[u8]) -> bool {
    let content = String::from_utf8_lossy(bytes).to_ascii_lowercase();
    [
        "\"vue\"",
        "\"react\"",
        "\"next\"",
        "\"svelte\"",
        "\"@angular/core\"",
        "\"nuxt\"",
        "\"astro\"",
        "\"solid-js\"",
    ]
    .iter()
    .any(|value| content.contains(value))
}

fn commands(files: &[ScannedFile], root: &Path) -> Vec<ProjectCommand> {
    let mut result = BTreeSet::new();
    for file in files.iter().filter(|file| file.kind == "manifest") {
        let cwd = parent(&file.path);
        match filename(Path::new(&file.path)).as_str() {
            "pom.xml" => {
                result.insert((cwd, "test".to_string(), "mvn test".to_string()));
            }
            "build.gradle" | "build.gradle.kts" => {
                result.insert((cwd, "test".to_string(), "gradle test".to_string()));
            }
            "cargo.toml" => {
                result.insert((cwd, "test".to_string(), "cargo test".to_string()));
            }
            "go.mod" => {
                result.insert((cwd, "test".to_string(), "go test ./...".to_string()));
            }
            "pyproject.toml" | "requirements.txt" | "setup.py" => {
                result.insert((cwd, "test".to_string(), "pytest".to_string()));
            }
            "package.json" => {
                if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&file.bytes) {
                    let manager = if root.join(join_module(&cwd, "pnpm-lock.yaml")).is_file() {
                        "pnpm"
                    } else if root.join(join_module(&cwd, "yarn.lock")).is_file() {
                        "yarn"
                    } else {
                        "npm"
                    };
                    if let Some(scripts) =
                        value.get("scripts").and_then(|scripts| scripts.as_object())
                    {
                        for name in ["test", "lint", "typecheck", "build"] {
                            if scripts.contains_key(name) {
                                result.insert((
                                    cwd.clone(),
                                    name.to_string(),
                                    format!("{manager} run {name}"),
                                ));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
    result
        .into_iter()
        .map(|(cwd, name, command)| ProjectCommand { name, command, cwd })
        .collect()
}

fn owner(path: &str, modules: &[ProjectModule]) -> Option<String> {
    modules
        .iter()
        .filter(|module| contains_path(&module.path, path))
        .max_by_key(|module| {
            if module.path == "." {
                0
            } else {
                module.path.split('/').count()
            }
        })
        .map(|module| module.path.clone())
}

fn contains_path(parent: &str, child: &str) -> bool {
    parent == "." || child == parent || child.starts_with(&format!("{parent}/"))
}

fn parent(path: &str) -> String {
    Path::new(path)
        .parent()
        .and_then(|path| path.to_str())
        .filter(|path| !path.is_empty())
        .unwrap_or(".")
        .replace('\\', "/")
}

fn join_module(module: &str, name: &str) -> PathBuf {
    if module == "." {
        PathBuf::from(name)
    } else {
        Path::new(module).join(name)
    }
}

fn filename(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|value| value.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}

fn safe_relative(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map_err(|_| format!("Path escaped root: {}", path.display()))?
        .components()
        .map(|component| match component {
            Component::Normal(value) => value
                .to_str()
                .map(str::to_string)
                .ok_or_else(|| format!("Non-UTF-8 path: {}", path.display())),
            _ => Err(format!("Unsafe path: {}", path.display())),
        })
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join("/"))
}

fn inventory_path(path: &str) -> Result<PathBuf, String> {
    let relative = Path::new(path);
    if path.is_empty()
        || path.contains('\\')
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(format!("Unsafe inventory path: {path}"));
    }
    Ok(relative.to_path_buf())
}

fn no_source_symlinks(root: &Path, relative: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Err("Unsafe source path".to_string());
        };
        current.push(value);
        let metadata = fs::symlink_metadata(&current)
            .map_err(|error| format!("Cannot inspect {}: {error}", current.display()))?;
        if metadata.file_type().is_symlink() {
            return Err(format!(
                "Symlink source is not allowed: {}",
                current.display()
            ));
        }
    }
    Ok(())
}

fn no_workspace_parent_symlinks(root: &Path, relative: &Path) -> Result<(), String> {
    let mut current = root.to_path_buf();
    for component in relative.components() {
        let Component::Normal(value) = component else {
            return Err("Unsafe workspace path".to_string());
        };
        current.push(value);
        match fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "Symlink workspace parent is not allowed: {}",
                    current.display()
                ));
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(format!(
                    "Workspace parent is not a directory: {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => {
                return Err(format!("Cannot inspect {}: {error}", current.display()));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    struct Fixture {
        root: PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock")
                .as_nanos();
            let sequence = FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "vibe-inventory-{name}-{}-{nonce}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(&root).expect("create fixture root");
            Self { root }
        }

        fn path(&self) -> &Path {
            &self.root
        }

        fn write(&self, relative: &str, content: impl AsRef<[u8]>) {
            let path = self.root.join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create fixture parent");
            }
            fs::write(path, content).expect("write fixture file");
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn inspects_nested_maven_and_vue_projects_without_dependency_caches() {
        let fixture = Fixture::new("maven-vue");
        fixture.write(
            "pom.xml",
            r#"<project><packaging>pom</packaging><modules><module>services/iam</module></modules></project>"#,
        );
        fixture.write(
            "services/iam/pom.xml",
            r#"<project><artifactId>iam</artifactId></project>"#,
        );
        fixture.write(
            "services/iam/src/main/java/example/IamApplication.java",
            "final class IamApplication {}",
        );
        fixture.write(
            "apps/web/package.json",
            r#"{"scripts":{"build":"vite build","test":"vitest"},"dependencies":{"vue":"3.5.0"}}"#,
        );
        fixture.write("apps/web/src/router/index.ts", "export const routes = [];");
        fixture.write(
            "apps/web/node_modules/vue/package.json",
            r#"{"name":"vue"}"#,
        );

        let inventory = inspect_project(fixture.path()).expect("inventory");

        assert!(inventory.layers.backend);
        assert!(inventory.layers.frontend);
        assert!(inventory
            .modules
            .iter()
            .any(|module| module.path == "services/iam"));
        assert!(inventory
            .files
            .iter()
            .any(|file| file.path == "apps/web/src/router/index.ts"));
        assert!(!inventory
            .files
            .iter()
            .any(|file| file.path.contains("node_modules")));
    }

    #[cfg(unix)]
    #[test]
    fn skips_symlink_escapes_and_cycles_in_inventory_and_workspace() {
        use std::os::unix::fs::symlink;

        let fixture = Fixture::new("links");
        let outside = Fixture::new("outside");
        fixture.write("src/main.rs", "fn main() {}\n");
        outside.write("outside-secret.txt", "must never be copied");
        symlink(outside.path(), fixture.path().join("outside-link")).expect("outside symlink");
        symlink(fixture.path(), fixture.path().join("loop")).expect("loop symlink");

        let inventory = inspect_project(fixture.path()).expect("inventory");
        assert_eq!(
            inventory
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["src/main.rs"]
        );

        let workspace_parent = Fixture::new("link-workspace");
        let workspace = workspace_parent.path().join("workspace");
        create_filtered_workspace(fixture.path(), &workspace, &inventory).expect("workspace");
        assert!(workspace.join("src/main.rs").is_file());
        assert!(!workspace.join("outside-link").exists());
        assert!(!workspace.join("loop").exists());

        symlink(outside.path(), fixture.path().join(".vibe-coding-platform"))
            .expect("workspace parent escape symlink");
        let escaped_workspace = fixture
            .path()
            .join(".vibe-coding-platform/runs/run/workspace");
        assert!(create_filtered_workspace(fixture.path(), &escaped_workspace, &inventory).is_err());
        assert!(!outside.path().join("runs").exists());
    }

    #[test]
    fn excludes_private_key_files_from_inventory_and_workspace() {
        let fixture = Fixture::new("private-key");
        fixture.write("src/lib.rs", "pub fn ready() -> bool { true }\n");
        fixture.write(
            "secrets/server.pem",
            "-----BEGIN PRIVATE KEY-----\nnot-a-real-key\n-----END PRIVATE KEY-----\n",
        );
        fixture.write("secrets/id_ed25519", "not-a-real-key\n");

        let inventory = inspect_project(fixture.path()).expect("inventory");
        assert!(!inventory
            .files
            .iter()
            .any(|file| file.path.starts_with("secrets/")));

        let workspace_parent = Fixture::new("private-key-workspace");
        let workspace = workspace_parent.path().join("workspace");
        create_filtered_workspace(fixture.path(), &workspace, &inventory).expect("workspace");
        assert!(!workspace.join("secrets/server.pem").exists());
        assert!(!workspace.join("secrets/id_ed25519").exists());
    }

    #[test]
    fn redacts_only_sensitive_configuration_values_in_the_workspace() {
        let fixture = Fixture::new("redaction");
        fixture.write(
            "config/application.yml",
            concat!(
                "spring:\n",
                "  datasource:\n",
                "    username: iam-app\n",
                "    password: do-not-copy\n",
                "feature-enabled: true\n",
                "api-token: abc-123\n",
            ),
        );

        let inventory = inspect_project(fixture.path()).expect("inventory");
        let workspace_parent = Fixture::new("redaction-workspace");
        let workspace = workspace_parent.path().join("workspace");
        create_filtered_workspace(fixture.path(), &workspace, &inventory).expect("workspace");

        let copied = fs::read_to_string(workspace.join("config/application.yml"))
            .expect("copied configuration");
        assert!(copied.contains("spring:\n  datasource:"));
        assert!(copied.contains("username: iam-app"));
        assert!(copied.contains("feature-enabled: true"));
        assert!(copied.contains("password: [REDACTED]"));
        assert!(copied.contains("api-token: [REDACTED]"));
        assert!(!copied.contains("do-not-copy"));
        assert!(!copied.contains("abc-123"));
        assert!(inventory.risk_keys.iter().any(|finding| {
            finding.path == "config/application.yml" && finding.key == "password"
        }));
        assert!(inventory.risk_keys.iter().any(|finding| {
            finding.path == "config/application.yml" && finding.key == "api-token"
        }));
    }

    #[test]
    fn detects_modules_and_source_roots_beyond_five_levels() {
        let fixture = Fixture::new("deep-module");
        let module = "products/platform/services/security/components/token";
        fixture.write(
            &format!("{module}/Cargo.toml"),
            "[package]\nname = \"token-service\"\nversion = \"0.1.0\"\n",
        );
        fixture.write(&format!("{module}/src/lib.rs"), "pub fn issue_token() {}\n");

        let inventory = inspect_project(fixture.path()).expect("inventory");

        assert!(inventory
            .modules
            .iter()
            .any(|candidate| candidate.path == module));
        assert!(inventory
            .source_roots
            .iter()
            .any(|candidate| candidate == &format!("{module}/src")));
    }

    #[test]
    fn excludes_binary_media_and_files_above_the_size_cap() {
        let fixture = Fixture::new("binary-large");
        fixture.write("src/valid.rs", "pub const VALID: bool = true;\n");
        fixture.write("assets/logo.png", [0x89, b'P', b'N', b'G', 0, 1, 2]);
        fixture.write("data/binary.dat", [b'a', 0, b'b']);
        fixture.write("generated/oversized.txt", vec![b'x'; 1_048_577]);

        let inventory = inspect_project(fixture.path()).expect("inventory");

        assert!(inventory
            .files
            .iter()
            .any(|file| file.path == "src/valid.rs"));
        for excluded in [
            "assets/logo.png",
            "data/binary.dat",
            "generated/oversized.txt",
        ] {
            assert!(!inventory.files.iter().any(|file| file.path == excluded));
        }
    }

    #[test]
    fn produces_deterministic_sha256_hashes_and_sorted_inventory() {
        let fixture = Fixture::new("hashes");
        fixture.write("z-last.txt", "last\n");
        fixture.write("a-first.txt", "abc");

        let first = inspect_project(fixture.path()).expect("first inventory");
        let second = inspect_project(fixture.path()).expect("second inventory");

        assert_eq!(
            content_sha256(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            first
                .files
                .iter()
                .map(|file| (&file.path, &file.sha256))
                .collect::<Vec<_>>(),
            second
                .files
                .iter()
                .map(|file| (&file.path, &file.sha256))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            first
                .files
                .iter()
                .map(|file| file.path.as_str())
                .collect::<Vec<_>>(),
            vec!["a-first.txt", "z-last.txt"]
        );
        assert_eq!(
            first
                .files
                .iter()
                .find(|file| file.path == "a-first.txt")
                .expect("hashed file")
                .sha256,
            content_sha256(b"abc")
        );
    }
}
