use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
use std::io::{Read, Write};
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

struct SafeRead {
    bytes: Vec<u8>,
    permissions: fs::Permissions,
}

pub fn content_sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn inspect_project(root: &Path) -> Result<ProjectInventory, String> {
    inspect_project_platform(root)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
)))]
fn inspect_project_platform(_root: &Path) -> Result<ProjectInventory, String> {
    Err(
        "Project inventory is unsupported on this platform until handle-safe traversal is available"
            .to_string(),
    )
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn inspect_project_platform(root: &Path) -> Result<ProjectInventory, String> {
    validate_root(root)?;
    let mut paths = collect_files(root)?;
    paths.sort();

    let mut scanned = Vec::new();
    let mut risk_keys = Vec::new();
    for relative_path in paths {
        let relative = relative_path_string(&relative_path)?;
        let path = root.join(&relative_path);
        if excluded_file(&path) {
            continue;
        }
        let Some(safe_read) = read_project_file(root, Path::new(&relative))? else {
            continue;
        };
        let bytes = safe_read.bytes;
        if binary(&bytes) || private_key_content(&bytes) {
            continue;
        }
        let (bytes, keys) = if configuration_file(&path) {
            let Ok(redacted) = redact_configuration(&path, &bytes) else {
                continue;
            };
            redacted
        } else if suspected_credentials(&path, &bytes) {
            continue;
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
        commands: commands(&scanned),
        risk_keys,
    })
}

pub fn create_filtered_workspace(
    root: &Path,
    workspace: &Path,
    inventory: &ProjectInventory,
) -> Result<(), String> {
    create_filtered_workspace_platform(root, workspace, inventory)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
)))]
fn create_filtered_workspace_platform(
    _root: &Path,
    _workspace: &Path,
    _inventory: &ProjectInventory,
) -> Result<(), String> {
    Err(
        "Filtered workspace creation is unsupported on this platform until handle-safe filesystem access is available"
            .to_string(),
    )
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn create_filtered_workspace_platform(
    root: &Path,
    workspace: &Path,
    inventory: &ProjectInventory,
) -> Result<(), String> {
    validate_root(root)?;
    let writer = WorkspaceWriter::create(workspace)?;
    copy_inventory(root, inventory, &writer)?;
    writer.verify_path()
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn copy_inventory(
    root: &Path,
    inventory: &ProjectInventory,
    writer: &WorkspaceWriter,
) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for file in &inventory.files {
        if !seen.insert(file.path.as_str()) {
            return Err(format!("Duplicate inventory path: {}", file.path));
        }
        let relative = inventory_path(&file.path)?;
        let source = root.join(&relative);
        if excluded_file(&source) {
            return Err(format!("Unsafe inventory source: {}", file.path));
        }
        let Some(safe_read) = read_project_file(root, &relative)? else {
            return Err(format!("Unsafe inventory source: {}", file.path));
        };
        let bytes = safe_read.bytes;
        if binary(&bytes)
            || private_key_content(&bytes)
            || (!configuration_file(&source) && suspected_credentials(&source, &bytes))
        {
            return Err(format!("Inventory source became excluded: {}", file.path));
        }
        let safe_bytes = if configuration_file(&source) {
            redact_configuration(&source, &bytes)
                .map_err(|_| format!("Configuration became unsafe: {}", file.path))?
                .0
        } else {
            bytes
        };
        if content_sha256(&safe_bytes) != file.sha256 {
            return Err(format!(
                "Project file changed after inventory: {}",
                file.path
            ));
        }
        writer.write_file(&relative, &safe_bytes, safe_read.permissions)?;
    }
    Ok(())
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn read_bounded(file: &mut fs::File) -> Result<Option<Vec<u8>>, String> {
    let mut bytes = Vec::new();
    file.take(MAX_FILE_SIZE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("Cannot read project file: {error}"))?;
    if bytes.len() as u64 > MAX_FILE_SIZE_BYTES {
        Ok(None)
    } else {
        Ok(Some(bytes))
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn read_project_file(root: &Path, relative: &Path) -> Result<Option<SafeRead>, String> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;

    let root_name = CString::new(root.as_os_str().as_bytes())
        .map_err(|_| format!("Project root contains a NUL byte: {}", root.display()))?;
    let root_fd = unsafe {
        libc::open(
            root_name.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
        )
    };
    if root_fd < 0 {
        return Err(format!(
            "Cannot securely open project root {}: {}",
            root.display(),
            std::io::Error::last_os_error()
        ));
    }
    let mut directory = unsafe { OwnedFd::from_raw_fd(root_fd) };
    let components = relative.components().collect::<Vec<_>>();
    if components.is_empty() {
        return Err("Project file path is empty".to_string());
    }
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(name) = component else {
            return Err(format!("Unsafe project path: {}", relative.display()));
        };
        let name = CString::new(name.as_bytes())
            .map_err(|_| format!("Project path contains a NUL byte: {}", relative.display()))?;
        let final_component = index + 1 == components.len();
        let flags = if final_component {
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_NONBLOCK
        } else {
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_NOFOLLOW | libc::O_DIRECTORY
        };
        let opened = unsafe { libc::openat(directory.as_raw_fd(), name.as_ptr(), flags) };
        if opened < 0 {
            return Err(format!(
                "Cannot securely open {}: {}",
                relative.display(),
                std::io::Error::last_os_error()
            ));
        }
        let opened = unsafe { OwnedFd::from_raw_fd(opened) };
        if final_component {
            let mut file = fs::File::from(opened);
            let metadata = file
                .metadata()
                .map_err(|error| format!("Cannot inspect {}: {error}", relative.display()))?;
            if !metadata.is_file() {
                return Ok(None);
            }
            let permissions = metadata.permissions();
            let Some(bytes) = read_bounded(&mut file)? else {
                return Ok(None);
            };
            return Ok(Some(SafeRead { bytes, permissions }));
        }
        directory = opened;
    }
    Err(format!("Cannot open project file: {}", relative.display()))
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
struct WorkspaceWriter {
    path: PathBuf,
    directory: fs::File,
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
impl WorkspaceWriter {
    fn create(workspace: &Path) -> Result<Self, String> {
        use std::ffi::CString;
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        use std::os::unix::ffi::OsStrExt;

        let path = platform_workspace_path(absolute_path(workspace)?);
        let parent = path
            .parent()
            .ok_or_else(|| format!("Workspace has no parent: {}", path.display()))?;
        let final_name = path
            .file_name()
            .ok_or_else(|| format!("Workspace has no name: {}", path.display()))?;
        let parent_fd = open_or_create_directory_chain(parent)?;
        let final_name = CString::new(final_name.as_bytes())
            .map_err(|_| format!("Workspace name contains a NUL byte: {}", path.display()))?;
        let created = unsafe { libc::mkdirat(parent_fd.as_raw_fd(), final_name.as_ptr(), 0o700) };
        if created != 0 {
            return Err(format!(
                "Cannot securely create workspace {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
        let fd = unsafe {
            libc::openat(
                parent_fd.as_raw_fd(),
                final_name.as_ptr(),
                libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
            )
        };
        if fd < 0 {
            return Err(format!(
                "Cannot securely open workspace {}: {}",
                path.display(),
                std::io::Error::last_os_error()
            ));
        }
        let directory = fs::File::from(unsafe { OwnedFd::from_raw_fd(fd) });
        Ok(Self { path, directory })
    }

    fn write_file(
        &self,
        relative: &Path,
        bytes: &[u8],
        permissions: fs::Permissions,
    ) -> Result<(), String> {
        use std::ffi::CString;
        use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
        use std::os::unix::ffi::OsStrExt;

        let components = relative.components().collect::<Vec<_>>();
        if components.is_empty() {
            return Err("Workspace file path is empty".to_string());
        }
        let mut owned_directory: Option<OwnedFd> = None;
        for (index, component) in components.iter().enumerate() {
            let Component::Normal(name) = component else {
                return Err(format!(
                    "Unsafe workspace file path: {}",
                    relative.display()
                ));
            };
            let name = CString::new(name.as_bytes()).map_err(|_| {
                format!(
                    "Workspace file path contains a NUL byte: {}",
                    relative.display()
                )
            })?;
            let parent_fd = owned_directory
                .as_ref()
                .map(AsRawFd::as_raw_fd)
                .unwrap_or_else(|| self.directory.as_raw_fd());
            let final_component = index + 1 == components.len();
            if final_component {
                let fd = unsafe {
                    libc::openat(
                        parent_fd,
                        name.as_ptr(),
                        libc::O_WRONLY
                            | libc::O_CREAT
                            | libc::O_EXCL
                            | libc::O_CLOEXEC
                            | libc::O_NOFOLLOW
                            | libc::O_NONBLOCK,
                        0o600,
                    )
                };
                if fd < 0 {
                    return Err(format!(
                        "Cannot securely create workspace file {}: {}",
                        relative.display(),
                        std::io::Error::last_os_error()
                    ));
                }
                let mut file = fs::File::from(unsafe { OwnedFd::from_raw_fd(fd) });
                if !file
                    .metadata()
                    .map_err(|error| error.to_string())?
                    .is_file()
                {
                    return Err(format!(
                        "Workspace destination is not a file: {}",
                        relative.display()
                    ));
                }
                file.write_all(bytes)
                    .map_err(|error| format!("Cannot write {}: {error}", relative.display()))?;
                file.set_permissions(permissions).map_err(|error| {
                    format!(
                        "Cannot preserve permissions for {}: {error}",
                        relative.display()
                    )
                })?;
                return Ok(());
            }

            let mut fd = unsafe {
                libc::openat(
                    parent_fd,
                    name.as_ptr(),
                    libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
                )
            };
            if fd < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound {
                let created = unsafe { libc::mkdirat(parent_fd, name.as_ptr(), 0o700) };
                if created != 0
                    && std::io::Error::last_os_error().kind() != std::io::ErrorKind::AlreadyExists
                {
                    return Err(format!(
                        "Cannot create workspace directory {}: {}",
                        relative.display(),
                        std::io::Error::last_os_error()
                    ));
                }
                fd = unsafe {
                    libc::openat(
                        parent_fd,
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
                    )
                };
            }
            if fd < 0 {
                return Err(format!(
                    "Cannot securely open workspace directory {}: {}",
                    relative.display(),
                    std::io::Error::last_os_error()
                ));
            }
            owned_directory = Some(unsafe { OwnedFd::from_raw_fd(fd) });
        }
        Err(format!(
            "Cannot write workspace file: {}",
            relative.display()
        ))
    }

    fn verify_path(&self) -> Result<(), String> {
        use std::os::unix::fs::MetadataExt;

        let path_metadata = fs::symlink_metadata(&self.path)
            .map_err(|error| format!("Workspace path changed: {error}"))?;
        let handle_metadata = self
            .directory
            .metadata()
            .map_err(|error| format!("Cannot inspect workspace handle: {error}"))?;
        if path_metadata.file_type().is_symlink()
            || !path_metadata.is_dir()
            || path_metadata.dev() != handle_metadata.dev()
            || path_metadata.ino() != handle_metadata.ino()
        {
            return Err(format!(
                "Workspace path was replaced while copying: {}",
                self.path.display()
            ));
        }
        Ok(())
    }
}

#[cfg(target_vendor = "apple")]
fn platform_workspace_path(path: PathBuf) -> PathBuf {
    for (alias, physical) in [
        (Path::new("/var"), Path::new("/private/var")),
        (Path::new("/tmp"), Path::new("/private/tmp")),
        (Path::new("/etc"), Path::new("/private/etc")),
    ] {
        if let Ok(suffix) = path.strip_prefix(alias) {
            return physical.join(suffix);
        }
    }
    path
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn platform_workspace_path(path: PathBuf) -> PathBuf {
    path
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn open_or_create_directory_chain(path: &Path) -> Result<std::os::fd::OwnedFd, String> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;

    let root = CString::new("/").expect("static root path");
    let fd = unsafe {
        libc::open(
            root.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
        )
    };
    if fd < 0 {
        return Err(format!(
            "Cannot securely open filesystem root: {}",
            std::io::Error::last_os_error()
        ));
    }
    let mut directory = unsafe { OwnedFd::from_raw_fd(fd) };
    for component in path.components() {
        match component {
            Component::RootDir => continue,
            Component::Normal(name) => {
                let name = CString::new(name.as_bytes()).map_err(|_| {
                    format!("Workspace parent contains a NUL byte: {}", path.display())
                })?;
                let mut opened = unsafe {
                    libc::openat(
                        directory.as_raw_fd(),
                        name.as_ptr(),
                        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
                    )
                };
                if opened < 0
                    && std::io::Error::last_os_error().kind() == std::io::ErrorKind::NotFound
                {
                    let created =
                        unsafe { libc::mkdirat(directory.as_raw_fd(), name.as_ptr(), 0o700) };
                    if created != 0
                        && std::io::Error::last_os_error().kind()
                            != std::io::ErrorKind::AlreadyExists
                    {
                        return Err(format!(
                            "Cannot securely create workspace parent {}: {}",
                            path.display(),
                            std::io::Error::last_os_error()
                        ));
                    }
                    opened = unsafe {
                        libc::openat(
                            directory.as_raw_fd(),
                            name.as_ptr(),
                            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
                        )
                    };
                }
                if opened < 0 {
                    return Err(format!(
                        "Cannot securely open workspace parent {}: {}",
                        path.display(),
                        std::io::Error::last_os_error()
                    ));
                }
                directory = unsafe { OwnedFd::from_raw_fd(opened) };
            }
            _ => return Err(format!("Unsafe workspace parent: {}", path.display())),
        }
    }
    Ok(directory)
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn absolute_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        std::env::current_dir()
            .map(|current| current.join(path))
            .map_err(|error| format!("Cannot resolve workspace path: {error}"))
    }
}

fn validate_root(root: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(root)
        .map_err(|error| format!("Cannot inspect root {}: {error}", root.display()))?;
    if metadata_is_link_or_reparse(&metadata) || !metadata.is_dir() {
        return Err(format!(
            "Project root is not a real directory: {}",
            root.display()
        ));
    }
    Ok(())
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn collect_files(root: &Path) -> Result<Vec<PathBuf>, String> {
    use std::ffi::CString;
    use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
    use std::os::unix::ffi::OsStrExt;

    let root_name = CString::new(root.as_os_str().as_bytes())
        .map_err(|_| format!("Project root contains a NUL byte: {}", root.display()))?;
    let root_fd = unsafe {
        libc::open(
            root_name.as_ptr(),
            libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
        )
    };
    if root_fd < 0 {
        return Err(format!(
            "Cannot securely open project root {}: {}",
            root.display(),
            std::io::Error::last_os_error()
        ));
    }

    let mut result = Vec::new();
    let mut pending = vec![(
        unsafe { OwnedFd::from_raw_fd(root_fd) },
        PathBuf::new(),
        0usize,
    )];
    while let Some((directory, relative_directory, depth)) = pending.pop() {
        if depth > MAX_SCAN_DEPTH {
            continue;
        }
        for name in directory_entry_names(directory.as_raw_fd(), &relative_directory)? {
            let c_name = CString::new(name.as_bytes()).map_err(|_| {
                format!(
                    "Project path contains a NUL byte: {}",
                    relative_directory.join(&name).display()
                )
            })?;
            let mut metadata = std::mem::MaybeUninit::<libc::stat>::uninit();
            let inspected = unsafe {
                libc::fstatat(
                    directory.as_raw_fd(),
                    c_name.as_ptr(),
                    metadata.as_mut_ptr(),
                    libc::AT_SYMLINK_NOFOLLOW,
                )
            };
            if inspected != 0 {
                return Err(format!(
                    "Cannot securely inspect {}: {}",
                    relative_directory.join(&name).display(),
                    std::io::Error::last_os_error()
                ));
            }
            let metadata = unsafe { metadata.assume_init() };
            let file_type = metadata.st_mode & libc::S_IFMT;
            let relative = relative_directory.join(&name);
            if file_type == libc::S_IFDIR {
                if excluded_directory(&relative) {
                    continue;
                }
                let fd = unsafe {
                    libc::openat(
                        directory.as_raw_fd(),
                        c_name.as_ptr(),
                        libc::O_RDONLY | libc::O_CLOEXEC | libc::O_DIRECTORY | libc::O_NOFOLLOW,
                    )
                };
                if fd < 0 {
                    return Err(format!(
                        "Cannot securely open project directory {}: {}",
                        relative.display(),
                        std::io::Error::last_os_error()
                    ));
                }
                pending.push((unsafe { OwnedFd::from_raw_fd(fd) }, relative, depth + 1));
            } else if file_type == libc::S_IFREG {
                result.push(relative);
            }
        }
    }
    Ok(result)
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
struct DirectoryStream(*mut libc::DIR);

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
impl Drop for DirectoryStream {
    fn drop(&mut self) {
        unsafe {
            libc::closedir(self.0);
        }
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn directory_entry_names(
    directory_fd: std::os::fd::RawFd,
    relative: &Path,
) -> Result<Vec<std::ffi::OsString>, String> {
    use std::ffi::CStr;
    use std::os::unix::ffi::{OsStrExt, OsStringExt};

    let duplicate = unsafe { libc::fcntl(directory_fd, libc::F_DUPFD_CLOEXEC, 0) };
    if duplicate < 0 {
        return Err(format!(
            "Cannot duplicate project directory handle {}: {}",
            relative.display(),
            std::io::Error::last_os_error()
        ));
    }
    let stream = unsafe { libc::fdopendir(duplicate) };
    if stream.is_null() {
        unsafe {
            libc::close(duplicate);
        }
        return Err(format!(
            "Cannot enumerate project directory {}: {}",
            relative.display(),
            std::io::Error::last_os_error()
        ));
    }
    let stream = DirectoryStream(stream);
    let mut names = Vec::new();
    loop {
        set_errno(0);
        let entry = unsafe { libc::readdir(stream.0) };
        if entry.is_null() {
            let errno = current_errno();
            if errno != 0 {
                return Err(format!(
                    "Cannot enumerate project directory {}: {}",
                    relative.display(),
                    std::io::Error::from_raw_os_error(errno)
                ));
            }
            break;
        }
        let name = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) }.to_bytes();
        if matches!(name, b"." | b"..") {
            continue;
        }
        names.push(std::ffi::OsString::from_vec(name.to_vec()));
    }
    names.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    Ok(names)
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn errno_pointer() -> *mut libc::c_int {
    unsafe { libc::__errno_location() }
}

#[cfg(any(
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn errno_pointer() -> *mut libc::c_int {
    unsafe { libc::__error() }
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn set_errno(value: libc::c_int) {
    unsafe {
        *errno_pointer() = value;
    }
}

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd",
    target_os = "dragonfly"
))]
fn current_errno() -> libc::c_int {
    unsafe { *errno_pointer() }
}

fn excluded_directory(path: &Path) -> bool {
    let name = filename(path);
    matches!(
        name.as_str(),
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
            | ".pytest_cache"
            | ".mypy_cache"
            | ".tox"
            | ".turbo"
            | ".parcel-cache"
            | ".pnpm-store"
            | ".output"
            | ".venv"
            | "venv"
            | ".ruff_cache"
            | ".nox"
            | ".hypothesis"
            | ".vite"
    ) || path_ends_with(path, &[".yarn", "unplugged"])
        || path_ends_with(path, &[".angular", "cache"])
}

fn path_ends_with(path: &Path, suffix: &[&str]) -> bool {
    let components = path
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect::<Vec<_>>();
    components.len() >= suffix.len()
        && components[components.len() - suffix.len()..]
            .iter()
            .zip(suffix)
            .all(|(left, right)| left.eq_ignore_ascii_case(right))
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
            | ".git-credentials"
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
            | "ppk"
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
    text.lines().any(|line| {
        let line = line.trim();
        line.starts_with("-----BEGIN ")
            && line.ends_with(" PRIVATE KEY-----")
            && line.len() >= "-----BEGIN PRIVATE KEY-----".len()
    }) || text
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("PuTTY-User-Key-File-"))
}

fn configuration_file(path: &Path) -> bool {
    configuration_format(path).is_some()
}

#[derive(Clone, Copy)]
enum ConfigurationFormat {
    Json,
    Toml,
    Yaml,
    Xml,
    Assignment,
}

fn configuration_format(path: &Path) -> Option<ConfigurationFormat> {
    let name = filename(path);
    if path_ends_with(path, &[".kube", "config"]) {
        return Some(ConfigurationFormat::Yaml);
    }
    if name == "settings.py" {
        return Some(ConfigurationFormat::Assignment);
    }
    if matches!(
        name.as_str(),
        "nuget.config" | "web.config" | "app.config" | "packages.config"
    ) {
        return Some(ConfigurationFormat::Xml);
    }
    match extension(path).as_str() {
        "json" => Some(ConfigurationFormat::Json),
        "toml" => Some(ConfigurationFormat::Toml),
        "yaml" | "yml" => Some(ConfigurationFormat::Yaml),
        "properties" | "ini" | "conf" | "config" => Some(ConfigurationFormat::Assignment),
        _ if name == ".env"
            || name.starts_with(".env.")
            || matches!(name.as_str(), ".npmrc" | ".pypirc" | ".yarnrc") =>
        {
            Some(ConfigurationFormat::Assignment)
        }
        _ => None,
    }
}

fn redact_configuration(path: &Path, bytes: &[u8]) -> Result<(Vec<u8>, Vec<String>), String> {
    let format = configuration_format(path)
        .ok_or_else(|| format!("Unsupported configuration format: {}", path.display()))?;
    let container_policy = CredentialContainerPolicy::for_path(path);
    let mut keys = Vec::new();
    let output = match format {
        ConfigurationFormat::Json => {
            let mut value: serde_json::Value = serde_json::from_slice(bytes)
                .map_err(|_| format!("Invalid JSON configuration: {}", path.display()))?;
            redact_json_value(&mut value, None, false, container_policy, &mut keys);
            serde_json::to_vec_pretty(&value)
                .map_err(|_| format!("Cannot serialize JSON configuration: {}", path.display()))?
        }
        ConfigurationFormat::Toml => {
            let text = std::str::from_utf8(bytes)
                .map_err(|_| format!("Invalid UTF-8 TOML configuration: {}", path.display()))?;
            let mut value: toml::Value = toml::from_str(text)
                .map_err(|_| format!("Invalid TOML configuration: {}", path.display()))?;
            redact_toml_value(&mut value, None, false, container_policy, &mut keys);
            toml::to_string_pretty(&value)
                .map(String::into_bytes)
                .map_err(|_| format!("Cannot serialize TOML configuration: {}", path.display()))?
        }
        ConfigurationFormat::Yaml => {
            let mut value: serde_yaml::Value = serde_yaml::from_slice(bytes)
                .map_err(|_| format!("Invalid YAML configuration: {}", path.display()))?;
            redact_yaml_value(&mut value, None, false, container_policy, &mut keys);
            serde_yaml::to_string(&value)
                .map(String::into_bytes)
                .map_err(|_| format!("Cannot serialize YAML configuration: {}", path.display()))?
        }
        ConfigurationFormat::Xml => redact_xml(bytes, &mut keys)
            .map_err(|error| format!("Invalid XML configuration {}: {error}", path.display()))?,
        ConfigurationFormat::Assignment => redact_assignments(bytes, &mut keys)?,
    };
    Ok((output, keys))
}

fn redact_xml(bytes: &[u8], keys: &mut Vec<String>) -> Result<Vec<u8>, String> {
    use quick_xml::events::{BytesStart, Event};
    use quick_xml::{Decoder, Reader, Writer, XmlVersion};

    fn redact_start(
        mut start: BytesStart<'_>,
        decoder: Decoder,
        keys: &mut Vec<String>,
    ) -> Result<BytesStart<'static>, String> {
        let mut attributes = Vec::new();
        for attribute in start.attributes() {
            let attribute = attribute.map_err(|error| error.to_string())?;
            let name = std::str::from_utf8(attribute.key.as_ref())
                .map_err(|_| "XML attribute name is not UTF-8".to_string())?
                .to_string();
            let value = attribute
                .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
                .map_err(|error| error.to_string())?
                .into_owned();
            attributes.push((name, value));
        }
        let logical_key = attributes
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case("key") || name.eq_ignore_ascii_case("name"))
            .map(|(_, value)| value.clone());
        for (name, value) in &mut attributes {
            let logical_value = name.eq_ignore_ascii_case("value")
                && logical_key.as_deref().is_some_and(sensitive_key);
            if sensitive_key(name) || logical_value || connection_string(value) {
                keys.push(logical_key.clone().unwrap_or_else(|| name.clone()));
                *value = REDACTED.to_string();
            }
        }
        start.clear_attributes();
        for (name, value) in &attributes {
            start.push_attribute((name.as_str(), value.as_str()));
        }
        Ok(start.into_owned())
    }

    let mut reader = Reader::from_reader(bytes);
    let decoder = reader.decoder();
    let mut output = Vec::with_capacity(bytes.len());
    let mut writer = Writer::new(&mut output);
    loop {
        let event = reader.read_event().map_err(|error| error.to_string())?;
        match event {
            Event::Start(start) => writer
                .write_event(Event::Start(redact_start(start, decoder, keys)?))
                .map_err(|error| error.to_string())?,
            Event::Empty(start) => writer
                .write_event(Event::Empty(redact_start(start, decoder, keys)?))
                .map_err(|error| error.to_string())?,
            Event::Eof => break,
            event => writer
                .write_event(event.into_owned())
                .map_err(|error| error.to_string())?,
        }
    }
    Ok(output)
}

fn redact_json_value(
    value: &mut serde_json::Value,
    context: Option<&str>,
    protected: bool,
    container_policy: CredentialContainerPolicy,
    keys: &mut Vec<String>,
) {
    if protected && !value.is_object() && !value.is_array() {
        *value = serde_json::Value::String(REDACTED.to_string());
        keys.push(context.unwrap_or("credential-container").to_string());
        return;
    }
    match value {
        serde_json::Value::Object(object) => {
            for (key, value) in object {
                let composite = value.is_object() || value.is_array();
                let protected = protected
                    || container_policy.redacts(key)
                    || (!composite && sensitive_key(key));
                redact_json_value(value, Some(key), protected, container_policy, keys);
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                redact_json_value(value, context, protected, container_policy, keys);
            }
        }
        serde_json::Value::String(text) if connection_string(text) => {
            *text = REDACTED.to_string();
            keys.push(context.unwrap_or("connection-string").to_string());
        }
        _ => {}
    }
}

fn redact_toml_value(
    value: &mut toml::Value,
    context: Option<&str>,
    protected: bool,
    container_policy: CredentialContainerPolicy,
    keys: &mut Vec<String>,
) {
    if protected && !value.is_table() && !value.is_array() {
        *value = toml::Value::String(REDACTED.to_string());
        keys.push(context.unwrap_or("credential-container").to_string());
        return;
    }
    match value {
        toml::Value::Table(table) => {
            for (key, value) in table {
                let composite = value.is_table() || value.is_array();
                let protected = protected
                    || container_policy.redacts(key)
                    || (!composite && sensitive_key(key));
                redact_toml_value(value, Some(key), protected, container_policy, keys);
            }
        }
        toml::Value::Array(values) => {
            for value in values {
                redact_toml_value(value, context, protected, container_policy, keys);
            }
        }
        toml::Value::String(text) if connection_string(text) => {
            *text = REDACTED.to_string();
            keys.push(context.unwrap_or("connection-string").to_string());
        }
        _ => {}
    }
}

fn redact_yaml_value(
    value: &mut serde_yaml::Value,
    context: Option<&str>,
    protected: bool,
    container_policy: CredentialContainerPolicy,
    keys: &mut Vec<String>,
) {
    if protected
        && !matches!(
            value,
            serde_yaml::Value::Mapping(_)
                | serde_yaml::Value::Sequence(_)
                | serde_yaml::Value::Tagged(_)
        )
    {
        *value = serde_yaml::Value::String(REDACTED.to_string());
        keys.push(context.unwrap_or("credential-container").to_string());
        return;
    }
    match value {
        serde_yaml::Value::Mapping(mapping) => {
            for (key, value) in mapping {
                let key_text = key.as_str();
                let composite = matches!(
                    value,
                    serde_yaml::Value::Mapping(_) | serde_yaml::Value::Sequence(_)
                );
                let protected = protected
                    || key_text.is_some_and(|key| container_policy.redacts(key))
                    || (!composite && key_text.is_some_and(sensitive_key));
                redact_yaml_value(
                    value,
                    key_text.or(context),
                    protected,
                    container_policy,
                    keys,
                );
            }
        }
        serde_yaml::Value::Sequence(values) => {
            for value in values {
                redact_yaml_value(value, context, protected, container_policy, keys);
            }
        }
        serde_yaml::Value::String(text) if connection_string(text) => {
            *text = REDACTED.to_string();
            keys.push(context.unwrap_or("connection-string").to_string());
        }
        serde_yaml::Value::Tagged(tagged) => {
            redact_yaml_value(
                &mut tagged.value,
                context,
                protected,
                container_policy,
                keys,
            );
        }
        _ => {}
    }
}

fn redact_assignments(bytes: &[u8], keys: &mut Vec<String>) -> Result<Vec<u8>, String> {
    let text = std::str::from_utf8(bytes)
        .map_err(|_| "Assignment configuration is not valid UTF-8".to_string())?;
    let mut result = String::with_capacity(text.len());
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
    Ok(result.into_bytes())
}

fn redact_line(line: &str) -> Option<(String, String)> {
    let trimmed = line.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
        return None;
    }
    if connection_string(trimmed) {
        let indent = &line[..line.len() - trimmed.len()];
        return Some((
            format!("{indent}{REDACTED}"),
            "connection-string".to_string(),
        ));
    }
    let delimiter = match line.find('=') {
        Some(index) => index,
        None => line.find(':')?,
    };
    let raw_key = line[..delimiter].trim();
    let key = raw_key
        .trim_matches(['"', '\''])
        .rsplit(':')
        .next()
        .unwrap_or(raw_key)
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
    let key = normalized_key(key);
    matches!(key.as_str(), "auth" | "authorization" | "pwd")
        || [
            "password",
            "passwd",
            "secret",
            "token",
            "apikey",
            "privatekey",
            "credential",
            "credentials",
            "accesskey",
            "secretkey",
            "secretkeybase",
            "signingkey",
            "encryptionkey",
            "accountkey",
            "masterkey",
            "sessionkey",
            "clientkey",
            "clientkeydata",
            "tokenfile",
            "connectionstring",
            "databaseurl",
        ]
        .iter()
        .any(|candidate| key.ends_with(candidate))
}

#[derive(Clone, Copy)]
enum CredentialContainerPolicy {
    None,
    ComposerAuth,
    DockerAuth,
}

impl CredentialContainerPolicy {
    fn for_path(path: &Path) -> Self {
        let name = filename(path);
        if name == "auth.json" {
            Self::ComposerAuth
        } else if name == "config.json"
            && path
                .parent()
                .and_then(Path::file_name)
                .and_then(|value| value.to_str())
                .is_some_and(|parent| matches!(parent, "docker" | ".docker"))
        {
            Self::DockerAuth
        } else {
            Self::None
        }
    }

    fn redacts(self, key: &str) -> bool {
        let key = normalized_key(key);
        match self {
            Self::None => false,
            Self::ComposerAuth => matches!(
                key.as_str(),
                "auth"
                    | "auths"
                    | "credential"
                    | "credentials"
                    | "githuboauth"
                    | "gitlaboauth"
                    | "bitbucketoauth"
                    | "httpbasic"
                    | "httpbearer"
                    | "bearer"
                    | "gitlabtoken"
            ),
            Self::DockerAuth => key == "auths",
        }
    }
}

fn credential_container(key: &str) -> bool {
    let key = normalized_key(key);
    matches!(
        key.as_str(),
        "auth"
            | "auths"
            | "authentication"
            | "credential"
            | "credentials"
            | "githuboauth"
            | "gitlaboauth"
            | "bitbucketoauth"
            | "httpbasic"
            | "httpbearer"
            | "oauth"
            | "oauth2"
            | "secrets"
    ) || key.ends_with("oauth")
        || key.ends_with("credentials")
}

fn normalized_key(key: &str) -> String {
    key.chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
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

fn suspected_credentials(path: &Path, bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return true;
    };
    if bearer_credential(text) || xml_like_credentials(text) {
        return true;
    }
    let source = source_file(path);
    text.lines().any(|line| {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with(';') {
            return false;
        }
        if connection_string(trimmed) {
            return true;
        }
        line.char_indices()
            .filter(|(_, character)| matches!(character, '=' | ':'))
            .any(|(delimiter, _)| {
                let before = line[..delimiter].trim_end();
                let key_start = before
                    .char_indices()
                    .rev()
                    .find(|(_, character)| {
                        !character.is_ascii_alphanumeric()
                            && !matches!(character, '_' | '-' | '.' | '"' | '\'')
                    })
                    .map_or(0, |(index, character)| index + character.len_utf8());
                let key = before[key_start..]
                    .trim_matches(['"', '\''])
                    .rsplit('.')
                    .next()
                    .unwrap_or("");
                let value = line[delimiter + 1..].trim_start();
                !value.is_empty()
                    && (connection_string(value)
                        || (sensitive_key(key) && credential_assignment_value(value, source)))
            })
    })
}

fn bearer_credential(text: &str) -> bool {
    let lower = text.to_ascii_lowercase();
    let mut offset = 0;
    while let Some(found) = lower[offset..].find("bearer") {
        let start = offset + found;
        let before = text[..start].chars().next_back();
        let after_word = start + "bearer".len();
        let after = text[after_word..].chars().next();
        let bounded = before.is_none_or(|value| !value.is_ascii_alphanumeric())
            && after.is_none_or(|value| !value.is_ascii_alphanumeric());
        if bounded {
            let token = text[after_word..]
                .trim_start_matches(|value: char| {
                    value.is_ascii_whitespace() || matches!(value, ':' | '=' | '"' | '\'')
                })
                .split(|value: char| {
                    value.is_ascii_whitespace() || matches!(value, '"' | '\'' | '<' | '>')
                })
                .next()
                .unwrap_or("")
                .trim_end_matches(|value: char| matches!(value, ',' | ';' | '.'));
            if high_confidence_secret(token) {
                return true;
            }
        }
        offset = after_word;
    }
    false
}

fn credential_assignment_value(value: &str, source: bool) -> bool {
    if connection_string(value) || bearer_credential(value) {
        return true;
    }
    if source {
        direct_string_literal(value).is_some_and(hardcoded_credential_literal)
    } else {
        let value = value
            .split(|character: char| character.is_ascii_whitespace() || character == ';')
            .next()
            .unwrap_or("")
            .trim_matches(['"', '\'', '`']);
        !value.is_empty()
            && !matches!(
                value.to_ascii_lowercase().as_str(),
                "string"
                    | "str"
                    | "token"
                    | "secret"
                    | "password"
                    | "redacted"
                    | "example"
                    | "placeholder"
            )
    }
}

fn hardcoded_credential_literal(value: &str) -> bool {
    !value.trim().is_empty()
        && !matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "token" | "secret" | "password" | "redacted" | "example" | "placeholder"
        )
}

fn direct_string_literal(value: &str) -> Option<&str> {
    let value = value.trim_start();
    let (quote, remainder) = match value.chars().next()? {
        quote @ ('"' | '\'' | '`') => (quote, &value[quote.len_utf8()..]),
        _ => return None,
    };
    let end = remainder.find(quote)?;
    Some(&remainder[..end])
}

fn high_confidence_secret(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty()
        || matches!(
            value.to_ascii_lowercase().as_str(),
            "token"
                | "secret"
                | "password"
                | "redacted"
                | "example"
                | "placeholder"
                | "authentication"
        )
    {
        return false;
    }
    value.starts_with("eyJ") && value.matches('.').count() >= 2
        || ["ghp_", "github_pat_", "sk-", "xox", "akia", "ya29."]
            .iter()
            .any(|prefix| value.to_ascii_lowercase().starts_with(prefix))
        || value.len() >= 16
}

fn xml_like_credentials(text: &str) -> bool {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    if !text.trim_start().starts_with('<') {
        return false;
    }
    let mut reader = Reader::from_str(text);
    let decoder = reader.decoder();
    let mut sensitive_elements = Vec::new();
    loop {
        let event = match reader.read_event() {
            Ok(event) => event,
            Err(_) => return credential_vocabulary(text),
        };
        match event {
            Event::Start(start) => {
                let name = std::str::from_utf8(start.local_name().as_ref())
                    .map(str::to_string)
                    .unwrap_or_default();
                match xml_attributes_contain_credentials(&start, decoder) {
                    Ok(true) => return true,
                    Ok(false) => sensitive_elements.push(sensitive_key(&name)),
                    Err(_) => return credential_vocabulary(text),
                }
            }
            Event::Empty(start) => match xml_attributes_contain_credentials(&start, decoder) {
                Ok(true) => return true,
                Ok(false) => {}
                Err(_) => return credential_vocabulary(text),
            },
            Event::Text(value)
                if sensitive_elements.last() == Some(&true)
                    && (value.as_ref() as &[u8])
                        .iter()
                        .any(|byte| !byte.is_ascii_whitespace()) =>
            {
                return true;
            }
            Event::End(_) => {
                sensitive_elements.pop();
            }
            Event::Eof => {
                return if sensitive_elements.is_empty() {
                    false
                } else {
                    credential_vocabulary(text)
                };
            }
            _ => {}
        }
    }
}

fn xml_attributes_contain_credentials(
    start: &quick_xml::events::BytesStart<'_>,
    decoder: quick_xml::Decoder,
) -> Result<bool, String> {
    use quick_xml::XmlVersion;

    let mut attributes = Vec::new();
    for attribute in start.attributes() {
        let attribute = attribute.map_err(|error| error.to_string())?;
        let name = std::str::from_utf8(attribute.key.as_ref())
            .map_err(|_| "XML attribute name is not UTF-8".to_string())?
            .to_string();
        let value = attribute
            .decoded_and_normalized_value(XmlVersion::Implicit1_0, decoder)
            .map_err(|error| error.to_string())?
            .into_owned();
        attributes.push((name, value));
    }
    let logical_key = attributes
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case("key") || name.eq_ignore_ascii_case("name"))
        .map(|(_, value)| value.as_str());
    Ok(attributes.iter().any(|(name, value)| {
        !value.trim().is_empty()
            && (sensitive_key(name)
                || (name.eq_ignore_ascii_case("value") && logical_key.is_some_and(sensitive_key)))
    }))
}

fn credential_vocabulary(text: &str) -> bool {
    text.split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .any(|word| sensitive_key(word) || credential_container(word))
}

fn source_file(path: &Path) -> bool {
    matches!(
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
    )
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

fn commands(files: &[ScannedFile]) -> Vec<ProjectCommand> {
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
                    let manager = package_manager(files, &cwd);
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

fn package_manager(files: &[ScannedFile], module: &str) -> &'static str {
    let mut directory = module.to_string();
    loop {
        let lockfile = if directory == "." {
            "pnpm-lock.yaml".to_string()
        } else {
            format!("{directory}/pnpm-lock.yaml")
        };
        if files.iter().any(|file| file.path == lockfile) {
            return "pnpm";
        }
        let lockfile = if directory == "." {
            "yarn.lock".to_string()
        } else {
            format!("{directory}/yarn.lock")
        };
        if files.iter().any(|file| file.path == lockfile) {
            return "yarn";
        }
        if directory == "." {
            return "npm";
        }
        directory = parent(&format!("{directory}/package.json"));
        directory = parent(&directory);
    }
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

fn relative_path_string(path: &Path) -> Result<String, String> {
    path.components()
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

fn metadata_is_link_or_reparse(metadata: &fs::Metadata) -> bool {
    if metadata.file_type().is_symlink() {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        const FILE_ATTRIBUTE_REPARSE_POINT: u32 = 0x0000_0400;
        return metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT != 0;
    }
    #[cfg(not(windows))]
    {
        false
    }
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

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_vendor = "apple",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd",
        target_os = "dragonfly"
    ))]
    mod handle_safe_target {
        use super::*;

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
            assert!(
                create_filtered_workspace(fixture.path(), &escaped_workspace, &inventory).is_err()
            );
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
            assert!(copied.contains("password:"), "{copied}");
            assert!(copied.contains("api-token:"), "{copied}");
            assert!(copied.matches(REDACTED).count() >= 2, "{copied}");
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
        fn redacts_structured_compact_and_block_configuration_without_breaking_json() {
            let fixture = Fixture::new("structured-redaction");
            fixture.write(
            "docker/config.json",
            r#"{"auths":{"registry.example.com":{"auth":"docker-secret","identitytoken":"identity-secret"}},"debug":true}"#,
        );
            fixture.write(
                "config/secrets.toml",
                concat!(
                    "[database]\n",
                    "connection = { username = \"iam-app\", password = \"toml-secret\" }\n",
                ),
            );
            fixture.write(
                "config/secrets.yml",
                concat!(
                    "service:\n",
                    "  username: iam-app\n",
                    "  password: |\n",
                    "    yaml-secret-line-one\n",
                    "    yaml-secret-line-two\n",
                ),
            );
            fixture.write(
                "package.json",
                r#"{"dependencies":{"tokenizers":"1.2.3"},"scripts":{"test":"vitest"}}"#,
            );

            let inventory = inspect_project(fixture.path()).expect("inventory");
            let workspace_parent = Fixture::new("structured-redaction-workspace");
            let workspace = workspace_parent.path().join("workspace");
            create_filtered_workspace(fixture.path(), &workspace, &inventory).expect("workspace");

            let docker =
                fs::read_to_string(workspace.join("docker/config.json")).expect("docker config");
            let docker_json: serde_json::Value =
                serde_json::from_str(&docker).expect("redacted Docker config remains valid JSON");
            assert_eq!(
                docker_json["auths"]["registry.example.com"]["auth"],
                REDACTED
            );
            assert_eq!(
                docker_json["auths"]["registry.example.com"]["identitytoken"],
                REDACTED
            );
            assert_eq!(docker_json["debug"], true);
            assert!(!docker.contains("docker-secret"));
            assert!(!docker.contains("identity-secret"));

            let toml =
                fs::read_to_string(workspace.join("config/secrets.toml")).expect("redacted TOML");
            assert!(toml.contains("username = \"iam-app\""));
            assert!(toml.contains("password = \"[REDACTED]\""));
            assert!(!toml.contains("toml-secret"));

            let yaml =
                fs::read_to_string(workspace.join("config/secrets.yml")).expect("redacted YAML");
            assert!(yaml.contains("username: iam-app"));
            assert!(yaml.contains(REDACTED));
            assert!(!yaml.contains("yaml-secret-line-one"));
            assert!(!yaml.contains("yaml-secret-line-two"));

            let package: serde_json::Value = serde_json::from_slice(
                &fs::read(workspace.join("package.json")).expect("redacted package manifest"),
            )
            .expect("package manifest remains valid JSON");
            assert_eq!(package["dependencies"]["tokenizers"], "1.2.3");
        }

        #[test]
        fn redacts_framework_operational_and_kubernetes_secret_keys() {
            let fixture = Fixture::new("framework-secrets");
            fixture.write(
                "config/secrets.yml",
                concat!(
                    "SECRET_KEY: django-secret\n",
                    "secretKeyBase: rails-secret\n",
                    "signingKey: signing-secret\n",
                    "encryption_key: encryption-secret\n",
                    "accountKey: account-secret\n",
                    "master-key: master-secret\n",
                    "sessionKey: session-secret\n",
                    "publicName: safe-name\n",
                ),
            );
            fixture.write(
                ".kube/config",
                concat!(
                    "apiVersion: v1\n",
                    "users:\n",
                    "  - name: developer\n",
                    "    user:\n",
                    "      token: kube-secret\n",
                    "      client-key-data: kube-client-key-secret\n",
                    "      clientKeyData: kube-camel-client-key-secret\n",
                ),
            );
            fixture.write(
                "app/settings.py",
                "SECRET_KEY = 'django-settings-secret'\nDEBUG = True\n",
            );

            let inventory = inspect_project(fixture.path()).expect("inventory");
            let workspace_parent = Fixture::new("framework-secrets-workspace");
            let workspace = workspace_parent.path().join("workspace");
            create_filtered_workspace(fixture.path(), &workspace, &inventory).expect("workspace");

            let yaml =
                fs::read_to_string(workspace.join("config/secrets.yml")).expect("framework YAML");
            let kube =
                fs::read_to_string(workspace.join(".kube/config")).expect("Kubernetes config");
            let settings =
                fs::read_to_string(workspace.join("app/settings.py")).expect("Django settings");
            for secret in [
                "django-secret",
                "rails-secret",
                "signing-secret",
                "encryption-secret",
                "account-secret",
                "master-secret",
                "session-secret",
                "kube-secret",
                "kube-client-key-secret",
                "kube-camel-client-key-secret",
                "django-settings-secret",
            ] {
                assert!(!yaml.contains(secret));
                assert!(!kube.contains(secret));
                assert!(!settings.contains(secret));
            }
            assert!(yaml.contains("publicName: safe-name"));
            assert!(settings.contains("DEBUG = True"));
        }

        #[test]
        fn redacts_every_scalar_below_credential_containers_in_composer_auth() {
            let fixture = Fixture::new("composer-auth");
            fixture.write(
                ".composer/auth.json",
                r#"{
  "github-oauth": {"github.com": "github-secret"},
  "http-basic": {
    "repo.example.com": {
      "username": "composer-user",
      "password": "composer-password",
      "port": 443,
      "enabled": true
    }
  },
  "credentials": {"nested": ["first-secret", "second-secret"]},
  "repositories": {"main": "https://repo.example.com/public-index"}
}"#,
            );

            let inventory = inspect_project(fixture.path()).expect("inventory");
            let workspace_parent = Fixture::new("composer-auth-workspace");
            let workspace = workspace_parent.path().join("workspace");
            create_filtered_workspace(fixture.path(), &workspace, &inventory).expect("workspace");

            let copied: serde_json::Value = serde_json::from_slice(
                &fs::read(workspace.join(".composer/auth.json")).expect("Composer auth"),
            )
            .expect("redacted Composer auth remains JSON");
            assert_eq!(copied["github-oauth"]["github.com"], REDACTED);
            assert_eq!(
                copied["http-basic"]["repo.example.com"]["username"],
                REDACTED
            );
            assert_eq!(
                copied["http-basic"]["repo.example.com"]["password"],
                REDACTED
            );
            assert_eq!(copied["http-basic"]["repo.example.com"]["port"], REDACTED);
            assert_eq!(
                copied["http-basic"]["repo.example.com"]["enabled"],
                REDACTED
            );
            assert_eq!(copied["credentials"]["nested"][0], REDACTED);
            assert_eq!(copied["credentials"]["nested"][1], REDACTED);
            assert_eq!(
                copied["repositories"]["main"],
                "https://repo.example.com/public-index"
            );
            let serialized = serde_json::to_string(&copied).expect("serialize copied auth");
            for secret in [
                "github-secret",
                "composer-user",
                "composer-password",
                "first-secret",
                "second-secret",
            ] {
                assert!(!serialized.contains(secret));
            }
        }

        #[test]
        fn preserves_public_oauth_metadata_outside_known_credential_containers() {
            let fixture = Fixture::new("public-oauth");
            fixture.write(
                "package.json",
                r#"{"dependencies":{"oauth":"2.0.0","oauth2":"3.0.0","tokenizers":"1.2.3"}}"#,
            );
            fixture.write(
                "config/oauth.json",
                r#"{
  "oauth": {
    "authorization_endpoint": "https://identity.example.com/authorize",
    "scope": "openid profile",
    "client_id": "public-client-id",
    "timeout": 30,
    "pkce": true,
    "client_secret": "private-client-secret"
  }
}"#,
            );

            let inventory = inspect_project(fixture.path()).expect("inventory");
            let workspace_parent = Fixture::new("public-oauth-workspace");
            let workspace = workspace_parent.path().join("workspace");
            create_filtered_workspace(fixture.path(), &workspace, &inventory).expect("workspace");

            let package: serde_json::Value = serde_json::from_slice(
                &fs::read(workspace.join("package.json")).expect("package manifest"),
            )
            .expect("package manifest JSON");
            assert_eq!(package["dependencies"]["oauth"], "2.0.0");
            assert_eq!(package["dependencies"]["oauth2"], "3.0.0");

            let oauth: serde_json::Value = serde_json::from_slice(
                &fs::read(workspace.join("config/oauth.json")).expect("OAuth config"),
            )
            .expect("OAuth config JSON");
            assert_eq!(
                oauth["oauth"]["authorization_endpoint"],
                "https://identity.example.com/authorize"
            );
            assert_eq!(oauth["oauth"]["scope"], "openid profile");
            assert_eq!(oauth["oauth"]["client_id"], "public-client-id");
            assert_eq!(oauth["oauth"]["timeout"], 30);
            assert_eq!(oauth["oauth"]["pkce"], true);
            assert_eq!(oauth["oauth"]["client_secret"], REDACTED);
        }

        #[test]
        fn redacts_nuget_xml_and_excludes_suspicious_unstructured_credentials() {
            let fixture = Fixture::new("xml-secrets");
            fixture.write(
            "NuGet.Config",
            concat!(
                "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n",
                "<configuration><packageSourceCredentials><private>",
                "<add key=\"Username\" value=\"nuget-user\" />",
                "<add key=\"ClearTextPassword\" value=\"nuget-secret\" />",
                "</private></packageSourceCredentials>",
                "<packageSources><add key=\"private\" value=\"https://nuget.example.com/v3/index.json\" />",
                "</packageSources></configuration>\n",
            ),
        );
            fixture.write(
                "notes/credentials.txt",
                "master_key = cannot-safely-keep-this\n",
            );

            let inventory = inspect_project(fixture.path()).expect("inventory");
            assert!(!inventory
                .files
                .iter()
                .any(|file| file.path == "notes/credentials.txt"));
            let workspace_parent = Fixture::new("xml-secrets-workspace");
            let workspace = workspace_parent.path().join("workspace");
            create_filtered_workspace(fixture.path(), &workspace, &inventory).expect("workspace");

            let xml = fs::read_to_string(workspace.join("NuGet.Config")).expect("NuGet config");
            assert!(!xml.contains("nuget-secret"));
            assert!(xml.contains("nuget-user"));
            assert!(xml.contains("https://nuget.example.com/v3/index.json"));
            let mut reader = quick_xml::Reader::from_str(&xml);
            loop {
                if matches!(
                    reader
                        .read_event()
                        .expect("redacted NuGet XML remains parseable"),
                    quick_xml::events::Event::Eof
                ) {
                    break;
                }
            }
        }

        #[test]
        fn excludes_unknown_multi_assignment_xml_bearer_and_opaque_credential_text() {
            let fixture = Fixture::new("unknown-credentials");
            fixture.write(
                "notes/multiple.txt",
                "mode=debug password=multi-assignment-secret region=local\n",
            );
            fixture.write(
                "notes/xmlish.txt",
                "<settings><password>xml-element-secret</password></settings>\n",
            );
            fixture.write(
                "notes/request.txt",
                "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ1c2VyIn0.signature\n",
            );
            fixture.write(
                "notes/oauth.txt",
                "access_token = opaque-credential-payload\n",
            );
            fixture.write("notes/readme.txt", "public architecture notes\n");

            let inventory = inspect_project(fixture.path()).expect("inventory");
            for excluded in [
                "notes/multiple.txt",
                "notes/xmlish.txt",
                "notes/request.txt",
                "notes/oauth.txt",
            ] {
                assert!(
                    !inventory.files.iter().any(|file| file.path == excluded),
                    "credential-bearing text was inventoried: {excluded}"
                );
            }
            assert!(inventory
                .files
                .iter()
                .any(|file| file.path == "notes/readme.txt"));
        }

        #[test]
        fn parses_unknown_xml_credentials_and_fails_closed_on_malformed_vocabulary() {
            let fixture = Fixture::new("unknown-xml-credentials");
            fixture.write(
                "notes/property.xml",
                "<settings><property name=\"password\" value=\"xml-property-secret\"/></settings>\n",
            );
            fixture.write(
                "notes/key-value.xml",
                "<settings><add key=\"api-token\" value=\"xml-token-secret\"/></settings>\n",
            );
            fixture.write(
                "notes/element.xml",
                "<settings><password>xml-element-secret</password></settings>\n",
            );
            fixture.write(
                "notes/malformed.xml",
                "<settings><property name=\"password\" value=\"unclosed-secret\"\n",
            );
            fixture.write(
                "notes/public.xml",
                "<settings><property name=\"region\" value=\"local\"/></settings>\n",
            );

            let inventory = inspect_project(fixture.path()).expect("inventory");
            for excluded in [
                "notes/property.xml",
                "notes/key-value.xml",
                "notes/element.xml",
                "notes/malformed.xml",
            ] {
                assert!(
                    !inventory.files.iter().any(|file| file.path == excluded),
                    "XML credential file was inventoried: {excluded}"
                );
            }
            assert!(inventory
                .files
                .iter()
                .any(|file| file.path == "notes/public.xml"));
        }

        #[test]
        fn source_and_document_scanning_requires_high_confidence_credential_literals() {
            let fixture = Fixture::new("literal-aware");
            fixture.write(
                "src/types.ts",
                "export type Credentials = { token: string; authorization: string };\n",
            );
            fixture.write(
                "src/types.rs",
                "pub fn typed(token: String) { let token: String = token; }\n",
            );
            fixture.write("src/leak.ts", "export const token = \"abc123\";\n");
            fixture.write(
                "docs/auth.md",
                "Use the Bearer authentication scheme with an access token.\n",
            );
            fixture.write(
                "notes/oauth.txt",
                "OAuth redirects use PKCE and the authorization code flow.\n",
            );
            fixture.write(
                "docs/secrets.md",
                "Secrets are injected by the deployment platform; no values live here.\n",
            );

            let inventory = inspect_project(fixture.path()).expect("inventory");
            for retained in [
                "src/types.ts",
                "src/types.rs",
                "docs/auth.md",
                "notes/oauth.txt",
                "docs/secrets.md",
            ] {
                assert!(
                    inventory.files.iter().any(|file| file.path == retained),
                    "safe typed source or documentation was excluded: {retained}"
                );
            }
            assert!(!inventory
                .files
                .iter()
                .any(|file| file.path == "src/leak.ts"));
        }

        #[test]
        fn redacts_package_credentials_and_excludes_unparseable_or_raw_credentials() {
            let fixture = Fixture::new("package-credentials");
            fixture.write(
                ".npmrc",
                concat!(
                    "registry=https://registry.npmjs.org/\n",
                    "//registry.npmjs.org/:_authToken=npm-secret\n",
                ),
            );
            fixture.write(
                ".pypirc",
                concat!(
                    "[distutils]\n",
                    "index-servers = private\n",
                    "[private]\n",
                    "repository = https://pypi.example.com/\n",
                    "password = pypi-secret\n",
                ),
            );
            fixture.write(
                ".git-credentials",
                "https://git-user:git-secret@git.example.com\n",
            );
            fixture.write("config/broken.json", r#"{"token":"broken-secret""#);

            let inventory = inspect_project(fixture.path()).expect("inventory");
            assert!(!inventory
                .files
                .iter()
                .any(|file| file.path == ".git-credentials"));
            assert!(!inventory
                .files
                .iter()
                .any(|file| file.path == "config/broken.json"));

            let workspace_parent = Fixture::new("package-credentials-workspace");
            let workspace = workspace_parent.path().join("workspace");
            create_filtered_workspace(fixture.path(), &workspace, &inventory).expect("workspace");
            let npm = fs::read_to_string(workspace.join(".npmrc")).expect("npm config");
            let pypi = fs::read_to_string(workspace.join(".pypirc")).expect("PyPI config");
            assert!(npm.contains("registry=https://registry.npmjs.org/"));
            assert!(npm.contains("_authToken=[REDACTED]"), "{npm}");
            assert!(!npm.contains("npm-secret"));
            assert!(pypi.contains("repository = https://pypi.example.com/"));
            assert!(pypi.contains("password = [REDACTED]"));
            assert!(!pypi.contains("pypi-secret"));
        }

        #[test]
        fn excludes_all_private_key_encodings_and_putty_keys() {
            let fixture = Fixture::new("private-key-variants");
            fixture.write(
            "secrets/encrypted.txt",
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\nsecret\n-----END ENCRYPTED PRIVATE KEY-----\n",
        );
            fixture.write(
                "secrets/generic.txt",
                "-----BEGIN PRIVATE KEY-----\nsecret\n-----END PRIVATE KEY-----\n",
            );
            fixture.write(
                "secrets/dsa.txt",
                "-----BEGIN DSA PRIVATE KEY-----\nsecret\n-----END DSA PRIVATE KEY-----\n",
            );
            fixture.write(
            "secrets/vendor.txt",
            "-----BEGIN VENDOR HARDWARE PRIVATE KEY-----\nsecret\n-----END VENDOR HARDWARE PRIVATE KEY-----\n",
        );
            fixture.write(
                "secrets/client.ppk",
                "PuTTY-User-Key-File-3: ssh-ed25519\nEncryption: none\nPrivate-Lines: 1\nsecret\n",
            );

            let inventory = inspect_project(fixture.path()).expect("inventory");
            assert!(!inventory
                .files
                .iter()
                .any(|file| file.path.starts_with("secrets/")));
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
        fn excludes_additional_dependency_and_build_cache_directories() {
            let fixture = Fixture::new("additional-caches");
            fixture.write("src/valid.rs", "pub const VALID: bool = true;\n");
            for directory in [
                ".pytest_cache",
                ".mypy_cache",
                ".tox",
                ".turbo",
                ".parcel-cache",
                ".pnpm-store",
                ".output",
                ".ruff_cache",
                ".nox",
                ".hypothesis",
                ".vite",
                ".yarn/unplugged",
                ".angular/cache",
            ] {
                fixture.write(&format!("{directory}/escaped.txt"), "must be excluded\n");
            }

            let inventory = inspect_project(fixture.path()).expect("inventory");
            assert_eq!(
                inventory
                    .files
                    .iter()
                    .map(|file| file.path.as_str())
                    .collect::<Vec<_>>(),
                vec!["src/valid.rs"]
            );
        }

        #[test]
        fn nested_package_uses_the_nearest_ancestor_package_manager_lockfile() {
            let fixture = Fixture::new("ancestor-lockfile");
            fixture.write("pnpm-lock.yaml", "lockfileVersion: '9.0'\n");
            fixture.write(
                "packages/console/package.json",
                r#"{"scripts":{"test":"vitest"},"dependencies":{"vue":"3.5.0"}}"#,
            );

            let inventory = inspect_project(fixture.path()).expect("inventory");
            assert!(inventory.commands.iter().any(|command| {
                command.cwd == "packages/console"
                    && command.name == "test"
                    && command.command == "pnpm run test"
            }));
        }

        #[test]
        fn rejects_an_external_workspace_parent_symlink_without_writing_outside() {
            use std::os::unix::fs::symlink;

            let fixture = Fixture::new("external-workspace-source");
            fixture.write("src/main.rs", "fn main() {}\n");
            let inventory = inspect_project(fixture.path()).expect("inventory");
            let workspace_base = Fixture::new("external-workspace-base");
            let outside = Fixture::new("external-workspace-outside");
            symlink(outside.path(), workspace_base.path().join("linked-parent"))
                .expect("external workspace parent symlink");
            let workspace = workspace_base.path().join("linked-parent/workspace");

            assert!(create_filtered_workspace(fixture.path(), &workspace, &inventory).is_err());
            assert!(!outside.path().join("workspace").exists());
        }

        #[test]
        fn securely_creates_multiple_missing_workspace_parent_directories() {
            let fixture = Fixture::new("missing-workspace-source");
            fixture.write("src/main.rs", "fn main() {}\n");
            let inventory = inspect_project(fixture.path()).expect("inventory");
            let workspace_base = Fixture::new("missing-workspace-base");
            let workspace = workspace_base.path().join("runs/missing/nested/workspace");

            create_filtered_workspace(fixture.path(), &workspace, &inventory).expect("workspace");
            assert!(workspace.join("src/main.rs").is_file());
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

    #[cfg(windows)]
    #[test]
    fn windows_inventory_and_workspace_are_explicitly_unsupported_until_handle_safe() {
        let fixture = Fixture::new("windows-unsupported");
        fixture.write("src/main.rs", "fn main() {}\n");
        let inventory_error = inspect_project(fixture.path()).expect_err("unsupported scan");
        assert!(inventory_error.contains("unsupported"));

        let inventory = ProjectInventory {
            schema_version: 1,
            project_name: "fixture".to_string(),
            layers: ProjectLayers {
                frontend: false,
                backend: false,
            },
            modules: Vec::new(),
            source_roots: Vec::new(),
            files: Vec::new(),
            commands: Vec::new(),
            risk_keys: Vec::new(),
        };
        let error = create_filtered_workspace(
            fixture.path(),
            &fixture.path().join("workspace"),
            &inventory,
        )
        .expect_err("unsupported workspace");
        assert!(error.contains("unsupported"));
    }
}
