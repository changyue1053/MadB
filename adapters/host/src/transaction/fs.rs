use super::{fail, io_error};
use madb_app::Result;
use rustix::fs::{
    AtFlags, Dir, FileType, FlockOperation, Mode, OFlags, flock, mkdirat, openat, statat, unlinkat,
};
use std::{fs::File, io::Write, os::unix::fs::MetadataExt};
const DIR_MODE: Mode = Mode::from_bits_retain(0o700);

pub(super) fn directory(root: &File, path: &str) -> Result<File> {
    let mut current = root.try_clone().map_err(|e| io_error(path, e))?;
    for part in path.split('/') {
        current = File::from(
            openat(
                &current,
                part,
                OFlags::RDONLY
                    | OFlags::DIRECTORY
                    | OFlags::NOFOLLOW
                    | OFlags::CLOEXEC
                    | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(|e| io_error(path, e))?,
        );
    }
    Ok(current)
}
pub(super) fn ensure_directory(root: &File, name: &str) -> Result<File> {
    match mkdirat(root, name, DIR_MODE) {
        Ok(()) => root.sync_all().map_err(|e| io_error(name, e))?,
        Err(rustix::io::Errno::EXIST) => {}
        Err(error) => return Err(io_error(name, error)),
    }
    directory(root, name)
}
pub(super) fn create_directory(root: &File, name: &str) -> Result<File> {
    mkdirat(root, name, DIR_MODE).map_err(|e| io_error(name, e))?;
    directory(root, name)
}
pub(super) fn regular(root: &File, name: &str, create: bool, exclusive: bool) -> Result<File> {
    let flags = OFlags::CLOEXEC
        | OFlags::NOFOLLOW
        | OFlags::NONBLOCK
        | if create {
            OFlags::RDWR | OFlags::CREATE
        } else {
            OFlags::RDONLY
        }
        | if exclusive {
            OFlags::EXCL
        } else {
            OFlags::empty()
        };
    let file = File::from(
        openat(root, name, flags, Mode::from_bits_retain(0o600)).map_err(|e| io_error(name, e))?,
    );
    let metadata = file.metadata().map_err(|e| io_error(name, e))?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(fail(
            "STORE004",
            name,
            "expected a single-link regular file",
        ));
    }
    Ok(file)
}
pub(super) fn lock(root: &File, name: &str, operation: FlockOperation) -> Result<File> {
    let file = regular(root, name, false, false)?;
    flock(&file, operation).map_err(|e| io_error(name, e))?;
    Ok(file)
}
pub(super) fn exists(root: &File, name: &str) -> Result<bool> {
    match statat(root, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => Ok(true),
        Err(rustix::io::Errno::NOENT) => Ok(false),
        Err(e) => Err(io_error(name, e)),
    }
}
pub(super) fn write(root: &File, name: &str, bytes: &[u8]) -> Result<File> {
    let mut file = regular(root, name, true, true)?;
    file.write_all(bytes).map_err(|e| io_error(name, e))?;
    Ok(file)
}
pub(super) fn names(root: &File, limit: usize) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in Dir::read_from(root).map_err(|e| io_error("directory", e))? {
        let entry = entry.map_err(|e| io_error("directory", e))?;
        let name = entry
            .file_name()
            .to_str()
            .map_err(|e| io_error("directory", e))?;
        if matches!(name, "." | "..") {
            continue;
        }
        if names.len() >= limit {
            return Err(fail("STORE007", "directory", "store entry budget exceeded"));
        }
        names.push(name.to_owned());
    }
    names.sort();
    Ok(names)
}
/// Only called for store-owned, unindexed, exclusively locked trees. Does not
/// follow links. A fixed node budget prevents an unbounded recovery traversal.
pub(super) fn remove_tree(parent: &File, name: &str, remaining: &mut usize) -> Result<()> {
    *remaining = remaining
        .checked_sub(1)
        .ok_or_else(|| fail("STORE007", name, "recovery node budget exceeded"))?;
    let stat = statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|e| io_error(name, e))?;
    if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
        let dir = directory(parent, name)?;
        rustix::fs::fchmod(&dir, DIR_MODE).map_err(|e| io_error(name, e))?;
        for child in names(&dir, *remaining)? {
            remove_tree(&dir, &child, remaining)?;
        }
        unlinkat(parent, name, AtFlags::REMOVEDIR).map_err(|e| io_error(name, e))?;
    } else {
        unlinkat(parent, name, AtFlags::empty()).map_err(|e| io_error(name, e))?;
    }
    Ok(())
}
pub(super) fn readonly_tree(root: &File, directories: &[madb_app::LogicalPath]) -> Result<()> {
    for path in directories.iter().rev() {
        let dir = directory(root, path.as_str())?;
        rustix::fs::fchmod(&dir, Mode::from_bits_retain(0o500))
            .map_err(|e| io_error(path.as_str(), e))?;
        dir.sync_all().map_err(|e| io_error(path.as_str(), e))?;
    }
    rustix::fs::fchmod(root, Mode::from_bits_retain(0o500)).map_err(|e| io_error("files", e))?;
    root.sync_all().map_err(|e| io_error("files", e))
}
