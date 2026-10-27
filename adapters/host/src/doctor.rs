//! The doctor's workspace has no writer or process entry point. The root is
//! opened component by component and retained, not resolved through symlinks.
use crate::LocalWorkspace;
use madb_app::use_cases::doctor::RootPath;
use madb_app::{ErrorKind, QueryError, Result};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};
type PathIdentities = Vec<(u64, u64, u32)>;
pub struct DoctorWorkspace {
    workspace: LocalWorkspace,
    root: RootPath,
    identities: RefCell<BTreeMap<madb_app::LogicalPath, PathIdentities>>,
}
impl DoctorWorkspace {
    pub fn open(path: &Path) -> Result<Self> {
        let base = if path.is_absolute() {
            path.to_path_buf()
        } else {
            std::env::current_dir()
                .map_err(|e| host("root", e))?
                .join(path)
        };
        let mut normalized = PathBuf::new();
        for part in base.components() {
            match part {
                Component::RootDir => normalized.push("/"),
                Component::Normal(p) => normalized.push(p),
                Component::CurDir => {}
                _ => return Err(invalid("root must not contain parent traversal")),
            }
        }
        let text = normalized
            .to_str()
            .ok_or_else(|| invalid("root is not UTF-8"))?;
        if text.chars().count() > 4096 || text.chars().any(char::is_control) {
            return Err(invalid("root exceeds the display path contract"));
        }
        let root = RootPath::new(text)?;
        let workspace = LocalWorkspace {
            root: open_directory(&normalized)?,
        };
        Ok(Self {
            workspace,
            root,
            identities: RefCell::new(BTreeMap::new()),
        })
    }
    pub fn root(&self) -> &RootPath {
        &self.root
    }
    pub fn workspace(&self) -> &Self {
        self
    }
    #[cfg(unix)]
    pub fn revalidate(&self) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        let current = open_directory(Path::new(self.root.as_str()))
            .and_then(|v| v.metadata().map_err(|e| host("root", e)));
        let held = self
            .workspace
            .root
            .metadata()
            .map_err(|e| host("root", e))?;
        if !current.is_ok_and(|v| v.dev() == held.dev() && v.ino() == held.ino()) {
            return Err(changed("doctor workspace path changed during observation"));
        }
        for (path, expected) in self.identities.borrow().iter() {
            if identities(&self.workspace.root, path).as_ref().ok() != Some(expected) {
                return Err(changed(
                    "captured input path identity changed during observation",
                ));
            }
        }
        Ok(())
    }
    #[cfg(not(unix))]
    pub fn revalidate(&self) -> Result<()> {
        Err(host("root", "doctor root observation requires Unix"))
    }
}
#[cfg(unix)]
impl madb_app::ports::ReadWorkspace for DoctorWorkspace {
    fn read(&self, path: &madb_app::LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        let before = identities(&self.workspace.root, path)?;
        let bytes = self.workspace.read_with_policy(path, limit, true);
        let after = identities(&self.workspace.root, path)?;
        if before != after {
            return Err(changed("input identity changed while reading"));
        }
        let mut captured = self.identities.borrow_mut();
        if captured.get(path).is_some_and(|value| value != &before) {
            return Err(changed("input was replaced since the first observation"));
        }
        captured.entry(path.clone()).or_insert(before);
        bytes
    }
}
#[cfg(not(unix))]
impl madb_app::ports::ReadWorkspace for DoctorWorkspace {
    fn read(&self, _: &madb_app::LogicalPath, _: usize) -> Result<Option<Vec<u8>>> {
        Err(host("platform", "doctor requires Unix"))
    }
}
#[cfg(unix)]
fn identities(root: &std::fs::File, path: &madb_app::LogicalPath) -> Result<Vec<(u64, u64, u32)>> {
    use rustix::fs::{Mode, OFlags, openat};
    use std::os::unix::fs::MetadataExt;
    let mut directory = root.try_clone().map_err(|e| host("root", e))?;
    let mut values = Vec::new();
    let mut parts = path.as_str().split('/').peekable();
    while let Some(part) = parts.next() {
        let flags = OFlags::RDONLY
            | OFlags::CLOEXEC
            | OFlags::NONBLOCK
            | OFlags::NOFOLLOW
            | if parts.peek().is_some() {
                OFlags::DIRECTORY
            } else {
                OFlags::empty()
            };
        let file = match openat(&directory, part, flags, Mode::empty()) {
            Ok(fd) => std::fs::File::from(fd),
            Err(rustix::io::Errno::NOENT) => return Ok(values),
            Err(error) => return Err(crate::read_error(path.as_str(), error.into(), true)),
        };
        let metadata = file.metadata().map_err(|e| host(path.as_str(), e))?;
        values.push((metadata.dev(), metadata.ino(), metadata.mode()));
        directory = file;
    }
    Ok(values)
}
fn changed(message: &str) -> QueryError {
    QueryError::new(ErrorKind::Conflict, "QUERY002", "doctor inputs", message)
}
#[cfg(unix)]
fn open_directory(path: &Path) -> Result<std::fs::File> {
    use rustix::fs::{Mode, OFlags, open, openat};
    let flags =
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
    let mut directory =
        std::fs::File::from(open("/", flags, Mode::empty()).map_err(|e| host("root", e))?);
    for part in path.components() {
        if let Component::Normal(name) = part {
            directory = std::fs::File::from(
                openat(&directory, name, flags, Mode::empty()).map_err(|e| host("root", e))?,
            );
        }
    }
    Ok(directory)
}
#[cfg(not(unix))]
fn open_directory(_: &Path) -> Result<std::fs::File> {
    Err(host("root", "doctor root observation requires Unix"))
}
fn invalid(message: &str) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "PLAN001", "root", message)
}
fn host(subject: &str, error: impl std::fmt::Display) -> QueryError {
    QueryError::new(ErrorKind::Host, "HOST002", subject, error.to_string())
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use madb_app::{ErrorKind, LogicalPath, ports::ReadWorkspace};
    use std::{
        fs,
        os::unix::fs::{PermissionsExt, symlink},
        sync::atomic::{AtomicU64, Ordering},
    };
    fn root() -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let root = fs::canonicalize(std::env::temp_dir())
            .unwrap()
            .join(format!(
                "madb-doctor-host-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir(&root).unwrap();
        root
    }
    #[test]
    fn same_bytes_replacement_and_directory_rename_are_conflicts() {
        let root = root();
        fs::write(root.join("a"), b"same").unwrap();
        let workspace = DoctorWorkspace::open(&root).unwrap();
        let path = LogicalPath::new("a").unwrap();
        assert_eq!(workspace.read(&path, 4).unwrap(), Some(b"same".to_vec()));
        fs::rename(root.join("a"), root.join("old")).unwrap();
        fs::write(root.join("a"), b"same").unwrap();
        assert_eq!(
            workspace.revalidate().unwrap_err().kind,
            ErrorKind::Conflict
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn root_and_descendant_links_are_not_followed() {
        let root = root();
        fs::create_dir(root.join("real")).unwrap();
        symlink("real", root.join("alias")).unwrap();
        assert!(DoctorWorkspace::open(&root.join("alias")).is_err());
        let workspace = DoctorWorkspace::open(&root).unwrap();
        assert_eq!(
            workspace
                .read(&LogicalPath::new("alias/file").unwrap(), 8)
                .unwrap_err()
                .kind,
            ErrorKind::InvalidInput
        );
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn permission_error_is_host_and_controls_are_invalid() {
        let root = root();
        let file = root.join("unreadable");
        fs::write(&file, b"secret").unwrap();
        fs::set_permissions(&file, fs::Permissions::from_mode(0o0)).unwrap();
        let workspace = DoctorWorkspace::open(&root).unwrap();
        let result = workspace.read(&LogicalPath::new("unreadable").unwrap(), 10);
        fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(result.unwrap_err().kind, ErrorKind::Host);
        for text in [
            "/private/tmp/bad\0",
            "/private/tmp/bad\n",
            "/private/tmp/bad\u{7f}",
        ] {
            assert_eq!(
                DoctorWorkspace::open(Path::new(text)).err().unwrap().kind,
                ErrorKind::InvalidInput
            );
        }
        fs::remove_dir_all(root).unwrap();
    }
}
