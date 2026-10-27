use super::*;
use madb_app::transaction::CommitDisposition;
use rustix::fs::{fchmod, renameat};

impl LocalBatchStore {
    /// Test-only failure/barrier seam. BeforeIndexReplace and AfterIndexReplace
    /// run under the domain lock, but outside the cancellation admission mutex.
    /// Hooks must not enter this store recursively. Production uses commit().
    pub fn commit_with(
        &self,
        staged: StagedBatch,
        expected: &IndexSnapshot,
        control: &CommitControl,
        revalidate: impl FnOnce() -> Result<()>,
        mut hook: impl FnMut(TransactionPoint) -> Result<()>,
    ) -> Result<CommitReceipt> {
        self.writable()?;
        let mut permit = control.begin()?;
        permit.reject_if_cancelled()?;
        let _guard = self.lock(FlockOperation::LockExclusive)?;
        ownership::validate_root(&self.root)?;
        if staged.domain_identity != identity(&self.root)? {
            return Err(fail(
                "STORE001",
                "staging",
                "staged batch belongs to another output domain",
            ));
        }
        let staging = fs::directory(&self.root, "staging")?;
        let location = fs::directory(&staging, &staged.name)?;
        if identity(&location)? != identity(&staged.root)? {
            return Err(fail(
                "STORE004",
                "staging",
                "staging directory was replaced",
            ));
        }
        let (info, _) = load(&staged.root, Some(staged.id()))?;
        if info != staged.info || info.content_id != *staged.batch.id() {
            return Err(fail(
                "STORE004",
                "staging",
                "staged bytes no longer match prepared batch",
            ));
        }
        let current = codec::index(&self.root)?;
        self.validate_parent(&info.spec, &current)?;
        // Reuse requires verifying the actual complete bytes, not only an ID or
        // cache label. Corrupt bytes are rebuilt into a fresh physical instance.
        if let Some(id) = current.get(info.spec.layer)
            && let Ok((existing, _)) = self
                .batch_directory(id)
                .and_then(|directory| load(&directory, Some(id)))
            && existing.content_id == info.content_id
        {
            revalidate()?;
            permit.publish(|| Ok(()))?;
            return Ok(CommitReceipt {
                disposition: CommitDisposition::Reused,
                index: current.clone(),
                batch_id: id.clone(),
            });
        }
        if &current != expected {
            return Err(fail(
                "STORE003",
                "index",
                "output generation changed; replan against current index",
            ));
        }
        let next = current.advance(info.spec.layer, info.id.clone())?;
        let batches = fs::directory(&self.root, "batches")?;
        if fs::exists(&batches, id_name(&info.id))? {
            return Err(fail(
                "STORE004",
                "batch",
                "new physical instance already exists",
            ));
        }
        // Darwin requires write permission when the directory parent changes.
        // Only the private container changes mode, before any index points here.
        fchmod(&staged.root, Mode::from_bits_retain(0o700))
            .map_err(|e| io_error("install batch mode", e))?;
        renameat(&staging, staged.name.as_str(), &batches, id_name(&info.id))
            .map_err(|e| io_error("install batch", e))?;
        fchmod(&staged.root, Mode::from_bits_retain(0o500))
            .map_err(|e| io_error("installed batch mode", e))?;
        staged
            .root
            .sync_all()
            .map_err(|e| io_error("installed batch", e))?;
        batches.sync_all().map_err(|e| io_error("batches", e))?;
        staging.sync_all().map_err(|e| io_error("staging", e))?;
        let temporary = self.index_temporary(&next)?;
        hook(TransactionPoint::BeforeIndexReplace)?;
        // This final bounded observation occurs after test barriers and as near
        // acceptance as possible. It is not a filesystem-wide atomic snapshot.
        revalidate()?;
        permit.publish(|| {
            renameat(&self.root, temporary.as_str(), &self.root, "current.json")
                .map_err(|e| io_error("index replacement", e))
        })?;
        // Cancellation is now AlreadyFinished. A subsequent durability/receipt
        // error must never claim that publication did not happen.
        hook(TransactionPoint::AfterIndexReplace).and_then(|()| self.root.sync_all().map_err(|e| io_error("domain durability", e)))
            .map_err(|e| fail("STORE008", "index", format!("index already replaced; durability/receipt uncertain, inspect current before retry: {e}")))?;
        Ok(CommitReceipt {
            disposition: CommitDisposition::Committed,
            index: next,
            batch_id: info.id,
        })
    }
    fn validate_parent(&self, spec: &BatchSpec, current: &IndexSnapshot) -> Result<()> {
        if spec.layer == Layer::Source {
            return pair(spec, None);
        }
        // Child acceptance requires a complete, valid current parent chain.
        let parent_layer = if spec.layer == Layer::Build {
            Layer::Source
        } else {
            Layer::Build
        };
        let id = current
            .get(parent_layer)
            .ok_or_else(|| fail("STORE003", "parent", "required current parent is missing"))?;
        let mut leases = Vec::new();
        let (parent, _) = self.load_chain(id, parent_layer, &mut leases)?;
        if spec.layer == Layer::Candidate && parent.spec.parent.as_ref() != current.source.as_ref()
        {
            return Err(fail(
                "STORE004",
                "parent",
                "current build does not belong to current source",
            ));
        }
        pair(spec, Some(&parent))
    }
    fn index_temporary(&self, next: &IndexSnapshot) -> Result<String> {
        for _ in 0..ENTRY_LIMIT {
            let name = format!(".index-{}.tmp", nonce());
            if fs::exists(&self.root, &name)? {
                continue;
            }
            let file = fs::write(
                &self.root,
                &name,
                &codec::encode(&codec::index_value(next))?,
            )?;
            file.sync_all().map_err(|e| io_error("index", e))?;
            return Ok(name);
        }
        Err(fail("STORE007", "index", "cannot allocate index temporary"))
    }
}
