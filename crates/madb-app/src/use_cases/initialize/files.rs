use super::{AbsolutePath, FrozenInitRequest, invalid};
use crate::ports::FileReference;
use crate::{Digest, ErrorKind, Id, LogicalPath, QueryError, Result};
use madb_plan::project::{FileIdentity, FileOwnership, ProjectPlan};
use std::sync::Arc;

/// Bytes are held independently of the caller's mutable source. Local-state is
/// a reserved file path and has no content, hash, or create operation.
#[derive(Clone, Debug)]
pub struct InitialFile {
    module: Id,
    path: LogicalPath,
    ownership: FileOwnership,
    bytes: Option<Arc<[u8]>>,
}
impl InitialFile {
    pub fn rendered(
        module: Id,
        path: LogicalPath,
        ownership: FileOwnership,
        bytes: Vec<u8>,
    ) -> Result<Self> {
        if ownership == FileOwnership::LocalState {
            return Err(invalid(
                path.as_str(),
                "local-state must not contain placeholder bytes",
            ));
        }
        Ok(Self {
            module,
            path,
            ownership,
            bytes: Some(bytes.into()),
        })
    }
    pub fn local_state(module: Id, path: LogicalPath) -> Self {
        Self {
            module,
            path,
            ownership: FileOwnership::LocalState,
            bytes: None,
        }
    }
    pub fn module(&self) -> &Id {
        &self.module
    }
    pub fn path(&self) -> &LogicalPath {
        &self.path
    }
    pub fn ownership(&self) -> FileOwnership {
        self.ownership
    }
    pub fn bytes(&self) -> Option<&[u8]> {
        self.bytes.as_deref()
    }
    pub fn reference(&self) -> Option<FileReference> {
        self.bytes().map(|bytes| FileReference {
            path: self.path.clone(),
            digest: Digest::of_bytes(bytes),
            size: bytes.len() as u64,
        })
    }
}
#[derive(Clone, Debug)]
pub struct PreparedProject {
    root: AbsolutePath,
    plan: ProjectPlan,
    files: Vec<InitialFile>,
    request: FrozenInitRequest,
    manifest: FileIdentity,
    package: FileIdentity,
}
impl PreparedProject {
    pub fn new(
        request: &FrozenInitRequest,
        manifest: FileIdentity,
        package: FileIdentity,
        mut files: Vec<InitialFile>,
    ) -> Result<Self> {
        let root = request.projection().output.clone();
        let plan = request
            .selected()
            .plan_with_provenance(&manifest)
            .map_err(|e| invalid(e.subject, e.message))?;
        files.sort_by(|a, b| a.path.cmp(&b.path));
        if files.len() > 4096 || files.len() != plan.files().len() {
            return Err(invalid(
                "plan",
                "rendered initial inventory differs from the complete plan or exceeds the result budget",
            ));
        }
        for (file, expected) in files.iter().zip(plan.files()) {
            if file.module != expected.module
                || file.path != expected.path
                || file.ownership != expected.ownership
            {
                return Err(invalid(
                    file.path.as_str(),
                    "rendered file identity/owner differs from the initial plan",
                ));
            }
        }
        let mut provenance = request.selected().template().files.clone();
        provenance.push(manifest.clone());
        for input in provenance {
            let output = format!(".madb/template-input/{}", input.path.as_str());
            let actual = files
                .iter()
                .find(|file| file.path.as_str() == output)
                .and_then(InitialFile::reference)
                .ok_or_else(|| invalid(&output, "missing initial provenance bytes"))?;
            if actual.digest != input.sha256 || actual.size != input.size {
                return Err(invalid(
                    output,
                    "initial provenance differs from frozen package inputs",
                ));
            }
        }
        let archive = files
            .iter()
            .find(|file| file.path.as_str() == ".madb/template-package.tar")
            .and_then(InitialFile::reference)
            .ok_or_else(|| invalid("package", "missing original package provenance bytes"))?;
        if archive.digest != package.sha256 || archive.size != package.size {
            return Err(invalid(
                "package",
                "original package provenance bytes differ",
            ));
        }
        Ok(Self {
            root,
            plan,
            files,
            request: request.clone(),
            manifest,
            package,
        })
    }
    pub fn root(&self) -> &AbsolutePath {
        &self.root
    }
    pub fn request(&self) -> &FrozenInitRequest {
        &self.request
    }
    pub fn manifest(&self) -> &FileIdentity {
        &self.manifest
    }
    pub fn package(&self) -> &FileIdentity {
        &self.package
    }
    pub fn plan(&self) -> &ProjectPlan {
        &self.plan
    }
    pub fn files(&self) -> &[InitialFile] {
        &self.files
    }
    pub fn references(&self) -> Vec<FileReference> {
        self.files
            .iter()
            .filter_map(InitialFile::reference)
            .collect()
    }
}
/// Err means no user directory was created. Once the atomic create succeeds,
/// Host must return Ok even when inventory verification fails: that failure is
/// carried by `files`, preserving the irreversible creation observation.
pub trait CommitNewPort {
    fn commit_new(&mut self, project: &PreparedProject) -> Result<HostCommitObservation>;
}
#[derive(Debug)]
pub struct HostCommitObservation {
    pub root: AbsolutePath,
    pub files: Result<Vec<FileReference>>,
}
/// This proves only that this trusted Host invocation created a directory. It
/// does not prove the intended root or inventory and cannot authorize success.
#[derive(Clone, Debug)]
pub struct CreatedProject {
    request: FrozenInitRequest,
    root: AbsolutePath,
}
impl CreatedProject {
    pub fn root(&self) -> &AbsolutePath {
        &self.root
    }
    pub(super) fn request(&self) -> &FrozenInitRequest {
        &self.request
    }
}
#[derive(Debug)]
pub struct CommitAttempt {
    created: CreatedProject,
    validation: Result<CommittedProject>,
}
impl CommitAttempt {
    pub fn into_parts(self) -> (CreatedProject, Result<CommittedProject>) {
        (self.created, self.validation)
    }
    pub fn created(&self) -> &CreatedProject {
        &self.created
    }
    pub fn validated(&self) -> std::result::Result<&CommittedProject, &QueryError> {
        self.validation.as_ref()
    }
}
/// Only commit_new can create this opaque request/root/plan-bound receipt.
#[derive(Debug)]
pub struct CommittedProject {
    created: CreatedProject,
    plan: ProjectPlan,
    files: Vec<FileReference>,
}
impl CommittedProject {
    pub fn created(&self) -> &CreatedProject {
        &self.created
    }
    pub fn root(&self) -> &AbsolutePath {
        self.created.root()
    }
    pub(super) fn matches(&self, request: &FrozenInitRequest, project: &PreparedProject) -> bool {
        self.created.request() == request
            && self.created.root == *project.root()
            && self.plan == *project.plan()
            && self.files == project.references()
    }
}
pub fn commit_new<P: CommitNewPort>(
    port: &mut P,
    request: &FrozenInitRequest,
    project: &PreparedProject,
) -> Result<CommitAttempt> {
    if request.projection().dry_run {
        return Err(invalid(
            "commit",
            "dry-run cannot request a user project commit",
        ));
    }
    if request != project.request() || &request.projection().output != project.root() {
        return Err(invalid("commit", "project differs from the frozen request"));
    }
    let observed = port.commit_new(project)?;
    let created = CreatedProject {
        request: request.clone(),
        root: observed.root,
    };
    let validation = observed.files.and_then(|mut files| {
        files.sort_by(|a, b| a.path.cmp(&b.path));
        if created.root != *project.root() || files != project.references() {
            return Err(QueryError::new(
                ErrorKind::Host,
                "PORT001",
                "commit",
                "Host committed a directory but its root or complete initial inventory differs",
            ));
        }
        Ok(CommittedProject {
            created: created.clone(),
            plan: project.plan.clone(),
            files,
        })
    });
    Ok(CommitAttempt {
        created,
        validation,
    })
}
