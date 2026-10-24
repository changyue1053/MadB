//! Host paths for explicit CLI composition. Descendant directories never follow
//! symlinks; callers keep the named workspace root stable during the operation.
use crate::LocalWorkspace;
use madb_app::{ErrorKind, LogicalPath, QueryError, Result};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

/// Resolve an existing directory, or create its missing components when the
/// operation permits writes. A read-only lookup never creates lock/cache paths.
#[cfg(unix)]
pub fn directory(root: &Path, relative: &LogicalPath, create: bool) -> Result<Option<PathBuf>> {
    use rustix::fs::{Mode, OFlags, mkdirat, openat};
    if !relative.as_str().is_ascii() || relative.as_str().split('/').count() > 32 {
        return Err(error(
            relative.as_str(),
            "managed paths require ASCII and at most 32 components",
        ));
    }
    let root = std::fs::canonicalize(root).map_err(|e| error("root", e))?;
    let mut directory = LocalWorkspace::open(&root)?.root;
    for part in relative.as_str().split('/') {
        let flags = OFlags::RDONLY
            | OFlags::DIRECTORY
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | OFlags::NONBLOCK;
        let opened = match openat(&directory, part, flags, Mode::empty()) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT) if !create => return Ok(None),
            Err(rustix::io::Errno::NOENT) => {
                match mkdirat(&directory, part, Mode::from_bits_retain(0o700)) {
                    Ok(()) | Err(rustix::io::Errno::EXIST) => {}
                    Err(e) => return Err(error(relative.as_str(), e)),
                }
                openat(&directory, part, flags, Mode::empty())
                    .map_err(|e| error(relative.as_str(), e))?
            }
            Err(e) => return Err(error(relative.as_str(), e)),
        };
        directory = opened.into();
    }
    Ok(Some(root.join(relative.as_str())))
}

#[cfg(not(unix))]
pub fn directory(_: &Path, _: &LogicalPath, _: bool) -> Result<Option<PathBuf>> {
    Err(error(
        "platform",
        "managed execution paths require a Unix host",
    ))
}

/// Names are only allocation candidates: PreparedRun still creates exclusively.
/// Neither wall time nor a run path contributes to a stable recipe/cache identity.
pub fn unique_run_name() -> LogicalPath {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |value| value.as_nanos());
    LogicalPath::new(format!("madb-{}-{nonce:x}-{count:x}", std::process::id()))
        .expect("generated ASCII path")
}

pub fn temporary_parent() -> Result<PathBuf> {
    std::fs::canonicalize(std::env::temp_dir()).map_err(|e| error("temporary-directory", e))
}

fn error(subject: &str, cause: impl std::fmt::Display) -> QueryError {
    QueryError::new(ErrorKind::Host, "HOST005", subject, cause.to_string())
}
