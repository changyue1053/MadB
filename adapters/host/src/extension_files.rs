//! Private filesystem domains for extension sessions. No protocol or semantic
//! decisions live here. Only frozen bytes leave a successful host admission;
//! retained paths remain diagnostic locations, never downstream inputs.
use crate::{LocalWorkspace, inputs::PreparationError};
use madb_app::{ErrorKind, LogicalPath, QueryError, Result, inputs::FrozenInputs};
use std::path::{Path, PathBuf};
#[cfg(unix)]
use {
    rustix::fs::{Dir, Mode, OFlags, fchmod, mkdirat, openat},
    std::{fs::File, io::Write},
};

/// Exclusive retained ownership. Drop closes handles, without deleting evidence.
/// Read-only input modes deter accidental writes; this is not a same-user sandbox.
#[must_use]
pub struct LocalExtensionWorkspace {
    path: PathBuf,
    work_path: PathBuf,
    output_path: PathBuf,
    output_root: LogicalPath,
    snapshot: FrozenInputs,
    #[cfg(unix)]
    root: File,
    #[cfg(unix)]
    work: File,
    #[cfg(unix)]
    inputs: LocalWorkspace,
    #[cfg(unix)]
    outputs: LocalWorkspace,
}
impl LocalExtensionWorkspace {
    pub fn create(
        parent: &Path,
        name: &LogicalPath,
        snapshot: FrozenInputs,
        output_root: &LogicalPath,
    ) -> std::result::Result<Self, PreparationError> {
        Self::allocate(parent, name, snapshot, output_root)
    }
    #[cfg(unix)]
    fn allocate(
        parent: &Path,
        name: &LogicalPath,
        snapshot: FrozenInputs,
        output_root: &LogicalPath,
    ) -> std::result::Result<Self, PreparationError> {
        let parts: Vec<_> = output_root.as_str().split('/').collect();
        if parts.len() < 2 || parts[0] != "outputs" || !output_root.as_str().is_ascii() {
            return Err(PreparationError {
                error: invalid("output_root", "expected outputs/request domain"),
                retained_path: None,
            });
        }
        let (path, root) = crate::inputs::allocate_directory(parent, name)?;
        let retained = |error| PreparationError {
            error,
            retained_path: Some(path.clone()),
        };
        mkdir(&root, "work").map_err(retained)?;
        let work = directory(&root, "work").map_err(retained)?;
        mkdir(&work, "inputs").map_err(retained)?;
        let input_dir = directory(&work, "inputs").map_err(retained)?;
        materialize(&input_dir, &snapshot).map_err(retained)?;
        let mut parent_dir = work.try_clone().map_err(|e| retained(invalid("work", e)))?;
        for part in parts {
            mkdir(&parent_dir, part).map_err(retained)?;
            parent_dir = directory(&parent_dir, part).map_err(retained)?;
        }
        let work_path = path.join("work");
        let prepared = Self {
            output_path: work_path.join(output_root.as_str()),
            path,
            work_path,
            output_root: output_root.clone(),
            snapshot,
            root,
            work,
            inputs: LocalWorkspace { root: input_dir },
            outputs: LocalWorkspace { root: parent_dir },
        };
        prepared.verify_inputs().map_err(|error| PreparationError {
            error,
            retained_path: Some(prepared.path.clone()),
        })?;
        Ok(prepared)
    }
    #[cfg(not(unix))]
    fn allocate(
        _: &Path,
        _: &LogicalPath,
        _: FrozenInputs,
        _: &LogicalPath,
    ) -> std::result::Result<Self, PreparationError> {
        Err(PreparationError {
            error: invalid("platform", "extension file domains require a Unix host"),
            retained_path: None,
        })
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn work_path(&self) -> &Path {
        &self.work_path
    }
    pub fn output_path(&self) -> &Path {
        &self.output_path
    }
    pub fn output_root(&self) -> &LogicalPath {
        &self.output_root
    }

    #[cfg(unix)]
    pub fn output_workspace(&self) -> &LocalWorkspace {
        &self.outputs
    }

    /// Check before launch and after a clean terminal observation. This includes
    /// exact spelling, undeclared siblings, root replacement and input mutations.
    #[cfg(unix)]
    pub fn verify_inputs(&self) -> Result<()> {
        crate::inputs::verify_directory(&self.path, &self.root)?;
        crate::inputs::verify_directory(&self.work_path, &self.work)?;
        crate::inputs::verify_directory(&self.work_path.join("inputs"), &self.inputs.root)?;
        crate::inputs::verify_directory(&self.output_path, &self.outputs.root)?;
        names(&self.root, &["work"])?;
        names(&self.work, &["inputs", "outputs"])?;
        let parts: Vec<_> = self.output_root.as_str().split('/').collect();
        let mut current = self.work.try_clone().map_err(|e| invalid("work", e))?;
        for (index, part) in parts.iter().enumerate() {
            current = directory(&current, part)?;
            if let Some(next) = parts.get(index + 1) {
                names(&current, &[next])?;
            }
        }
        // Bind the traversed domain to its originally opened directory too.
        use std::os::unix::fs::MetadataExt;
        let named = current.metadata().map_err(|e| invalid("output", e))?;
        let opened = self
            .outputs
            .root
            .metadata()
            .map_err(|e| invalid("output", e))?;
        if named.dev() != opened.dev() || named.ino() != opened.ino() {
            return Err(invalid("output", "output domain replaced"));
        }
        crate::inputs::tree::verify_contents(&self.inputs, &self.snapshot)
    }
    #[cfg(not(unix))]
    pub fn verify_inputs(&self) -> Result<()> {
        let _ = &self.snapshot;
        Err(invalid(
            "platform",
            "extension file domains require a Unix host",
        ))
    }

    /// Invoke after semantic capture, still inside cancellation admission. Exact
    /// inventory checking catches unlisted files, aliases, links and substitutions.
    #[cfg(unix)]
    pub fn verify_outputs(&self, files: &FrozenInputs) -> Result<()> {
        self.verify_inputs()?;
        crate::inputs::tree::verify_contents(&self.outputs, files)?;
        self.verify_inputs()
    }
    #[cfg(not(unix))]
    pub fn verify_outputs(&self, _: &FrozenInputs) -> Result<()> {
        self.verify_inputs()
    }
}
#[cfg(unix)]
fn directory(root: &File, path: &str) -> Result<File> {
    let mut opened = root.try_clone().map_err(|e| invalid(path, e))?;
    for part in path.split('/') {
        opened = File::from(
            openat(
                &opened,
                part,
                OFlags::RDONLY
                    | OFlags::DIRECTORY
                    | OFlags::CLOEXEC
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK,
                Mode::empty(),
            )
            .map_err(|e| invalid(path, e))?,
        );
    }
    Ok(opened)
}
#[cfg(unix)]
fn mkdir(root: &File, name: &str) -> Result<()> {
    mkdirat(root, name, Mode::from_bits_retain(0o700)).map_err(|e| invalid(name, e))
}
#[cfg(unix)]
fn parent(root: &File, path: &str) -> Result<(File, String)> {
    match path.rsplit_once('/') {
        Some((parent, name)) => Ok((directory(root, parent)?, name.into())),
        None => Ok((root.try_clone().map_err(|e| invalid(path, e))?, path.into())),
    }
}
#[cfg(unix)]
fn materialize(root: &File, snapshot: &FrozenInputs) -> Result<()> {
    for path in snapshot.directories() {
        let (dir, name) = parent(root, path.as_str())?;
        mkdir(&dir, &name)?;
    }
    for (reference, bytes) in snapshot.files() {
        let (dir, name) = parent(root, reference.path.as_str())?;
        let mut file = File::from(
            openat(
                &dir,
                name.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
                Mode::from_bits_retain(0o600),
            )
            .map_err(|e| invalid(reference.path.as_str(), e))?,
        );
        file.write_all(bytes)
            .map_err(|e| invalid(reference.path.as_str(), e))?;
        fchmod(&file, Mode::from_bits_retain(0o400))
            .map_err(|e| invalid(reference.path.as_str(), e))?;
    }
    let directories: Vec<_> = snapshot.directories().collect();
    for path in directories.into_iter().rev() {
        fchmod(
            directory(root, path.as_str())?,
            Mode::from_bits_retain(0o500),
        )
        .map_err(|e| invalid(path.as_str(), e))?;
    }
    fchmod(root, Mode::from_bits_retain(0o500)).map_err(|e| invalid("inputs", e))?;
    Ok(())
}
#[cfg(unix)]
fn names(directory: &File, expected: &[&str]) -> Result<()> {
    let mut actual = std::collections::BTreeSet::new();
    for entry in Dir::read_from(directory).map_err(|e| invalid("directory", e))? {
        let entry = entry.map_err(|e| invalid("directory", e))?;
        let name = entry
            .file_name()
            .to_str()
            .map_err(|e| invalid("directory", e))?;
        if name != "."
            && name != ".."
            && (!expected.contains(&name) || !actual.insert(name.to_owned()))
        {
            return Err(invalid(
                "directory",
                "undeclared entry or path spelling mismatch",
            ));
        }
    }
    if actual != expected.iter().map(|s| (*s).to_owned()).collect() {
        return Err(invalid(
            "directory",
            "undeclared entry or path spelling mismatch",
        ));
    }
    Ok(())
}
fn invalid(subject: &str, error: impl std::fmt::Display) -> QueryError {
    QueryError::new(
        ErrorKind::Conflict,
        "OutputInvalid",
        subject,
        error.to_string(),
    )
}
