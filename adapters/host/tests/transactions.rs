#![cfg(unix)]
use madb_app::{
    Digest, ErrorKind, LogicalPath, QueryError, Result,
    inputs::capture_inputs,
    ports::{FileReference, ReadWorkspace},
    process::CancelDecision,
    transaction::{Batch, BatchSpec, CommitControl, CommitDisposition, IndexSnapshot, Layer},
};
use madb_host::transaction::{LocalBatchStore, TransactionPoint};
use std::{
    fs,
    os::unix::fs::{MetadataExt, PermissionsExt, symlink},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Temp(PathBuf);
impl Temp {
    fn new() -> Self {
        loop {
            let path = std::env::temp_dir().join(format!(
                "madb-transactions-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => panic!("{e}"),
            }
        }
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        remove(&self.0);
    }
}
fn remove(path: &Path) {
    if fs::symlink_metadata(path).unwrap().is_dir() {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        for entry in fs::read_dir(path).unwrap() {
            remove(&entry.unwrap().path());
        }
        fs::remove_dir(path).unwrap();
    } else {
        fs::remove_file(path).unwrap();
    }
}
struct Source(Vec<u8>);
impl ReadWorkspace for Source {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        assert_eq!(path.as_str(), "include/api.h");
        assert!(self.0.len() <= limit);
        Ok(Some(self.0.clone()))
    }
}
fn batch(layer: Layer, parent: Option<Digest>, tag: &[u8], bytes: &[u8]) -> Batch {
    let file = FileReference {
        path: LogicalPath::new("include/api.h").unwrap(),
        digest: Digest::of_bytes(bytes),
        size: bytes.len() as u64,
    };
    Batch::new(
        BatchSpec {
            layer,
            parent,
            input_digest: Digest::of_bytes(tag),
            task_digest: Digest::of_bytes(b"task"),
            profile_digest: Digest::of_bytes(b"profile"),
            generation_id: Digest::of_bytes(tag),
        },
        capture_inputs(&Source(bytes.to_vec()), vec![file]).unwrap(),
    )
    .unwrap()
}
fn publish(
    store: &LocalBatchStore,
    batch: &Batch,
    expected: &IndexSnapshot,
) -> madb_app::transaction::CommitReceipt {
    store
        .commit(
            store.stage(batch).unwrap(),
            expected,
            &CommitControl::default(),
            || Ok(()),
        )
        .unwrap()
}
fn code<T>(result: Result<T>) -> String {
    result.err().expect("expected failure").findings[0]
        .code
        .to_owned()
}
fn batch_path(root: &Path, id: &Digest) -> PathBuf {
    root.join("batches").join(&id.as_str()[7..])
}

#[test]
fn three_layers_pin_fixed_ancestry_and_source_switch_clears_descendants() {
    let tmp = Temp::new();
    let store = LocalBatchStore::open(&tmp.0).unwrap();
    let source = publish(
        &store,
        &batch(Layer::Source, None, b"A", b"source A"),
        &IndexSnapshot::default(),
    );
    let build = publish(
        &store,
        &batch(
            Layer::Build,
            Some(source.batch_id.clone()),
            b"A",
            b"build A",
        ),
        &source.index,
    );
    let candidate = publish(
        &store,
        &batch(
            Layer::Candidate,
            Some(build.batch_id.clone()),
            b"A",
            b"candidate A",
        ),
        &build.index,
    );
    let lease = store.pin(Layer::Candidate).unwrap().unwrap();
    let new = publish(
        &store,
        &batch(Layer::Source, None, b"B", b"source B"),
        &candidate.index,
    );
    assert!(new.index.build.is_none() && new.index.candidate.is_none());
    assert_eq!(lease.files().files().next().unwrap().1, b"candidate A");
    assert_eq!(
        fs::read(lease.path().join("include/api.h")).unwrap(),
        b"candidate A"
    );
    let report = store.recover().unwrap();
    assert_eq!(report.retained_active.len(), 3);
    for id in [&source.batch_id, &build.batch_id, &candidate.batch_id] {
        assert!(batch_path(&tmp.0, id).exists());
    }
    drop(lease);
    assert_eq!(store.recover().unwrap().removed_batches.len(), 3);
}
#[test]
fn active_staging_writer_is_retained_then_collectible() {
    let tmp = Temp::new();
    let store = LocalBatchStore::open(&tmp.0).unwrap();
    let staged = store
        .stage(&batch(Layer::Source, None, b"A", b"A"))
        .unwrap();
    let path = staged.path().to_owned();
    assert_eq!(store.recover().unwrap().retained_active.len(), 1);
    assert!(path.exists());
    drop(staged);
    assert_eq!(store.recover().unwrap().removed_staging.len(), 1);
    assert!(!path.exists());
}
#[test]
fn source_cas_rejects_old_result_and_verified_same_content_reuses() {
    let tmp = Temp::new();
    let store = LocalBatchStore::open(&tmp.0).unwrap();
    let expected = IndexSnapshot::default();
    let old = store
        .stage(&batch(Layer::Source, None, b"A", b"A"))
        .unwrap();
    let b = batch(Layer::Source, None, b"B", b"B");
    let current = publish(&store, &b, &expected);
    assert_eq!(
        code(store.commit(old, &expected, &CommitControl::default(), || Ok(()))),
        "STORE003"
    );
    let reuse = publish(&store, &b, &expected);
    assert_eq!(reuse.disposition, CommitDisposition::Reused);
    assert_eq!(reuse.index, current.index);
    assert_eq!(reuse.batch_id, current.batch_id);
}
#[test]
fn build_parent_profile_and_generation_must_match_current_source() {
    let tmp = Temp::new();
    let store = LocalBatchStore::open(&tmp.0).unwrap();
    let a = publish(
        &store,
        &batch(Layer::Source, None, b"A", b"A"),
        &IndexSnapshot::default(),
    );
    let mismatch = batch(
        Layer::Build,
        Some(a.batch_id.clone()),
        b"different lineage",
        b"build",
    );
    assert_eq!(
        code(store.commit(
            store.stage(&mismatch).unwrap(),
            &a.index,
            &CommitControl::default(),
            || Ok(())
        )),
        "STORE003"
    );
    let oldbuild = store
        .stage(&batch(
            Layer::Build,
            Some(a.batch_id.clone()),
            b"A",
            b"build",
        ))
        .unwrap();
    let b = publish(&store, &batch(Layer::Source, None, b"B", b"B"), &a.index);
    assert_eq!(
        code(store.commit(oldbuild, &a.index, &CommitControl::default(), || Ok(()))),
        "STORE003"
    );
    assert_eq!(store.snapshot().unwrap(), b.index);
}
#[test]
fn cancel_before_index_replace_leaves_previous_index_and_collectible_orphan() {
    let tmp = Temp::new();
    let store = LocalBatchStore::open(&tmp.0).unwrap();
    let control = CommitControl::default();
    let staged = store
        .stage(&batch(Layer::Source, None, b"A", b"A"))
        .unwrap();
    let result = store.commit_with(
        staged,
        &IndexSnapshot::default(),
        &control,
        || Ok(()),
        |point| {
            if point == TransactionPoint::BeforeIndexReplace {
                assert_eq!(control.cancel(), CancelDecision::Requested);
            }
            Ok(())
        },
    );
    assert_eq!(code(result), "STORE002");
    assert_eq!(store.snapshot().unwrap(), IndexSnapshot::default());
    assert_eq!(store.recover().unwrap().removed_batches.len(), 1);
}
#[test]
fn publication_wins_before_cancel_and_late_failure_is_explicitly_uncertain() {
    let tmp = Temp::new();
    let store = LocalBatchStore::open(&tmp.0).unwrap();
    let control = CommitControl::default();
    let staged = store
        .stage(&batch(Layer::Source, None, b"A", b"A"))
        .unwrap();
    let result = store.commit_with(
        staged,
        &IndexSnapshot::default(),
        &control,
        || Ok(()),
        |point| {
            if point == TransactionPoint::AfterIndexReplace {
                assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
                return Err(QueryError::new(
                    ErrorKind::Host,
                    "TEST",
                    "receipt",
                    "injected receipt failure",
                ));
            }
            Ok(())
        },
    );
    assert_eq!(code(result), "STORE008");
    assert_eq!(store.snapshot().unwrap().revision, 1);
    assert_eq!(
        store
            .pin(Layer::Source)
            .unwrap()
            .unwrap()
            .files()
            .files()
            .next()
            .unwrap()
            .1,
        b"A"
    );
}
#[test]
fn input_revalidation_after_barrier_rejects_change() {
    let tmp = Temp::new();
    let store = LocalBatchStore::open(&tmp.0).unwrap();
    let changed = std::cell::Cell::new(false);
    let staged = store
        .stage(&batch(Layer::Source, None, b"A", b"A"))
        .unwrap();
    let result = store.commit_with(
        staged,
        &IndexSnapshot::default(),
        &CommitControl::default(),
        || {
            assert!(changed.get());
            Err(QueryError::new(
                ErrorKind::Conflict,
                "TEST_INPUT_CHANGED",
                "source",
                "changed",
            ))
        },
        |point| {
            if point == TransactionPoint::BeforeIndexReplace {
                changed.set(true);
            }
            Ok(())
        },
    );
    assert_eq!(code(result), "TEST_INPUT_CHANGED");
    assert_eq!(store.snapshot().unwrap().revision, 0);
}
#[test]
fn corrupt_bytes_rebuild_to_new_physical_instance_without_reusing_current() {
    let tmp = Temp::new();
    let store = LocalBatchStore::open(&tmp.0).unwrap();
    let b = batch(Layer::Source, None, b"A", b"A");
    let first = publish(&store, &b, &IndexSnapshot::default());
    let path = batch_path(&tmp.0, &first.batch_id).join("files/include/api.h");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&path, b"X").unwrap();
    assert_eq!(code(store.pin(Layer::Source)), "STORE004");
    let fresh = publish(&store, &b, &first.index);
    assert_ne!(fresh.batch_id, first.batch_id);
    assert_eq!(fresh.disposition, CommitDisposition::Committed);
    assert_eq!(fs::read(&path).unwrap(), b"X");
    assert_eq!(
        store
            .pin(Layer::Source)
            .unwrap()
            .unwrap()
            .files()
            .files()
            .next()
            .unwrap()
            .1,
        b"A"
    );
}
#[test]
fn staged_inventory_and_link_substitution_are_rejected() {
    let tmp = Temp::new();
    let store = LocalBatchStore::open(&tmp.0).unwrap();
    let staged = store
        .stage(&batch(Layer::Source, None, b"A", b"A"))
        .unwrap();
    let parent = staged.path().join("files/include");
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    fs::write(parent.join("extra.h"), b"extra").unwrap();
    assert_eq!(
        code(store.commit(
            staged,
            &IndexSnapshot::default(),
            &CommitControl::default(),
            || Ok(())
        )),
        "STORE004"
    );
    for hard in [false, true] {
        let staged = store
            .stage(&batch(Layer::Source, None, b"A", b"A"))
            .unwrap();
        let parent = staged.path().join("files/include");
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
        fs::remove_file(parent.join("api.h")).unwrap();
        // This is genuinely outside the managed output domain; the store now
        // rejects unrelated top-level entries before processing any batch.
        let external_root = Temp::new();
        let external = external_root.0.join(if hard { "hard" } else { "symlink" });
        fs::write(&external, b"A").unwrap();
        if hard {
            fs::hard_link(&external, parent.join("api.h")).unwrap();
        } else {
            symlink(&external, parent.join("api.h")).unwrap();
        }
        assert_eq!(
            code(store.commit(
                staged,
                &IndexSnapshot::default(),
                &CommitControl::default(),
                || Ok(())
            )),
            "STORE004"
        );
    }
}
#[test]
fn readonly_open_and_pin_do_not_create_or_change_output_metadata() {
    let tmp = Temp::new();
    let missing = tmp.0.join("missing");
    assert!(LocalBatchStore::open_readonly(&missing).unwrap().is_none());
    assert!(!missing.exists());
    let store = LocalBatchStore::open(&tmp.0).unwrap();
    let current = publish(
        &store,
        &batch(Layer::Source, None, b"A", b"A"),
        &IndexSnapshot::default(),
    );
    let before = fs::metadata(&tmp.0).unwrap();
    let index = fs::read(tmp.0.join("current.json")).unwrap();
    let readonly = LocalBatchStore::open_readonly(&tmp.0).unwrap().unwrap();
    assert_eq!(readonly.snapshot().unwrap(), current.index);
    drop(readonly.pin(Layer::Source).unwrap());
    assert_eq!(
        code(readonly.stage(&batch(Layer::Source, None, b"B", b"B"))),
        "STORE006"
    );
    assert_eq!(code(readonly.recover()), "STORE006");
    let after = fs::metadata(&tmp.0).unwrap();
    assert_eq!(
        (before.mtime(), before.mtime_nsec()),
        (after.mtime(), after.mtime_nsec())
    );
    assert_eq!(index, fs::read(tmp.0.join("current.json")).unwrap());
}
#[test]
fn wrong_output_domain_and_corrupt_index_cannot_publish_or_recover() {
    let first = Temp::new();
    let second = Temp::new();
    let a = LocalBatchStore::open(&first.0).unwrap();
    let b = LocalBatchStore::open(&second.0).unwrap();
    let staged = a.stage(&batch(Layer::Source, None, b"A", b"A")).unwrap();
    assert_eq!(
        code(b.commit(
            staged,
            &IndexSnapshot::default(),
            &CommitControl::default(),
            || Ok(())
        )),
        "STORE001"
    );
    let current = publish(
        &a,
        &batch(Layer::Source, None, b"A", b"A"),
        &IndexSnapshot::default(),
    );
    fs::write(first.0.join("current.json"), b"{broken").unwrap();
    assert_eq!(code(a.recover()), "STORE004");
    assert!(batch_path(&first.0, &current.batch_id).exists());
}
#[test]
fn canonical_output_alias_uses_same_store_and_persistent_lock_inode() {
    let tmp = Temp::new();
    let root = tmp.0.join("actual");
    let a = LocalBatchStore::open(&root).unwrap();
    let alias = tmp.0.join("alias");
    symlink(&root, &alias).unwrap();
    let b = LocalBatchStore::open(&alias).unwrap();
    let lock = fs::metadata(root.join(".domain-lock")).unwrap().ino();
    let c = publish(
        &a,
        &batch(Layer::Source, None, b"A", b"A"),
        &IndexSnapshot::default(),
    );
    assert_eq!(b.snapshot().unwrap(), c.index);
    a.recover().unwrap();
    assert_eq!(lock, fs::metadata(root.join(".domain-lock")).unwrap().ino());
}

#[test]
fn empty_readonly_domain_is_missing_without_initialization_but_unknown_contents_fail() {
    let tmp = Temp::new();
    let before = fs::metadata(&tmp.0).unwrap();
    assert!(LocalBatchStore::open_readonly(&tmp.0).unwrap().is_none());
    assert_eq!(fs::read_dir(&tmp.0).unwrap().count(), 0);
    let after = fs::metadata(&tmp.0).unwrap();
    assert_eq!(
        (before.mtime(), before.mtime_nsec()),
        (after.mtime(), after.mtime_nsec())
    );
    fs::write(tmp.0.join("unknown"), b"not a managed store").unwrap();
    assert!(LocalBatchStore::open_readonly(&tmp.0).is_err());
    assert!(!tmp.0.join(".domain-lock").exists());
}

#[test]
fn nonempty_unknown_directory_is_never_claimed_or_modified() {
    let tmp = Temp::new();
    let photos = tmp.0.join("staging/photos");
    fs::create_dir_all(&photos).unwrap();
    fs::write(photos.join("family.jpg"), b"user content").unwrap();
    fs::create_dir(tmp.0.join("batches")).unwrap();
    let before = fs::metadata(&tmp.0).unwrap();
    assert_eq!(code(LocalBatchStore::open(&tmp.0)), "STORE004");
    assert_eq!(
        fs::read(photos.join("family.jpg")).unwrap(),
        b"user content"
    );
    assert!(!tmp.0.join(".domain-lock").exists());
    assert!(!tmp.0.join(".owner.json").exists());
    let after = fs::metadata(&tmp.0).unwrap();
    assert_eq!(
        (before.mtime(), before.mtime_nsec()),
        (after.mtime(), after.mtime_nsec())
    );
}

#[test]
fn recovery_requires_valid_owner_marker_even_for_an_existing_open_store() {
    let tmp = Temp::new();
    let store = LocalBatchStore::open(&tmp.0).unwrap();
    let staged = store
        .stage(&batch(Layer::Source, None, b"A", b"A"))
        .unwrap();
    let path = staged.path().to_owned();
    drop(staged);
    let owner = tmp.0.join(".owner.json");
    let marker = fs::read(&owner).unwrap();
    fs::remove_file(&owner).unwrap();
    assert_eq!(code(store.recover()), "STORE004");
    assert_eq!(code(LocalBatchStore::open(&tmp.0)), "STORE004");
    assert_eq!(code(LocalBatchStore::open_readonly(&tmp.0)), "STORE004");
    assert!(path.exists());
    fs::write(&owner, b"{\"schema\":\"another-tool\"}").unwrap();
    assert_eq!(code(store.recover()), "STORE004");
    assert!(path.exists());
    fs::write(owner, marker).unwrap();
    assert_eq!(store.recover().unwrap().removed_staging.len(), 1);
}

#[test]
fn interrupted_initialization_needs_durable_owner_before_layout_repair() {
    let unknown = Temp::new();
    fs::write(unknown.0.join(".domain-lock"), b"").unwrap();
    assert_eq!(code(LocalBatchStore::open(&unknown.0)), "STORE004");
    assert_eq!(fs::read_dir(&unknown.0).unwrap().count(), 1);
    let owned = Temp::new();
    drop(LocalBatchStore::open(&owned.0).unwrap());
    fs::remove_dir(owned.0.join("staging")).unwrap();
    fs::remove_dir(owned.0.join("batches")).unwrap();
    assert!(LocalBatchStore::open_readonly(&owned.0).is_err());
    assert!(!owned.0.join("staging").exists());
    let restored = LocalBatchStore::open(&owned.0).unwrap();
    assert_eq!(restored.snapshot().unwrap(), IndexSnapshot::default());
    assert!(owned.0.join("staging").is_dir() && owned.0.join("batches").is_dir());
}

#[test]
fn recovery_preflights_all_namespaces_before_deleting_any_orphan() {
    for unknown in ["staging/photos", "batches/photos", "user-data"] {
        let tmp = Temp::new();
        let store = LocalBatchStore::open(&tmp.0).unwrap();
        let orphan = store
            .stage(&batch(Layer::Source, None, b"A", b"A"))
            .unwrap();
        let path = orphan.path().to_owned();
        drop(orphan);
        let user = tmp.0.join(unknown);
        fs::create_dir(&user).unwrap();
        fs::write(user.join("sentinel"), b"preserve").unwrap();
        assert_eq!(code(store.recover()), "STORE004");
        assert!(path.exists(), "known orphan must survive failed preflight");
        assert_eq!(fs::read(user.join("sentinel")).unwrap(), b"preserve");
    }
}

#[test]
fn first_openers_coordinate_initialization_without_holding_lock_for_store_lifetime() {
    let tmp = Temp::new();
    let barrier = std::sync::Barrier::new(4);
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                scope.spawn(|| {
                    barrier.wait();
                    let first = LocalBatchStore::open(&tmp.0).unwrap();
                    // Keep first alive while opening again; a dup-based init flock
                    // would remain owned and block this second independent open.
                    let second = LocalBatchStore::open_readonly(&tmp.0).unwrap().unwrap();
                    assert_eq!(first.snapshot().unwrap(), second.snapshot().unwrap());
                })
            })
            .collect();
        for handle in handles {
            handle.join().unwrap();
        }
    });
}
