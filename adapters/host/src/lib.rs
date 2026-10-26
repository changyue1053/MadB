//! Host-owned bounded filesystem reads and approved process execution.
pub mod artifact;
pub mod cache;
pub mod cancellation;
pub mod dag;
pub mod dependencies;
pub mod extension_files;
pub mod inputs;
pub mod locations;
mod observation;
pub mod process;
pub mod task;
pub mod transaction;
use madb_app::{ErrorKind, LogicalPath, QueryError, Result, ports::ReadWorkspace};
use std::path::Path;

/// Root is resolved once by the caller's chosen filesystem directory. Every
/// descendant is opened relative to directory descriptors without symlinks.
pub struct LocalWorkspace {
    root: std::fs::File,
}
impl LocalWorkspace {
    #[cfg(unix)]
    pub fn open(path: &Path) -> Result<Self> {
        let root = std::fs::File::from(
            rustix::fs::open(
                path,
                rustix::fs::OFlags::RDONLY
                    | rustix::fs::OFlags::DIRECTORY
                    | rustix::fs::OFlags::CLOEXEC
                    | rustix::fs::OFlags::NONBLOCK,
                rustix::fs::Mode::empty(),
            )
            .map_err(|e| host_error("root", std::io::Error::from(e)))?,
        );
        if !root.metadata().map_err(|e| host_error("root", e))?.is_dir() {
            return Err(QueryError::new(
                ErrorKind::InvalidInput,
                "HOST002",
                "root",
                "workspace root must be a directory",
            ));
        }
        Ok(Self { root })
    }
    #[cfg(not(unix))]
    pub fn open(_path: &Path) -> Result<Self> {
        Err(QueryError::new(
            ErrorKind::Host,
            "HOST004",
            "platform",
            "confined workspace reads are not implemented on this host",
        ))
    }
}

#[cfg(unix)]
impl ReadWorkspace for LocalWorkspace {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        use rustix::fs::{Mode, OFlags, openat};
        use std::io::Read;
        use std::os::unix::fs::MetadataExt;
        let mut directory = self
            .root
            .try_clone()
            .map_err(|e| host_error(path.as_str(), e))?;
        let mut parts = path.as_str().split('/').peekable();
        while let Some(part) = parts.next() {
            let is_directory = parts.peek().is_some();
            let flags = OFlags::RDONLY
                | OFlags::CLOEXEC
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK
                | if is_directory {
                    OFlags::DIRECTORY
                } else {
                    OFlags::empty()
                };
            let fd = match openat(&directory, part, flags, Mode::empty()) {
                Ok(fd) => fd,
                Err(rustix::io::Errno::NOENT) => return Ok(None),
                Err(e) => return Err(host_error(path.as_str(), std::io::Error::from(e))),
            };
            let file = std::fs::File::from(fd);
            if is_directory {
                directory = file;
                continue;
            }
            let metadata = file.metadata().map_err(|e| host_error(path.as_str(), e))?;
            if !metadata.is_file() || metadata.nlink() != 1 {
                return Err(QueryError::new(
                    ErrorKind::InvalidInput,
                    "HOST002",
                    path.as_str(),
                    "expected a regular file with one link",
                ));
            }
            if metadata.len() > limit as u64 {
                return Err(limit_error(path));
            }
            let mut bytes = Vec::new();
            file.take(limit as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(|e| host_error(path.as_str(), e))?;
            if bytes.len() > limit {
                return Err(limit_error(path));
            }
            return Ok(Some(bytes));
        }
        unreachable!("LogicalPath is nonempty")
    }
}

#[cfg(not(unix))]
impl ReadWorkspace for LocalWorkspace {
    fn read(&self, _path: &LogicalPath, _limit: usize) -> Result<Option<Vec<u8>>> {
        let _ = &self.root;
        Err(QueryError::new(
            ErrorKind::Host,
            "HOST004",
            "platform",
            "confined workspace reads are not implemented on this host",
        ))
    }
}

#[cfg(unix)]
fn host_error(path: &str, error: std::io::Error) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "HOST002", path, error.to_string())
}
#[cfg(unix)]
fn limit_error(path: &LogicalPath) -> QueryError {
    QueryError::new(
        ErrorKind::InvalidInput,
        "HOST003",
        path.as_str(),
        "file exceeds the bounded read limit",
    )
}
