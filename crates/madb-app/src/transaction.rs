//! Immutable three-layer batch identities and commit admission. Filesystem
//! durability, inter-process locks and leases belong to the host adapter.
use crate::{
    Digest, ErrorKind, QueryError, Result, inputs::FrozenInputs, ports::FileReference,
    process::CancelDecision,
};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Layer {
    Source,
    Build,
    Candidate,
}
impl Layer {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Source => "source",
            Self::Build => "build",
            Self::Candidate => "candidate",
        }
    }
}

/// Storage lineage, not an SDK certification. The application must validate the
/// corresponding generation/build/package payloads before preparing this value.
/// input_digest is source lineage, not the digest of generated build inputs.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchSpec {
    pub layer: Layer,
    pub input_digest: Digest,
    pub task_digest: Digest,
    pub profile_digest: Digest,
    pub generation_id: Digest,
    /// A fixed *physical instance* identity, never a mutable current alias.
    pub parent: Option<Digest>,
}
#[derive(Clone, Debug)]
pub struct Batch {
    id: Digest,
    spec: BatchSpec,
    files: FrozenInputs,
}
impl Batch {
    pub fn new(spec: BatchSpec, files: FrozenInputs) -> Result<Self> {
        if (spec.layer == Layer::Source) != spec.parent.is_none() {
            return Err(error(
                "STORE001",
                "parent",
                "source has no parent; build/candidate require one",
            ));
        }
        let mut encoded = b"madb.storage-content/1\0".to_vec();
        append(&mut encoded, spec.layer.as_str().as_bytes());
        for digest in [
            &spec.input_digest,
            &spec.task_digest,
            &spec.profile_digest,
            &spec.generation_id,
        ] {
            append(&mut encoded, digest.as_str().as_bytes());
        }
        append(
            &mut encoded,
            spec.parent
                .as_ref()
                .map_or(b"".as_slice(), |id| id.as_str().as_bytes()),
        );
        for reference in files.references() {
            append(&mut encoded, reference.path.as_str().as_bytes());
            append(&mut encoded, reference.digest.as_str().as_bytes());
            encoded.extend_from_slice(&reference.size.to_le_bytes());
        }
        Ok(Self {
            id: Digest::of_bytes(&encoded),
            spec,
            files,
        })
    }
    /// Deterministic content identity. Store index references are separate
    /// physical instance IDs so rebuilding never mutates a leased directory.
    pub fn id(&self) -> &Digest {
        &self.id
    }
    pub fn spec(&self) -> &BatchSpec {
        &self.spec
    }
    pub fn files(&self) -> &FrozenInputs {
        &self.files
    }
}
fn append(target: &mut Vec<u8>, value: &[u8]) {
    target.extend_from_slice(&(value.len() as u64).to_le_bytes());
    target.extend_from_slice(value);
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BatchInfo {
    pub id: Digest,
    pub content_id: Digest,
    pub spec: BatchSpec,
    pub files: Vec<FileReference>,
}
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct IndexSnapshot {
    pub revision: u64,
    pub source: Option<Digest>,
    pub build: Option<Digest>,
    pub candidate: Option<Digest>,
}
impl IndexSnapshot {
    pub fn get(&self, layer: Layer) -> Option<&Digest> {
        match layer {
            Layer::Source => self.source.as_ref(),
            Layer::Build => self.build.as_ref(),
            Layer::Candidate => self.candidate.as_ref(),
        }
    }
    pub fn advance(&self, layer: Layer, id: Digest) -> Result<Self> {
        let mut next = self.clone();
        next.revision = self
            .revision
            .checked_add(1)
            .ok_or_else(|| error("STORE007", "revision", "store revision exhausted"))?;
        match layer {
            Layer::Source => {
                next.source = Some(id);
                next.build = None;
                next.candidate = None;
            }
            Layer::Build => {
                next.build = Some(id);
                next.candidate = None;
            }
            Layer::Candidate => next.candidate = Some(id),
        }
        Ok(next)
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommitDisposition {
    Committed,
    Reused,
}
#[derive(Clone, Debug)]
pub struct CommitReceipt {
    pub disposition: CommitDisposition,
    pub index: IndexSnapshot,
    pub batch_id: Digest,
}
/// The fixed bytes and provenance of one pinned physical batch.
pub trait BatchLease {
    fn info(&self) -> &BatchInfo;
    fn files(&self) -> &FrozenInputs;
}
pub trait BatchStore {
    type Staged;
    type Lease: BatchLease;
    fn snapshot(&self) -> Result<IndexSnapshot>;
    fn stage(&self, batch: &Batch) -> Result<Self::Staged>;
    /// Revalidation may do bounded IO but must not execute external tools.
    fn commit(
        &self,
        staged: Self::Staged,
        expected: &IndexSnapshot,
        control: &CommitControl,
        revalidate: impl FnOnce() -> Result<()>,
    ) -> Result<CommitReceipt>;
    fn pin(&self, layer: Layer) -> Result<Option<Self::Lease>>;
}

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Phase {
    #[default]
    Ready,
    Running,
    Finished,
}
#[derive(Default)]
struct State {
    phase: Phase,
    cancelled: bool,
}
/// A separate single-use admission interval for actual index publication.
/// Completing a DAG never consumes this control or authorizes a commit.
#[derive(Clone, Default)]
pub struct CommitControl {
    state: Arc<Mutex<State>>,
}
impl CommitControl {
    pub fn cancel(&self) -> CancelDecision {
        let mut state = self.lock();
        if state.phase == Phase::Finished {
            return CancelDecision::AlreadyFinished;
        }
        if state.cancelled {
            return CancelDecision::AlreadyRequested;
        }
        state.cancelled = true;
        CancelDecision::Requested
    }
    pub fn begin(&self) -> Result<CommitPermit<'_>> {
        let mut state = self.lock();
        if state.phase != Phase::Ready {
            return Err(error("STORE002", "commit", "commit control already used"));
        }
        state.phase = Phase::Running;
        Ok(CommitPermit {
            control: self,
            finished: false,
        })
    }
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
/// Host capability to linearize one atomic index replacement against cancel.
/// It is not held while waiting for a domain lock, copying files or revalidating.
pub struct CommitPermit<'a> {
    control: &'a CommitControl,
    finished: bool,
}
impl CommitPermit<'_> {
    pub fn cancelled(&self) -> bool {
        self.control.lock().cancelled
    }
    /// Run only the bounded index replacement (or same-content acceptance).
    /// Do not call external tools, callbacks, or cancel from within `publish`.
    pub fn publish(mut self, publish: impl FnOnce() -> Result<()>) -> Result<()> {
        let mut state = self.control.lock();
        if state.cancelled {
            state.phase = Phase::Finished;
            self.finished = true;
            return Err(error(
                "STORE002",
                "commit",
                "commit cancelled before index acceptance",
            ));
        }
        let result = publish();
        state.phase = Phase::Finished;
        self.finished = true;
        result
    }
}
impl Drop for CommitPermit<'_> {
    fn drop(&mut self) {
        if !self.finished {
            self.control.lock().phase = Phase::Finished;
        }
    }
}
fn error(code: &'static str, subject: &str, message: &str) -> QueryError {
    QueryError::new(ErrorKind::Conflict, code, subject, message)
}
