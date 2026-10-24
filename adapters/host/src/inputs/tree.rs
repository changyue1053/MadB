//! Descriptor-relative creation and verification of the bounded input tree.
use super::preparation_error;
use crate::LocalWorkspace;
use madb_app::{ErrorKind, QueryError, Result, inputs::FrozenInputs};
use rustix::fs::{Dir, Mode, OFlags, fchmod, mkdirat, openat};
use std::{collections::BTreeSet, fs::File, io::Write};

pub(super) const PRIVATE_DIR: Mode = Mode::from_bits_retain(0o700);
const INPUT_DIR: Mode = Mode::from_bits_retain(0o500);
const INPUT_FILE: Mode = Mode::from_bits_retain(0o400);

pub(super) fn open_directory(root: &File, path: &str) -> Result<File> {
    let mut directory = root.try_clone().map_err(|e| preparation_error(path, e))?;
    for part in path.split('/') {
        directory = File::from(
            openat(
                &directory,
                part,
                OFlags::RDONLY
                    | OFlags::DIRECTORY
                    | OFlags::CLOEXEC
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(|e| preparation_error(path, e))?,
        );
    }
    Ok(directory)
}

fn parent_and_name(root: &File, path: &str) -> Result<(File, String)> {
    match path.rsplit_once('/') {
        Some((parent, name)) => Ok((open_directory(root, parent)?, name.to_owned())),
        None => Ok((
            root.try_clone().map_err(|e| preparation_error(path, e))?,
            path.to_owned(),
        )),
    }
}

pub(super) fn materialize(root: &File, snapshot: &FrozenInputs) -> Result<()> {
    for name in ["inputs", "work"] {
        mkdirat(root, name, PRIVATE_DIR).map_err(|e| preparation_error(name, e))?;
    }
    let inputs = open_directory(root, "inputs")?;
    let directories: Vec<_> = snapshot.directories().collect();
    for path in &directories {
        let (parent, name) = parent_and_name(&inputs, path.as_str())?;
        mkdirat(&parent, name.as_str(), PRIVATE_DIR)
            .map_err(|e| preparation_error(path.as_str(), e))?;
    }
    for (reference, bytes) in snapshot.files() {
        let path = reference.path.as_str();
        let (parent, name) = parent_and_name(&inputs, path)?;
        let mut file = File::from(
            openat(
                &parent,
                name.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::from_bits_retain(0o600),
            )
            .map_err(|e| preparation_error(path, e))?,
        );
        file.write_all(bytes)
            .map_err(|e| preparation_error(path, e))?;
        fchmod(&file, INPUT_FILE).map_err(|e| preparation_error(path, e))?;
    }
    for path in directories.into_iter().rev() {
        let directory = open_directory(&inputs, path.as_str())?;
        fchmod(&directory, INPUT_DIR).map_err(|e| preparation_error(path.as_str(), e))?;
    }
    fchmod(&inputs, INPUT_DIR).map_err(|e| preparation_error("inputs", e))?;
    Ok(())
}

pub(super) fn verify(root: &File, snapshot: &FrozenInputs) -> Result<()> {
    let inputs = open_directory(root, "inputs")?;
    verify_contents(&LocalWorkspace { root: inputs }, snapshot)
}

pub(crate) fn verify_contents(workspace: &LocalWorkspace, snapshot: &FrozenInputs) -> Result<()> {
    let files: BTreeSet<_> = snapshot.references().map(|r| r.path.as_str()).collect();
    let directories: BTreeSet<_> = snapshot.directories().map(|p| p.as_str()).collect();
    let mut seen = BTreeSet::new();
    inspect(&workspace.root, "", &files, &directories, &mut seen)?;
    if seen.len() != files.len() + directories.len() {
        return Err(integrity_error(
            "files",
            "declared path missing from private input tree",
        ));
    }
    snapshot.verify(workspace)
}

fn inspect(
    directory: &File,
    prefix: &str,
    files: &BTreeSet<&str>,
    directories: &BTreeSet<&str>,
    seen: &mut BTreeSet<String>,
) -> Result<()> {
    let entries = Dir::read_from(directory).map_err(|e| integrity_error(prefix, e))?;
    for entry in entries {
        let entry = entry.map_err(|e| integrity_error(prefix, e))?;
        let name = entry
            .file_name()
            .to_str()
            .map_err(|e| integrity_error(prefix, e))?;
        if name == "." || name == ".." {
            continue;
        }
        let path = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}/{name}")
        };
        let is_directory = directories.contains(path.as_str());
        if (!is_directory && !files.contains(path.as_str())) || !seen.insert(path.clone()) {
            return Err(integrity_error(&path, "undeclared or duplicate input path"));
        }
        if is_directory {
            let child = open_directory(directory, name)?;
            inspect(&child, &path, files, directories, seen)?;
        }
    }
    Ok(())
}

pub(super) fn integrity_error(subject: &str, error: impl std::fmt::Display) -> QueryError {
    QueryError::new(ErrorKind::Conflict, "INPUT006", subject, error.to_string())
}
