//! Local-filesystem batch transactions. Domain locks serialize only admission,
//! verification and index replacement; no subprocess executes under a lock.
#![cfg(unix)]
mod codec;
mod commit;
mod fs;
mod ownership;
mod recovery;
mod staging;

use crate::LocalWorkspace;
use madb_app::{
    Digest, ErrorKind, QueryError, Result,
    inputs::{FrozenInputs, capture_inputs},
    transaction::{
        Batch, BatchInfo, BatchSpec, BatchStore, CommitControl, CommitReceipt, IndexSnapshot, Layer,
    },
};
use rustix::fs::{FlockOperation, Mode, OFlags};
use std::{
    fs::File,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

const ENTRY_LIMIT: usize = 16384;
static NEXT: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransactionPoint {
    StageCreated,
    FileWritten,
    FileSynced,
    Staged,
    BeforeIndexReplace,
    AfterIndexReplace,
}

/// One canonical, local output directory. The persistent lock inode is never
/// replaced by MadB. Separate lock opens serialize threads as well as processes.
pub struct LocalBatchStore {
    root: File,
    path: PathBuf,
    lock_identity: (u64, u64),
    writable: bool,
}
pub struct StagedBatch {
    root: File,
    domain_identity: (u64, u64),
    name: String,
    path: PathBuf,
    info: BatchInfo,
    batch: Batch,
    _writer: File,
}
impl StagedBatch {
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub fn id(&self) -> &Digest {
        &self.info.id
    }
}
/// Pins both this physical version and its complete source/build ancestry.
/// Bytes and path remain fixed if another process publishes a new generation.
pub struct BatchLease {
    info: BatchInfo,
    files: FrozenInputs,
    path: PathBuf,
    _leases: Vec<File>,
}
impl BatchLease {
    pub fn info(&self) -> &BatchInfo {
        &self.info
    }
    pub fn files(&self) -> &FrozenInputs {
        &self.files
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
}
impl madb_app::transaction::BatchLease for BatchLease {
    fn info(&self) -> &BatchInfo {
        &self.info
    }
    fn files(&self) -> &FrozenInputs {
        &self.files
    }
}
#[derive(Debug, Default)]
pub struct RecoveryReport {
    pub removed_staging: Vec<String>,
    pub removed_batches: Vec<String>,
    pub retained_active: Vec<String>,
}

impl LocalBatchStore {
    pub fn open(path: &Path) -> Result<Self> {
        std::fs::create_dir_all(path).map_err(|e| io_error("domain", e))?;
        let (root, path) = open_root(path)?;
        let _initialization = ownership::initialization(&root, FlockOperation::LockExclusive)?;
        let fresh = fs::names(&root, ENTRY_LIMIT)?.is_empty();
        let lock = if fresh {
            // Exclusive creation can never silently adopt an unrelated lock.
            let lock = fs::regular(&root, ".domain-lock", true, true)?;
            lock.sync_all().map_err(|e| io_error("domain-lock", e))?;
            root.sync_all().map_err(|e| io_error("domain", e))?;
            lock
        } else {
            ownership::validate_root(&root)?;
            fs::regular(&root, ".domain-lock", false, false)?
        };
        let identity = identity(&lock)?;
        let store = Self {
            root,
            path,
            lock_identity: identity,
            writable: true,
        };
        let _guard = store.lock(FlockOperation::LockExclusive)?;
        if fresh {
            ownership::establish(&store.root)?;
        }
        fs::ensure_directory(&store.root, "staging")?;
        fs::ensure_directory(&store.root, "batches")?;
        codec::index(&store.root)?;
        Ok(store)
    }
    /// Opens an existing store without creating files, fixing records or changing
    /// permissions. A missing directory is None; an incomplete store is an error.
    pub fn open_readonly(path: &Path) -> Result<Option<Self>> {
        match std::fs::metadata(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(io_error("domain", e)),
            Ok(_) => {}
        }
        let (root, path) = open_root(path)?;
        let _initialization = ownership::initialization(&root, FlockOperation::LockShared)?;
        // An explicitly created but untouched output directory is also an
        // uninitialized store. Unknown contents are never treated as absent.
        if fs::names(&root, ENTRY_LIMIT)?.is_empty() {
            return Ok(None);
        }
        ownership::validate_root(&root)?;
        let lock = fs::regular(&root, ".domain-lock", false, false)?;
        let store = Self {
            root,
            path,
            lock_identity: identity(&lock)?,
            writable: false,
        };
        let _guard = store.lock(FlockOperation::LockShared)?;
        fs::directory(&store.root, "staging")?;
        fs::directory(&store.root, "batches")?;
        codec::index(&store.root)?;
        Ok(Some(store))
    }
    pub fn snapshot(&self) -> Result<IndexSnapshot> {
        let _guard = self.lock(FlockOperation::LockShared)?;
        let index = codec::index(&self.root)?;
        self.validate_index(&index, false)?;
        Ok(index)
    }
    pub fn stage(&self, batch: &Batch) -> Result<StagedBatch> {
        self.stage_with(batch, |_| Ok(()))
    }
    pub fn commit(
        &self,
        staged: StagedBatch,
        expected: &IndexSnapshot,
        control: &CommitControl,
        revalidate: impl FnOnce() -> Result<()>,
    ) -> Result<CommitReceipt> {
        self.commit_with(staged, expected, control, revalidate, |_| Ok(()))
    }
    pub fn pin(&self, layer: Layer) -> Result<Option<BatchLease>> {
        let _guard = self.lock(FlockOperation::LockShared)?;
        let index = codec::index(&self.root)?;
        self.validate_index(&index, false)?;
        let Some(id) = index.get(layer) else {
            return Ok(None);
        };
        let mut leases = Vec::new();
        let (info, files) = self.load_chain(id, layer, &mut leases)?;
        Ok(Some(BatchLease {
            path: self.path.join("batches").join(id_name(id)).join("files"),
            info,
            files,
            _leases: leases,
        }))
    }
    fn writable(&self) -> Result<()> {
        if self.writable {
            Ok(())
        } else {
            Err(fail(
                "STORE006",
                "domain",
                "read-only store cannot mutate output",
            ))
        }
    }
    fn lock(&self, operation: FlockOperation) -> Result<File> {
        let file = fs::lock(&self.root, ".domain-lock", operation)?;
        if identity(&file)? != self.lock_identity {
            return Err(fail(
                "STORE004",
                "domain-lock",
                "persistent domain lock was replaced",
            ));
        }
        Ok(file)
    }
    fn batch_directory(&self, id: &Digest) -> Result<File> {
        fs::directory(&self.root, &format!("batches/{}", id_name(id)))
    }
    fn metadata(&self, id: &Digest) -> Result<BatchInfo> {
        let directory = self
            .batch_directory(id)
            .map_err(|e| corrupt(id.as_str(), e))?;
        let info = codec::info(&directory)?;
        if info.id != *id {
            return Err(fail(
                "STORE004",
                id.as_str(),
                "batch identity does not match its directory",
            ));
        }
        validate_spec(&info.spec)?;
        Ok(info)
    }
    fn validate_index(&self, index: &IndexSnapshot, bytes: bool) -> Result<()> {
        let mut parent: Option<BatchInfo> = None;
        for layer in [Layer::Source, Layer::Build, Layer::Candidate] {
            if let Some(id) = index.get(layer) {
                let info = if bytes {
                    load(&self.batch_directory(id)?, Some(id))?.0
                } else {
                    self.metadata(id)?
                };
                if info.spec.layer != layer {
                    return Err(fail(
                        "STORE004",
                        id.as_str(),
                        "index layer does not match batch",
                    ));
                }
                pair(&info.spec, parent.as_ref()).map_err(|e| corrupt(id.as_str(), e))?;
                parent = Some(info);
            }
        }
        Ok(())
    }
    fn load_chain(
        &self,
        id: &Digest,
        layer: Layer,
        leases: &mut Vec<File>,
    ) -> Result<(BatchInfo, FrozenInputs)> {
        let directory = self.batch_directory(id)?;
        leases.push(fs::lock(&directory, ".lease", FlockOperation::LockShared)?);
        let (info, files) = load(&directory, Some(id))?;
        if info.spec.layer != layer {
            return Err(fail("STORE004", id.as_str(), "parent layer mismatch"));
        }
        match layer {
            Layer::Source => pair(&info.spec, None)?,
            Layer::Build | Layer::Candidate => {
                let parent_id = info
                    .spec
                    .parent
                    .as_ref()
                    .ok_or_else(|| fail("STORE004", id.as_str(), "parent missing"))?;
                let parent_layer = if layer == Layer::Build {
                    Layer::Source
                } else {
                    Layer::Build
                };
                let (parent, _) = self.load_chain(parent_id, parent_layer, leases)?;
                pair(&info.spec, Some(&parent)).map_err(|e| corrupt(id.as_str(), e))?;
            }
        }
        Ok((info, files))
    }
}
impl BatchStore for LocalBatchStore {
    type Staged = StagedBatch;
    type Lease = BatchLease;
    fn snapshot(&self) -> Result<IndexSnapshot> {
        self.snapshot()
    }
    fn stage(&self, batch: &Batch) -> Result<StagedBatch> {
        self.stage(batch)
    }
    fn commit(
        &self,
        staged: StagedBatch,
        expected: &IndexSnapshot,
        control: &CommitControl,
        revalidate: impl FnOnce() -> Result<()>,
    ) -> Result<CommitReceipt> {
        self.commit(staged, expected, control, revalidate)
    }
    fn pin(&self, layer: Layer) -> Result<Option<BatchLease>> {
        self.pin(layer)
    }
}
fn open_root(path: &Path) -> Result<(File, PathBuf)> {
    let path = std::fs::canonicalize(path).map_err(|e| io_error("domain", e))?;
    let root = File::from(
        rustix::fs::open(
            &path,
            OFlags::RDONLY
                | OFlags::DIRECTORY
                | OFlags::NOFOLLOW
                | OFlags::CLOEXEC
                | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(|e| io_error("domain", e))?,
    );
    Ok((root, path))
}
fn identity(file: &File) -> Result<(u64, u64)> {
    let meta = file.metadata().map_err(|e| io_error("identity", e))?;
    Ok((meta.dev(), meta.ino()))
}
fn nonce() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}
fn id_name(id: &Digest) -> &str {
    &id.as_str()[7..]
}
fn validate_spec(spec: &BatchSpec) -> Result<()> {
    if (spec.layer == Layer::Source) != spec.parent.is_none() {
        return Err(fail("STORE004", "parent", "invalid storage ancestry"));
    }
    Ok(())
}
fn pair(spec: &BatchSpec, parent: Option<&BatchInfo>) -> Result<()> {
    match (spec.layer, parent) {
        (Layer::Source, None) if spec.parent.is_none() => Ok(()),
        (Layer::Build | Layer::Candidate, Some(parent))
            if spec.parent.as_ref() == Some(&parent.id)
                && parent.spec.layer
                    == if spec.layer == Layer::Build {
                        Layer::Source
                    } else {
                        Layer::Build
                    }
                && spec.input_digest == parent.spec.input_digest
                && spec.profile_digest == parent.spec.profile_digest
                && spec.generation_id == parent.spec.generation_id =>
        {
            Ok(())
        }
        _ => Err(fail(
            "STORE003",
            "parent",
            "batch parent, profile, generation or source lineage no longer matches",
        )),
    }
}
fn load(directory: &File, expected_id: Option<&Digest>) -> Result<(BatchInfo, FrozenInputs)> {
    if fs::names(directory, 4)? != [".lease", "files", "metadata.json"] {
        return Err(fail("STORE004", "batch", "unexpected batch entries"));
    }
    fs::regular(directory, ".lease", false, false)?;
    let info = codec::info(directory)?;
    if expected_id.is_some_and(|id| *id != info.id) {
        return Err(fail(
            "STORE004",
            "batch",
            "physical batch identity mismatch",
        ));
    }
    let workspace = LocalWorkspace {
        root: fs::directory(directory, "files")?,
    };
    let files = capture_inputs(&workspace, info.files.clone()).map_err(|e| corrupt("files", e))?;
    crate::inputs::tree::verify_contents(&workspace, &files).map_err(|e| corrupt("files", e))?;
    if info.files != files.references().cloned().collect::<Vec<_>>() {
        return Err(fail(
            "STORE004",
            "files",
            "file inventory is not in canonical order",
        ));
    }
    let batch = Batch::new(info.spec.clone(), files.clone()).map_err(|e| corrupt("metadata", e))?;
    if batch.id() != &info.content_id {
        return Err(fail(
            "STORE004",
            "content_id",
            "batch content identity mismatch",
        ));
    }
    Ok((info, files))
}
fn fail(code: &'static str, subject: &str, message: impl std::fmt::Display) -> QueryError {
    QueryError::new(ErrorKind::Conflict, code, subject, message.to_string())
}
fn io_error(subject: &str, error: impl std::fmt::Display) -> QueryError {
    QueryError::new(ErrorKind::Host, "STORE005", subject, error.to_string())
}
fn corrupt(subject: &str, error: impl std::fmt::Display) -> QueryError {
    fail("STORE004", subject, error)
}
