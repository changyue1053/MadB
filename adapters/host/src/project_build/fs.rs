use super::*;
use rustix::fs::{AtFlags, Dir, FileType, Mode, OFlags, mkdirat, open, openat, statat};
use std::{
    io::Write,
    os::unix::fs::{FileExt, MetadataExt},
    path::{Component, Path},
    sync::atomic::{AtomicU64, Ordering},
};

const FLAGS: OFlags = OFlags::RDONLY
    .union(OFlags::CLOEXEC)
    .union(OFlags::NOFOLLOW)
    .union(OFlags::NONBLOCK);
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Stamp {
    dev: u64,
    ino: u64,
    size: u64,
    links: u64,
    modified: (i64, i64),
    changed: (i64, i64),
}
impl Stamp {
    pub(super) fn same_inode(&self, other: &Self) -> bool {
        self.dev == other.dev && self.ino == other.ino
    }
}
pub(super) fn stamp(file: &File) -> Result<Stamp> {
    let m = file.metadata().map_err(|e| host("metadata", e))?;
    Ok(Stamp {
        dev: m.dev(),
        ino: m.ino(),
        size: m.size(),
        links: m.nlink(),
        modified: (m.mtime(), m.mtime_nsec()),
        changed: (m.ctime(), m.ctime_nsec()),
    })
}
pub(super) fn same_directory(a: &File, b: &File) -> Result<bool> {
    let a = stamp(a)?;
    let b = stamp(b)?;
    Ok(a.dev == b.dev && a.ino == b.ino)
}
pub(super) fn absolute(path: &Path) -> Result<File> {
    let mut parts = path.components();
    if parts.next() != Some(Component::RootDir) {
        return Err(invalid("root", "expected normalized absolute directory"));
    }
    let mut dir = File::from(
        open("/", FLAGS | OFlags::DIRECTORY, Mode::empty()).map_err(|e| host("root", e))?,
    );
    for part in parts {
        let Component::Normal(name) = part else {
            return Err(invalid("root", "unnormalized directory"));
        };
        dir = File::from(
            openat(&dir, name, FLAGS | OFlags::DIRECTORY, Mode::empty())
                .map_err(|e| host("root", e))?,
        );
    }
    Ok(dir)
}
pub(super) fn directory(root: &File, path: &str) -> Result<File> {
    if path.is_empty() {
        return root.try_clone().map_err(|e| host("directory", e));
    }
    logical(path)?;
    let mut dir = root.try_clone().map_err(|e| host(path, e))?;
    for part in path.split('/') {
        dir = File::from(
            openat(&dir, part, FLAGS | OFlags::DIRECTORY, Mode::empty())
                .map_err(|e| host(path, e))?,
        );
    }
    Ok(dir)
}
pub(super) fn parent(root: &File, path: &str) -> Result<(File, String)> {
    logical(path)?;
    let (p, n) = path.rsplit_once('/').unwrap_or(("", path));
    Ok((directory(root, p)?, n.into()))
}
pub(super) fn exists(root: &File, name: &str) -> Result<bool> {
    logical(name)?;
    match statat(root, name, AtFlags::SYMLINK_NOFOLLOW) {
        Ok(_) => Ok(true),
        Err(rustix::io::Errno::NOENT) => Ok(false),
        Err(e) => Err(host(name, e)),
    }
}
pub(super) fn regular(root: &File, path: &str) -> Result<File> {
    let (parent, name) = parent(root, path)?;
    let f = File::from(
        openat(&parent, name.as_str(), FLAGS, Mode::empty()).map_err(|e| host(path, e))?,
    );
    let m = f.metadata().map_err(|e| host(path, e))?;
    if !m.is_file() || m.nlink() != 1 {
        return Err(conflict(path, "expected a single-link regular file"));
    }
    Ok(f)
}
pub(super) fn read(root: &File, path: &str, limit: usize) -> Result<(Vec<u8>, Stamp)> {
    let file = regular(root, path)?;
    let before = stamp(&file)?;
    if before.size > limit as u64 {
        return Err(invalid(path, "file exceeds the finite byte budget"));
    }
    let mut bytes = vec![0; before.size as usize];
    file.read_exact_at(&mut bytes, 0)
        .map_err(|e| host(path, e))?;
    if stamp(&file)? != before || stamp(&regular(root, path)?)? != before {
        return Err(conflict(path, "file changed while captured"));
    }
    Ok((bytes, before))
}
/// A captured length is an identity, not a new input budget. Check its stamp
/// before allocating/reading, then recheck both the FD and its confined name.
pub(super) fn verify_captured(
    root: &File,
    path: &str,
    expected: &[u8],
    identity: &Stamp,
) -> Result<()> {
    verify_captured_with_reader(root, path, expected, identity, |file, bytes| {
        file.read_exact_at(bytes, 0)
    })
}
pub(super) fn verify_captured_with_reader(
    root: &File,
    path: &str,
    expected: &[u8],
    identity: &Stamp,
    read_bytes: impl FnOnce(&File, &mut [u8]) -> std::io::Result<()>,
) -> Result<()> {
    let file = captured_regular(root, path)?;
    if &stamp(&file)? != identity {
        return Err(conflict(path, "captured input identity changed"));
    }
    let mut bytes = vec![0; expected.len()];
    let read_result = read_bytes(&file, &mut bytes);
    // A truncate can produce EOF before the post-read stamp check. Other IO
    // errors retain their real Host classification, even if a writer also ran.
    if let Err(error) = &read_result
        && error.kind() != std::io::ErrorKind::UnexpectedEof
    {
        return Err(host(path, error));
    }
    if &stamp(&file)? != identity || &stamp(&captured_regular(root, path)?)? != identity {
        return Err(conflict(path, "captured input changed while read"));
    }
    read_result.map_err(|error| host(path, error))?;
    if bytes != expected {
        return Err(conflict(path, "captured input bytes changed"));
    }
    Ok(())
}
fn captured_regular(root: &File, path: &str) -> Result<File> {
    logical(path)?;
    let (parents, name) = path.rsplit_once('/').unwrap_or(("", path));
    let mut directory = root.try_clone().map_err(|error| host(path, error))?;
    for part in parents.split('/').filter(|part| !part.is_empty()) {
        directory = File::from(
            openat(&directory, part, FLAGS | OFlags::DIRECTORY, Mode::empty())
                .map_err(|error| captured_path_error(path, error))?,
        );
    }
    let named = statat(&directory, name, AtFlags::SYMLINK_NOFOLLOW)
        .map_err(|error| captured_path_error(path, error))?;
    if FileType::from_raw_mode(named.st_mode) != FileType::RegularFile || named.st_nlink != 1 {
        return Err(conflict(
            path,
            "captured input is no longer a single-link regular file",
        ));
    }
    let file = File::from(
        openat(&directory, name, FLAGS, Mode::empty())
            .map_err(|error| captured_path_error(path, error))?,
    );
    let metadata = file.metadata().map_err(|error| host(path, error))?;
    if !metadata.is_file() || metadata.nlink() != 1 {
        return Err(conflict(
            path,
            "captured input is no longer a single-link regular file",
        ));
    }
    Ok(file)
}
fn captured_path_error(path: &str, error: rustix::io::Errno) -> QueryError {
    match error {
        rustix::io::Errno::NOENT | rustix::io::Errno::NOTDIR | rustix::io::Errno::LOOP => {
            conflict(path, "captured input path disappeared or changed type")
        }
        _ => host(path, error),
    }
}
pub(super) fn create_directory(root: &File, name: &str) -> Result<File> {
    logical(name)?;
    if name.contains('/') {
        return Err(invalid(name, "creation requires one direct name"));
    }
    mkdirat(root, name, Mode::from_bits_retain(0o700)).map_err(|e| conflict(name, e))?;
    root.sync_all().map_err(|e| host(name, e))?;
    directory(root, name)
}
pub(super) fn ensure_directories(root: &File, path: &str) -> Result<File> {
    logical(path)?;
    let mut dir = root.try_clone().map_err(|e| host(path, e))?;
    for name in path.split('/') {
        dir = if exists(&dir, name)? {
            directory(&dir, name)?
        } else {
            // A second authorized operation may create the same shared parent
            // after our absence observation. Only EEXIST is a reopen; all other
            // failures retain their error, and NOFOLLOW still rejects a link.
            match mkdirat(&dir, name, Mode::from_bits_retain(0o700)) {
                Ok(()) => dir.sync_all().map_err(|e| host(name, e))?,
                Err(rustix::io::Errno::EXIST) => {}
                Err(error) => return Err(conflict(name, error)),
            }
            directory(&dir, name)?
        };
    }
    Ok(dir)
}
pub(super) fn write_new(root: &File, path: &str, bytes: &[u8], mode: u32) -> Result<File> {
    let (parent, name) = parent(root, path)?;
    let mut file = File::from(
        openat(
            &parent,
            name.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::from_raw_mode(mode as _),
        )
        .map_err(|e| conflict(path, e))?,
    );
    file.write_all(bytes).map_err(|e| host(path, e))?;
    file.sync_all().map_err(|e| host(path, e))?;
    parent.sync_all().map_err(|e| host(path, e))?;
    Ok(file)
}
pub(super) fn names(root: &File, limit: usize) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in Dir::read_from(root).map_err(|e| host("directory", e))? {
        let entry = entry.map_err(|e| host("directory", e))?;
        let name = entry
            .file_name()
            .to_str()
            .map_err(|e| invalid("directory", e.to_string()))?;
        if matches!(name, "." | "..") {
            continue;
        }
        if names.len() >= limit {
            return Err(invalid("directory", "member budget exceeded"));
        }
        logical(name)?;
        names.push(name.to_owned());
    }
    names.sort();
    let mut folded = std::collections::BTreeSet::new();
    for n in &names {
        if !folded.insert(n.to_ascii_lowercase()) {
            return Err(invalid(n, "case alias in directory"));
        }
    }
    Ok(names)
}
pub(super) fn kind(root: &File, name: &str) -> Result<MemberKind> {
    let s = statat(root, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|e| host(name, e))?;
    match FileType::from_raw_mode(s.st_mode) {
        FileType::Directory => Ok(MemberKind::Directory),
        FileType::RegularFile if s.st_nlink == 1 => Ok(MemberKind::File),
        _ => Err(conflict(name, "links and special nodes are unsupported")),
    }
}
pub(super) fn nonce() -> String {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |v| v.as_nanos());
    format!(
        "{}-{now:x}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    )
}
