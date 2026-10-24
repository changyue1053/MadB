//! Descriptor-relative metadata observations. Only ENOENT means absence;
//! permissions, symlink parents and non-directory parents remain errors.
use crate::LocalWorkspace;
use madb_app::{
    ErrorKind, LogicalPath, QueryError, Result,
    input_observation::{DirectoryEntry, DirectoryEntryKind, ObserveWorkspace},
};

#[cfg(unix)]
impl ObserveWorkspace for LocalWorkspace {
    fn exists(&self, path: &LogicalPath) -> Result<bool> {
        use rustix::fs::{AtFlags, statat};
        let (parent, name) = path
            .as_str()
            .rsplit_once('/')
            .unwrap_or(("", path.as_str()));
        let Some(directory) = existing_directory(&self.root, parent)? else {
            return Ok(false);
        };
        match statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW) {
            // Any terminal entry defeats an absence assertion, including a
            // dangling symlink. Never open it merely to decide presence.
            Ok(_) => Ok(true),
            Err(rustix::io::Errno::NOENT) => Ok(false),
            Err(error) => Err(observation_error(path.as_str(), error)),
        }
    }

    fn list_directory(
        &self,
        path: &LogicalPath,
        entry_limit: usize,
        name_bytes_limit: usize,
    ) -> Result<Vec<DirectoryEntry>> {
        use rustix::fs::{AtFlags, Dir, FileType, statat};
        let directory = existing_directory(&self.root, path.as_str())?
            .ok_or_else(|| observation_error(path.as_str(), "observed directory is missing"))?;
        let mut entries = Vec::new();
        let mut name_bytes = 0usize;
        // Reopening each call gives a fresh directory stream; retained streams
        // must not leave a later verification at the previous EOF position.
        let stream =
            Dir::read_from(&directory).map_err(|error| observation_error(path.as_str(), error))?;
        for entry in stream {
            let entry = entry.map_err(|error| observation_error(path.as_str(), error))?;
            let name = entry
                .file_name()
                .to_str()
                .map_err(|error| observation_error(path.as_str(), error))?;
            if matches!(name, "." | "..") {
                continue;
            }
            if entries.len() >= entry_limit
                || name.len() > name_bytes_limit.saturating_sub(name_bytes)
            {
                return Err(QueryError::new(
                    ErrorKind::InvalidInput,
                    "HOST003",
                    path.as_str(),
                    "directory enumeration exceeds entry or name-byte budget",
                ));
            }
            if !name.is_ascii() || name.contains('/') {
                return Err(observation_error(
                    path.as_str(),
                    "directory member must have one ASCII name",
                ));
            }
            let logical_name =
                LogicalPath::new(name).map_err(|error| observation_error(path.as_str(), error))?;
            // Do not trust d_type: it may be unknown or stale. A racing removal
            // is an error, never a silently omitted member.
            let stat = statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW)
                .map_err(|error| observation_error(path.as_str(), error))?;
            let kind = match FileType::from_raw_mode(stat.st_mode) {
                FileType::RegularFile if stat.st_nlink == 1 => DirectoryEntryKind::File,
                FileType::Directory => DirectoryEntryKind::Directory,
                _ => {
                    return Err(observation_error(
                        path.as_str(),
                        "observed members must be directories or regular files with one link",
                    ));
                }
            };
            name_bytes += name.len();
            entries.push(DirectoryEntry {
                name: logical_name,
                kind,
            });
        }
        entries.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries)
    }
}

#[cfg(unix)]
fn existing_directory(root: &std::fs::File, path: &str) -> Result<Option<std::fs::File>> {
    use rustix::fs::{Mode, OFlags, openat};
    let mut directory = root
        .try_clone()
        .map_err(|error| observation_error(path, error))?;
    if path.is_empty() {
        return Ok(Some(directory));
    }
    for part in path.split('/') {
        let next = openat(
            &directory,
            part,
            OFlags::RDONLY
                | OFlags::DIRECTORY
                | OFlags::CLOEXEC
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK,
            Mode::empty(),
        );
        match next {
            Ok(fd) => directory = std::fs::File::from(fd),
            Err(rustix::io::Errno::NOENT) => return Ok(None),
            Err(error) => return Err(observation_error(path, error)),
        }
    }
    Ok(Some(directory))
}

#[cfg(unix)]
fn observation_error(path: &str, error: impl std::fmt::Display) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "HOST002", path, error.to_string())
}

#[cfg(not(unix))]
impl ObserveWorkspace for LocalWorkspace {
    fn exists(&self, _: &LogicalPath) -> Result<bool> {
        Err(QueryError::new(
            ErrorKind::Host,
            "HOST004",
            "platform",
            "input observations require Unix",
        ))
    }
    fn list_directory(&self, _: &LogicalPath, _: usize, _: usize) -> Result<Vec<DirectoryEntry>> {
        Err(QueryError::new(
            ErrorKind::Host,
            "HOST004",
            "platform",
            "input observations require Unix",
        ))
    }
}
