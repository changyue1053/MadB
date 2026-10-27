//! Final-root execution for the finite installed initialization recipe. Tool
//! identities are supplied by the compiled installation record, never PATH.
//! This reuses C04 process admission/cleanup; it is not an OS sandbox.
mod installation;
mod runtime;
use crate::{LocalWorkspace, process::LocalProcessRunner};
pub use installation::{InstallationObserver, InstalledValuesTools};
use madb_app::{
    Digest, ErrorKind, LogicalPath, QueryError, Result,
    ports::{FileReference, ReadWorkspace},
    process::{
        ExecutionControl, ProcessError, ProcessExecution, ProcessPolicy, ProcessRequest,
        ProcessRun, execute_process_validated,
    },
    use_cases::initialize::{AbsolutePath, PreparedProject, ToolObservation},
};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct LockedInitializationFile {
    pub path: PathBuf,
    pub digest: Digest,
    pub size: u64,
}
impl LockedInitializationFile {
    pub fn verify(&self) -> Result<()> {
        if self.size > 256 * 1024 * 1024 {
            return Err(invalid(
                "installation",
                "locked file exceeds the explicit 256 MiB installation member budget",
            ));
        }
        let relative = absolute_relative(&self.path)?;
        let root = LocalWorkspace::open(Path::new("/"))?;
        let limit = 256 * 1024 * 1024;
        let bytes = root.read(&relative, limit)?.ok_or_else(|| {
            host(
                "installation",
                format!("locked file is missing: {}", self.path.display()),
            )
        })?;
        if bytes.len() as u64 != self.size || Digest::of_bytes(&bytes) != self.digest {
            return Err(invalid(
                "installation",
                format!("locked file differs: {}", self.path.display()),
            ));
        }
        Ok(())
    }
}
/// Freeze the generated ordinary consumer at its fixed final-root path. This
/// captures bytes; it does not certify generated code or permit PATH lookup.
pub fn capture_generated_executable(path: &Path) -> Result<LockedInitializationFile> {
    let source = LocalWorkspace::open(Path::new("/"))?;
    let bytes = source
        .read(&absolute_relative(path)?, 256 * 1024 * 1024)?
        .ok_or_else(|| host("consumer", "generated executable is absent"))?;
    let file = LockedInitializationFile {
        path: path.to_path_buf(),
        digest: Digest::of_bytes(&bytes),
        size: bytes.len() as u64,
    };
    file.verify()?;
    Ok(file)
}
#[derive(Clone, Debug)]
pub struct LockedInitializationTool {
    pub file: LockedInitializationFile,
    pub observation: ToolObservation,
}
impl LockedInitializationTool {
    pub fn revalidate(&self) -> Result<()> {
        self.verify().map_err(|e| conflict("tool", e))
    }
    pub fn verify(&self) -> Result<()> {
        if self.file.digest != self.observation.sha256 {
            return Err(invalid(
                "tool",
                "tool observation differs from its installation image",
            ));
        }
        self.file.verify()
    }
}

pub struct InitializationProcess<'a> {
    pub tool: &'a LockedInitializationTool,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub policy: ProcessPolicy,
}
/// Keeps the process owner even when a typed input/output validation fails.
pub struct InitializationProcessResult {
    pub execution: ProcessExecution,
    pub validation_error: Option<QueryError>,
}

/// Holding the final directory and its initial references prevents an ordinary
/// stage from silently adopting a different project root or author byte set.
pub struct FinalProjectExecution {
    root: PathBuf,
    native_relative: &'static str,
    directory: File,
    initial: Vec<FileReference>,
    runtime: std::cell::RefCell<Vec<FileReference>>,
}
impl FinalProjectExecution {
    #[cfg(unix)]
    pub fn open(project: &PreparedProject) -> Result<Self> {
        let root = PathBuf::from(project.root().as_str());
        let this = Self {
            directory: open_absolute_directory(&root)?,
            root,
            native_relative: ".madb/build/native",
            initial: project.references(),
            runtime: std::cell::RefCell::new(Vec::new()),
        };
        this.verify_initial()?;
        Ok(this)
    }
    #[cfg(not(unix))]
    pub fn open(_: &PreparedProject) -> Result<Self> {
        Err(invalid(
            "platform",
            "initialization execution requires a Unix host",
        ))
    }
    /// Open an already-owned project-build attempt; its Host owner created the
    /// native directory and supplies the complete captured input identities.
    #[cfg(unix)]
    pub fn open_frozen(root: &AbsolutePath, initial: Vec<FileReference>) -> Result<Self> {
        let root = PathBuf::from(root.as_str());
        let this = Self {
            directory: open_absolute_directory(&root)?,
            root,
            native_relative: "native",
            initial,
            runtime: std::cell::RefCell::new(Vec::new()),
        };
        this.verify_initial()?;
        child_directory(&this.directory, "native", false)?;
        Ok(this)
    }
    #[cfg(not(unix))]
    pub fn open_frozen(_: &AbsolutePath, _: Vec<FileReference>) -> Result<Self> {
        Err(invalid(
            "platform",
            "project execution requires a Unix host",
        ))
    }
    pub fn materialize_project_runtime(&self) -> Result<()> {
        runtime::materialize_project(self)
    }
    pub fn root(&self) -> &Path {
        &self.root
    }
    pub fn materialize_runtime(&self) -> Result<()> {
        runtime::materialize(self)
    }
    pub fn native_root(&self) -> PathBuf {
        self.root.join(self.native_relative)
    }
    pub fn verify_initial(&self) -> Result<()> {
        self.verify_root()?;
        let source = LocalWorkspace {
            root: self.directory.try_clone().map_err(|e| host("project", e))?,
        };
        for reference in self.initial.iter().chain(self.runtime.borrow().iter()) {
            let bytes = source
                .read(&reference.path, reference.size as usize)?
                .ok_or_else(|| conflict(reference.path.as_str(), "initial file is missing"))?;
            if bytes.len() as u64 != reference.size || Digest::of_bytes(&bytes) != reference.digest
            {
                return Err(conflict(
                    reference.path.as_str(),
                    "initial project input changed during initialization",
                ));
            }
        }
        self.verify_root()
    }
    #[cfg(unix)]
    fn verify_root(&self) -> Result<()> {
        use std::os::unix::fs::MetadataExt;
        let held = self.directory.metadata().map_err(|e| host("project", e))?;
        let current = open_absolute_directory(&self.root)?
            .metadata()
            .map_err(|e| host("project", e))?;
        if held.dev() != current.dev() || held.ino() != current.ino() {
            return Err(conflict(
                "project",
                "final project directory changed during initialization",
            ));
        }
        Ok(())
    }
    #[cfg(not(unix))]
    fn verify_root(&self) -> Result<()> {
        Err(invalid(
            "platform",
            "unsupported initialization execution host",
        ))
    }

    /// The native execution domain is created once, without replacing any
    /// user path. Inputs and generated state remain under this final root.
    #[cfg(unix)]
    pub fn create_native_root(&self) -> Result<()> {
        use rustix::fs::{Mode, mkdirat};
        self.verify_initial()?;
        let parent = child_directory(&self.directory, ".madb/build", true)?;
        mkdirat(&parent, "native", Mode::from_bits_retain(0o700))
            .map_err(|e| conflict(".madb/build/native", e))?;
        self.verify_root()
    }
    #[cfg(not(unix))]
    pub fn create_native_root(&self) -> Result<()> {
        self.verify_root()
    }

    /// Materialize only caller-selected immutable installation bytes. This
    /// never overwrites a source, generated output, or previously copied file.
    #[cfg(unix)]
    pub fn write_native_new(&self, path: &LogicalPath, bytes: &[u8]) -> Result<FileReference> {
        use rustix::fs::{Mode, OFlags, openat};
        self.verify_root()?;
        let native = child_directory(&self.directory, self.native_relative, false)?;
        let (parent, name) = match path.as_str().rsplit_once('/') {
            Some((parent, name)) => (child_directory(&native, parent, true)?, name),
            None => (native, path.as_str()),
        };
        let mut file = File::from(
            openat(
                &parent,
                name,
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                Mode::from_bits_retain(0o600),
            )
            .map_err(|e| conflict(path.as_str(), e))?,
        );
        file.write_all(bytes).map_err(|e| host(path.as_str(), e))?;
        file.sync_all().map_err(|e| host(path.as_str(), e))?;
        self.verify_root()?;
        let reference = FileReference {
            path: LogicalPath::new(format!("{}/{}", self.native_relative, path.as_str()))
                .map_err(|e| invalid("path", e.to_string()))?,
            digest: Digest::of_bytes(bytes),
            size: bytes.len() as u64,
        };
        let actual = self.read(&reference)?;
        if actual != bytes {
            return Err(conflict(path.as_str(), "written runtime input differs"));
        }
        self.runtime.borrow_mut().push(reference.clone());
        Ok(reference)
    }
    #[cfg(not(unix))]
    pub fn write_native_new(&self, _: &LogicalPath, _: &[u8]) -> Result<FileReference> {
        Err(invalid(
            "platform",
            "unsupported initialization execution host",
        ))
    }

    pub fn read(&self, reference: &FileReference) -> Result<Vec<u8>> {
        self.verify_root()?;
        let source = LocalWorkspace {
            root: self.directory.try_clone().map_err(|e| host("project", e))?,
        };
        let bytes = source
            .read(&reference.path, reference.size as usize)?
            .ok_or_else(|| conflict(reference.path.as_str(), "expected output is missing"))?;
        if bytes.len() as u64 != reference.size || Digest::of_bytes(&bytes) != reference.digest {
            return Err(conflict(
                reference.path.as_str(),
                "output differs from its validated identity",
            ));
        }
        self.verify_root()?;
        Ok(bytes)
    }
    /// Capture a fixed generated recipe output once, then include its exact
    /// bytes in every later input revalidation. The caller chooses fixed paths.
    pub fn freeze_generated(
        &self,
        path: &LogicalPath,
        limit: usize,
    ) -> Result<(LockedInitializationFile, Vec<u8>)> {
        if limit > 256 * 1024 * 1024 {
            return Err(invalid(
                "generated",
                "generated file budget exceeds installation maximum",
            ));
        }
        self.verify_initial()?;
        let source = LocalWorkspace {
            root: self.directory.try_clone().map_err(|e| host("project", e))?,
        };
        let bytes = source
            .read(path, limit)?
            .ok_or_else(|| host(path.as_str(), "generated output is missing"))?;
        let reference = FileReference {
            path: path.clone(),
            digest: Digest::of_bytes(&bytes),
            size: bytes.len() as u64,
        };
        self.runtime.borrow_mut().push(reference.clone());
        self.verify_initial()?;
        Ok((
            LockedInitializationFile {
                path: self.root.join(path.as_str()),
                digest: reference.digest,
                size: reference.size,
            },
            bytes,
        ))
    }

    /// C04 timeout bounds process supervision; synchronous trusted local-file
    /// validation remains inside cancellation admission, not a hard wall-clock
    /// bound on filesystem calls. This does not claim an OS sandbox.
    pub fn run(
        &self,
        process: InitializationProcess<'_>,
        control: &ExecutionControl,
        validate: impl FnOnce(&ProcessRun) -> Result<()>,
    ) -> Result<InitializationProcessResult> {
        self.verify_initial()?;
        process.tool.revalidate()?;
        let cwd = absolute_relative(&self.root)?;
        let request = ProcessRequest {
            tool: FileReference {
                path: absolute_relative(&process.tool.file.path)?,
                digest: process.tool.file.digest.clone(),
                size: process.tool.file.size,
            },
            working_directory: cwd,
            arguments: process.arguments,
            environment: process.environment,
            stdin: Vec::new(),
        };
        let runner = LocalProcessRunner::new(Path::new("/"), Path::new("/"), process.policy)
            .map_err(process_error)?;
        let mut validation_error = None;
        let executed = execute_process_validated(&runner, &request, control, |run, _| {
            let result = self
                .verify_initial()
                .and_then(|()| process.tool.revalidate())
                .and_then(|()| validate(run));
            if let Err(error) = result {
                let diagnostic = ProcessError::new("EXEC010", "validation", error.to_string());
                validation_error = Some(error);
                Err(diagnostic)
            } else {
                Ok(())
            }
        })
        .map_err(process_error)?;
        Ok(InitializationProcessResult {
            execution: executed.execution,
            validation_error,
        })
    }
}
fn process_error(error: madb_app::process::ProcessError) -> QueryError {
    QueryError::new(ErrorKind::Host, "EXEC010", error.stage, error.to_string())
}
fn absolute_relative(path: &Path) -> Result<LogicalPath> {
    let value = path
        .to_str()
        .ok_or_else(|| invalid("path", "installation path is not UTF-8"))?;
    let relative = value
        .strip_prefix('/')
        .ok_or_else(|| invalid("path", "installation path is not absolute"))?;
    LogicalPath::new(relative).map_err(|e| invalid("path", e.to_string()))
}
#[cfg(unix)]
fn open_absolute_directory(path: &Path) -> Result<File> {
    let logical = absolute_relative(path)?;
    let root = LocalWorkspace::open(Path::new("/"))?;
    child_directory(&root.root, logical.as_str(), false)
}
#[cfg(unix)]
fn child_directory(root: &File, path: &str, create: bool) -> Result<File> {
    use rustix::fs::{Mode, OFlags, mkdirat, openat};
    let path = LogicalPath::new(path).map_err(|e| invalid("path", e.to_string()))?;
    let mut directory = root.try_clone().map_err(|e| host("directory", e))?;
    for name in path.as_str().split('/') {
        let flags = OFlags::RDONLY
            | OFlags::DIRECTORY
            | OFlags::NOFOLLOW
            | OFlags::CLOEXEC
            | OFlags::NONBLOCK;
        let next = match openat(&directory, name, flags, Mode::empty()) {
            Ok(fd) => fd,
            Err(rustix::io::Errno::NOENT) if create => {
                mkdirat(&directory, name, Mode::from_bits_retain(0o700))
                    .map_err(|e| conflict(path.as_str(), e))?;
                openat(&directory, name, flags, Mode::empty())
                    .map_err(|e| conflict(path.as_str(), e))?
            }
            Err(e) => return Err(conflict(path.as_str(), e)),
        };
        directory = next.into();
    }
    Ok(directory)
}
fn host(subject: &str, error: impl std::fmt::Display) -> QueryError {
    QueryError::new(ErrorKind::Host, "HOST005", subject, error.to_string())
}
fn invalid(subject: &str, error: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "PLAN001", subject, error)
}
fn conflict(subject: &str, error: impl std::fmt::Display) -> QueryError {
    QueryError::new(ErrorKind::Conflict, "FILE001", subject, error.to_string())
}

#[cfg(all(test, unix))]
mod tests;
