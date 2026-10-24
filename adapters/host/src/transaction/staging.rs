use super::*;
use rustix::fs::{fchmod, flock};

impl LocalBatchStore {
    /// Fault hooks are an explicit testing seam, never loaded from environment.
    /// Normal callers use stage(). Hooks execute outside the domain lock.
    pub fn stage_with(
        &self,
        batch: &Batch,
        mut hook: impl FnMut(TransactionPoint) -> Result<()>,
    ) -> Result<StagedBatch> {
        self.writable()?;
        let staged = self.allocate(batch)?;
        hook(TransactionPoint::StageCreated)?;
        let files = fs::create_directory(&staged.root, "files")?;
        let directories: Vec<_> = batch.files().directories().cloned().collect();
        for path in &directories {
            let (parent, name) = parent_name(&files, path.as_str())?;
            fs::create_directory(&parent, name)?;
        }
        for (reference, bytes) in batch.files().files() {
            let (parent, name) = parent_name(&files, reference.path.as_str())?;
            let file = fs::write(&parent, name, bytes)?;
            hook(TransactionPoint::FileWritten)?;
            fchmod(&file, Mode::from_bits_retain(0o400)).map_err(|e| io_error(name, e))?;
            file.sync_all().map_err(|e| io_error(name, e))?;
            hook(TransactionPoint::FileSynced)?;
        }
        fs::readonly_tree(&files, &directories)?;
        let metadata = fs::write(
            &staged.root,
            "metadata.json",
            &codec::encode(&codec::metadata(&staged.info))?,
        )?;
        fchmod(&metadata, Mode::from_bits_retain(0o400)).map_err(|e| io_error("metadata", e))?;
        metadata.sync_all().map_err(|e| io_error("metadata", e))?;
        fchmod(&staged.root, Mode::from_bits_retain(0o500)).map_err(|e| io_error("batch", e))?;
        staged.root.sync_all().map_err(|e| io_error("batch", e))?;
        load(&staged.root, Some(staged.id()))?;
        hook(TransactionPoint::Staged)?;
        Ok(staged)
    }
    fn allocate(&self, batch: &Batch) -> Result<StagedBatch> {
        let _guard = self.lock(FlockOperation::LockExclusive)?;
        ownership::validate_root(&self.root)?;
        let staging = fs::directory(&self.root, "staging")?;
        let batches = fs::directory(&self.root, "batches")?;
        for _ in 0..ENTRY_LIMIT {
            let name = nonce();
            let id = Digest::of_bytes(
                format!("madb.storage-instance/1\0{}\0{name}", batch.id().as_str()).as_bytes(),
            );
            if fs::exists(&staging, &name)? || fs::exists(&batches, id_name(&id))? {
                continue;
            }
            let root = fs::create_directory(&staging, &name)?;
            let writer = fs::regular(&root, ".lease", true, true)?;
            flock(&writer, FlockOperation::LockExclusive)
                .map_err(|e| io_error("writer lease", e))?;
            writer.sync_all().map_err(|e| io_error("writer lease", e))?;
            root.sync_all().map_err(|e| io_error("staging", e))?;
            staging.sync_all().map_err(|e| io_error("staging", e))?;
            return Ok(StagedBatch {
                root,
                domain_identity: identity(&self.root)?,
                path: self.path.join("staging").join(&name),
                name,
                info: BatchInfo {
                    id,
                    content_id: batch.id().clone(),
                    spec: batch.spec().clone(),
                    files: batch.files().references().cloned().collect(),
                },
                batch: batch.clone(),
                _writer: writer,
            });
        }
        Err(fail(
            "STORE007",
            "staging",
            "cannot allocate unique staging instance",
        ))
    }
}
fn parent_name<'a>(root: &File, path: &'a str) -> Result<(File, &'a str)> {
    match path.rsplit_once('/') {
        Some((parent, name)) => Ok((fs::directory(root, parent)?, name)),
        None => Ok((root.try_clone().map_err(|e| io_error(path, e))?, path)),
    }
}
