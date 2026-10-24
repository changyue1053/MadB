use super::*;
use rustix::fs::{AtFlags, flock, unlinkat};
use std::collections::BTreeSet;

impl LocalBatchStore {
    /// Explicit recovery trusts only the committed index, never the newest
    /// directory. Existing read and writer leases protect their complete trees.
    pub fn recover(&self) -> Result<RecoveryReport> {
        self.writable()?;
        let _guard = self.lock(FlockOperation::LockExclusive)?;
        ownership::validate_root(&self.root)?;
        let staging_names = ownership::namespace(&self.root, "staging")?;
        let batch_names = ownership::namespace(&self.root, "batches")?;
        let current = codec::index(&self.root)?;
        self.validate_index(&current, true)?;
        let retained: BTreeSet<_> = [current.source, current.build, current.candidate]
            .into_iter()
            .flatten()
            .map(|id| id_name(&id).to_owned())
            .collect();
        let mut report = RecoveryReport::default();
        let mut remaining = 200_000;
        for (category, names) in [("staging", staging_names), ("batches", batch_names)] {
            let parent = fs::directory(&self.root, category)?;
            for name in names {
                if category == "batches" && retained.contains(&name) {
                    continue;
                }
                let directory = fs::directory(&parent, &name)?;
                let lease = if fs::exists(&directory, ".lease")? {
                    let file = fs::regular(&directory, ".lease", false, false)?;
                    match flock(&file, FlockOperation::NonBlockingLockExclusive) {
                        Ok(()) => Some(file),
                        Err(rustix::io::Errno::WOULDBLOCK) => {
                            report.retained_active.push(format!("{category}/{name}"));
                            continue;
                        }
                        Err(e) => return Err(io_error("recovery lease", e)),
                    }
                } else {
                    None
                };
                // Hold the exclusive lease until every owned entry is removed.
                fs::remove_tree(&parent, &name, &mut remaining)?;
                drop(lease);
                if category == "staging" {
                    report.removed_staging.push(name);
                } else {
                    report.removed_batches.push(name);
                }
            }
            parent.sync_all().map_err(|e| io_error(category, e))?;
        }
        for name in fs::names(&self.root, ENTRY_LIMIT)? {
            if ownership::index_temporary(&name) {
                fs::regular(&self.root, &name, false, false)?;
                unlinkat(&self.root, name.as_str(), AtFlags::empty())
                    .map_err(|e| io_error("index temporary", e))?;
            }
        }
        self.root.sync_all().map_err(|e| io_error("recovery", e))?;
        Ok(report)
    }
}
