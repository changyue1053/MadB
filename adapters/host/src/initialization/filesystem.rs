//! Small descriptor operations specific to one initialization attempt.
use super::{conflict, failure, host};
use madb_app::use_cases::initialize::AbsolutePath;
use madb_app::{Digest, ErrorKind, LogicalPath, Result, ports::FileReference};
use rustix::fs::{AtFlags, Dir, Mode, OFlags, openat};
use std::{collections::BTreeSet, fs::File, io::Read, os::unix::fs::MetadataExt};

pub(super) const PRIVATE: Mode = Mode::from_bits_retain(0o700);
pub(super) const FILE_MODE: Mode = Mode::from_bits_retain(0o600);
pub(super) type Identity = (u64, u64);

pub(super) fn identity(file: &File) -> Result<Identity> {
    let metadata = file.metadata().map_err(|e| host("identity", e))?;
    Ok((metadata.dev(), metadata.ino()))
}
pub(super) fn entry_identity(parent: &File, name: &str) -> Result<Identity> {
    let stat =
        rustix::fs::statat(parent, name, AtFlags::SYMLINK_NOFOLLOW).map_err(|e| host(name, e))?;
    Ok((stat.st_dev as u64, stat.st_ino))
}
pub(super) fn names(directory: &File) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for entry in Dir::read_from(directory).map_err(|e| host("directory", e))? {
        let entry = entry.map_err(|e| host("directory", e))?;
        let name = entry
            .file_name()
            .to_str()
            .map_err(|e| conflict("directory", e))?;
        if name != "." && name != ".." {
            names.push(name.to_owned());
        }
        if names.len() > 16384 {
            return Err(conflict("directory", "unexpected directory entry budget"));
        }
    }
    names.sort();
    Ok(names)
}
pub(super) fn directory(parent: &File, name: &str) -> Result<File> {
    let flags =
        OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC | OFlags::NONBLOCK;
    let fd = openat(parent, name, flags, Mode::empty()).map_err(|e| {
        let kind = if matches!(e, rustix::io::Errno::LOOP | rustix::io::Errno::NOTDIR) {
            ErrorKind::InvalidInput
        } else {
            ErrorKind::Host
        };
        failure(kind, name, e)
    })?;
    require_exact_name(parent, name)?;
    Ok(File::from(fd))
}
// Existing host parents are not part of the bounded initial project tree. Scan
// only for the requested spelling without collecting or limiting sibling names.
fn require_exact_name(parent: &File, name: &str) -> Result<()> {
    for entry in Dir::read_from(parent).map_err(|e| host(name, e))? {
        let entry = entry.map_err(|e| host(name, e))?;
        if entry.file_name().to_bytes() == name.as_bytes() {
            return Ok(());
        }
    }
    Err(conflict(name, "path spelling differs from actual entry"))
}
pub(super) fn absolute_directory(path: &str) -> Result<File> {
    let mut file = File::from(
        rustix::fs::open(
            "/",
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| host("/", e))?,
    );
    for part in path.split('/').filter(|part| !part.is_empty()) {
        file = directory(&file, part)?;
    }
    Ok(file)
}
pub(super) fn relative_directory(root: &File, path: &str) -> Result<File> {
    let mut current = root.try_clone().map_err(|e| host(path, e))?;
    for part in path.split('/').filter(|part| !part.is_empty()) {
        current = directory(&current, part)?;
    }
    Ok(current)
}
pub(super) fn parent_and_name(root: &File, path: &str) -> Result<(File, String)> {
    match path.rsplit_once('/') {
        Some((parent, name)) => Ok((relative_directory(root, parent)?, name.to_owned())),
        None => Ok((
            root.try_clone().map_err(|e| host(path, e))?,
            path.to_owned(),
        )),
    }
}

/// A lexical absolute root and the retained physical parent. No canonicalize
/// follows /tmp or any other symlink behind the caller's chosen spelling.
pub(super) struct ParentBinding {
    pub root: AbsolutePath,
    pub parent: File,
    parent_path: String,
    identity: Identity,
}
impl ParentBinding {
    pub fn open(root: &AbsolutePath) -> Result<Self> {
        let (parent_path, _) = root.as_str().rsplit_once('/').expect("absolute root");
        let parent_path = if parent_path.is_empty() {
            "/"
        } else {
            parent_path
        };
        let parent = absolute_directory(parent_path)?;
        Ok(Self {
            root: root.clone(),
            identity: identity(&parent)?,
            parent,
            parent_path: parent_path.to_owned(),
        })
    }
    pub fn verify(&self) -> Result<()> {
        let current = absolute_directory(&self.parent_path).map_err(|e| {
            conflict(
                self.root.as_str(),
                format!("frozen parent path changed: {e}"),
            )
        })?;
        if identity(&current)? != self.identity {
            return Err(conflict(
                self.root.as_str(),
                "frozen parent directory was replaced",
            ));
        }
        Ok(())
    }
}

pub(super) fn add_parents(path: &str, directories: &mut BTreeSet<String>) {
    for (index, _) in path.match_indices('/') {
        directories.insert(path[..index].to_owned());
    }
}
pub(super) fn read_reference(root: &File, expected: &FileReference) -> Result<FileReference> {
    let path = expected.path.as_str();
    let (parent, name) = parent_and_name(root, path)?;
    let mut file = File::from(
        openat(
            &parent,
            name.as_str(),
            OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC,
            Mode::empty(),
        )
        .map_err(|e| conflict(path, e))?,
    );
    let metadata = file.metadata().map_err(|e| host(path, e))?;
    if !metadata.is_file() || metadata.nlink() != 1 || metadata.len() != expected.size {
        return Err(conflict(
            path,
            "expected one-link regular file with frozen size",
        ));
    }
    let limit = usize::try_from(expected.size).map_err(|e| host(path, e))?;
    let mut bytes = Vec::new();
    file.by_ref()
        .take(expected.size.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(|e| host(path, e))?;
    if bytes.len() != limit
        || Digest::of_bytes(&bytes) != expected.digest
        || entry_identity(&parent, &name)? != identity(&file)?
    {
        return Err(conflict(path, "frozen bytes or file identity changed"));
    }
    require_exact_name(&parent, &name)?;
    Ok(FileReference {
        path: expected.path.clone(),
        digest: Digest::of_bytes(&bytes),
        size: bytes.len() as u64,
    })
}

pub(super) fn verify_tree(
    root: &File,
    files: &[FileReference],
    directories: &BTreeSet<String>,
) -> Result<Vec<FileReference>> {
    let file_paths: BTreeSet<_> = files.iter().map(|file| file.path.as_str()).collect();
    let mut seen = BTreeSet::new();
    inspect(root, "", &file_paths, directories, &mut seen)?;
    if seen.len() != files.len() + directories.len() {
        return Err(conflict(
            "inventory",
            "complete initial inventory differs from the frozen plan",
        ));
    }
    files
        .iter()
        .map(|file| read_reference(root, file))
        .collect()
}
fn inspect(
    root: &File,
    prefix: &str,
    files: &BTreeSet<&str>,
    directories: &BTreeSet<String>,
    seen: &mut BTreeSet<String>,
) -> Result<()> {
    for name in names(root)? {
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        if !seen.insert(path.clone()) {
            return Err(conflict(&path, "duplicate path"));
        }
        if directories.contains(&path) {
            inspect(&directory(root, &name)?, &path, files, directories, seen)?;
        } else if !files.contains(path.as_str()) {
            return Err(conflict(&path, "undeclared path in initial tree"));
        }
    }
    Ok(())
}
pub(super) fn logical(path: &str) -> Result<LogicalPath> {
    LogicalPath::new(path).map_err(|e| failure(ErrorKind::InvalidInput, path, e))
}
