//! Local native-preparation cache. Stored documents never mint a dependency
//! proof. A fresh supported snapshot is required for every lookup; corruption
//! is a miss, and damaged entries are retained instead of repaired in place.
use crate::{LocalWorkspace, inputs::PreparedRun};
use madb_app::{
    Digest, ErrorKind, LogicalPath, QueryError, Result,
    dependencies::{CacheManifest, DependencyFormat, DependencySnapshot},
    input_observation::ObserveWorkspace,
    inputs::{FrozenInputs, capture_inputs},
    ports::{FileReference, ReadWorkspace},
};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

pub struct LocalDependencyCache {
    path: PathBuf,
    root: Option<LocalWorkspace>,
    read_only: bool,
}
impl LocalDependencyCache {
    /// A read-only cache never creates its directory or a lock. Missing root is
    /// a normal miss. The caller grants a stable trusted root parent directory.
    pub fn open(root: &Path, read_only: bool) -> Result<Self> {
        if !read_only {
            std::fs::create_dir_all(root).map_err(cache_error)?;
        }
        let workspace = match std::fs::symlink_metadata(root) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound && read_only => None,
            Err(e) => return Err(cache_error(e)),
            Ok(meta) if meta.is_dir() => {
                let workspace = LocalWorkspace::open(root)?;
                if cache_owner(&workspace, read_only)? {
                    Some(workspace)
                } else {
                    None
                }
            }
            Ok(_) => return Err(cache_error("cache root must be a directory, not a symlink")),
        };
        Ok(Self {
            path: root.to_path_buf(),
            root: workspace,
            read_only,
        })
    }
    pub fn entry_path(&self, snapshot: &DependencySnapshot) -> PathBuf {
        self.path.join(key_name(snapshot))
    }
    pub fn load<W: ObserveWorkspace, F: DependencyFormat>(
        &self,
        source: &W,
        snapshot: &DependencySnapshot,
        expected_outputs: &[LogicalPath],
        format: &F,
    ) -> Result<Option<FrozenInputs>> {
        // Workspace errors are not cache misses. Otherwise a changed source
        // could silently turn a refused stale proof into an executable miss.
        snapshot.verify(source)?;
        let result = self.read_entry(snapshot, expected_outputs, format).ok();
        snapshot.verify(source)?;
        Ok(result)
    }
    #[cfg(unix)]
    fn read_entry<F: DependencyFormat>(
        &self,
        snapshot: &DependencySnapshot,
        expected_outputs: &[LogicalPath],
        format: &F,
    ) -> Result<FrozenInputs> {
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| cache_error("cache is absent"))?;
        let entry = directory(&root.root, key_name(snapshot))?;
        let work = LocalWorkspace {
            root: directory(&entry, "work")?,
        };
        let inputs = LocalWorkspace {
            root: directory(&entry, "inputs")?,
        };
        let metadata = work
            .read(&logical("cache.json"), 1024 * 1024)?
            .ok_or_else(|| cache_error("cache manifest missing"))?;
        let manifest = format.decode_cache(&metadata)?;
        if &manifest.input_key != snapshot.key()
            || manifest.dependency_manifest.path != logical("dependency.json")
        {
            return Err(cache_error("cache identity differs"));
        }
        let dependency = work
            .read(&logical("dependency.json"), 1024 * 1024)?
            .ok_or_else(|| cache_error("dependency manifest missing"))?;
        if manifest.dependency_manifest.size != dependency.len() as u64
            || manifest.dependency_manifest.digest != Digest::of_bytes(&dependency)
            || dependency != snapshot.manifest_bytes()
        {
            return Err(cache_error("dependency manifest differs"));
        }
        // Parse as well as matching bytes: an unknown/unsupported stored format
        // is not admissible even if a caller supplies identical byte identity.
        let decoded = format.decode_dependencies(&dependency)?;
        if &decoded != snapshot.manifest() {
            return Err(cache_error("dependency format mismatch"));
        }
        let actual: BTreeSet<_> = manifest.files.iter().map(|file| &file.path).collect();
        let expected: BTreeSet<_> = expected_outputs.iter().collect();
        if actual != expected
            || actual.len() != manifest.files.len()
            || expected.len() != expected_outputs.len()
        {
            return Err(cache_error("cache output inventory differs"));
        }
        let files = capture_inputs(&inputs, manifest.files)?;
        crate::inputs::tree::verify_contents(&inputs, &files)?;
        Ok(files)
    }
    #[cfg(not(unix))]
    fn read_entry<F: DependencyFormat>(
        &self,
        _: &DependencySnapshot,
        _: &[LogicalPath],
        _: &F,
    ) -> Result<FrozenInputs> {
        Err(cache_error("cache requires Unix host"))
    }

    #[cfg(unix)]
    pub fn store<F: DependencyFormat>(
        &self,
        snapshot: &DependencySnapshot,
        files: &FrozenInputs,
        format: &F,
    ) -> Result<()> {
        use rustix::fs::{Mode, OFlags, openat, renameat};
        use std::os::unix::fs::MetadataExt;
        if self.read_only {
            return Err(cache_error("read-only cache cannot store"));
        }
        let root = self
            .root
            .as_ref()
            .ok_or_else(|| cache_error("cache root absent"))?;
        crate::inputs::verify_directory(&self.path, &root.root)?;
        let lock = std::fs::File::from(
            openat(
                &root.root,
                ".lock",
                OFlags::RDWR
                    | OFlags::CREATE
                    | OFlags::CLOEXEC
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK,
                Mode::from_bits_retain(0o600),
            )
            .map_err(cache_error)?,
        );
        let meta = lock.metadata().map_err(cache_error)?;
        if !meta.is_file() || meta.nlink() != 1 {
            return Err(cache_error("cache lock must be a private regular file"));
        }
        lock_exclusive(&lock)?;
        let expected: Vec<_> = files.references().map(|file| file.path.clone()).collect();
        if let Ok(existing) = self.read_entry(snapshot, &expected, format) {
            if existing.references().ne(files.references()) {
                return Err(QueryError::new(
                    ErrorKind::Conflict,
                    "CACHE002",
                    "cache",
                    "same input identity produced different output bytes",
                ));
            }
            return Ok(());
        }
        // We only quarantine an actual directory. A symbolic link or other
        // unowned object at the key path is rejected, never followed/replaced.
        match directory(&root.root, key_name(snapshot)) {
            Ok(_) => {
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let sequence = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let stamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(cache_error)?
                    .as_nanos();
                let quarantine = format!("quarantine-{}-{stamp}-{sequence}", std::process::id());
                renameat(
                    &root.root,
                    key_name(snapshot),
                    &root.root,
                    quarantine.as_str(),
                )
                .map_err(cache_error)?;
            }
            Err(_) => {
                use rustix::fs::{AtFlags, statat};
                match statat(&root.root, key_name(snapshot), AtFlags::SYMLINK_NOFOLLOW) {
                    Err(rustix::io::Errno::NOENT) => {}
                    _ => return Err(cache_error("cache key is not an owned readable directory")),
                }
            }
        }
        // Exclusive create; partial publication has no cache.json and is a miss.
        let prepared = PreparedRun::create(&self.path, &logical(key_name(snapshot)), files.clone())
            .map_err(|e| e.error)?;
        let entry = directory(&root.root, key_name(snapshot))?;
        let work = directory(&entry, "work")?;
        let dependency = snapshot.manifest_bytes();
        write_new(&work, "dependency.json", dependency)?;
        let manifest = CacheManifest {
            input_key: snapshot.key().clone(),
            dependency_manifest: FileReference {
                path: logical("dependency.json"),
                digest: Digest::of_bytes(dependency),
                size: dependency.len() as u64,
            },
            files: files.references().cloned().collect(),
        };
        let bytes = format.encode_cache(&manifest)?;
        write_new(&work, "cache.pending", &bytes)?;
        renameat(&work, "cache.pending", &work, "cache.json").map_err(cache_error)?;
        work.sync_all().map_err(cache_error)?;
        entry.sync_all().map_err(cache_error)?;
        root.root.sync_all().map_err(cache_error)?;
        let verified = self.read_entry(snapshot, &expected, format)?;
        if verified.references().ne(files.references()) {
            return Err(cache_error("cache publication failed verification"));
        }
        // PreparedRun retains owned immutable data; Drop closes only descriptors.
        drop(prepared);
        Ok(())
    }
    #[cfg(not(unix))]
    pub fn store<F: DependencyFormat>(
        &self,
        _: &DependencySnapshot,
        _: &FrozenInputs,
        _: &F,
    ) -> Result<()> {
        Err(cache_error("cache requires Unix host"))
    }
}
fn key_name(snapshot: &DependencySnapshot) -> &str {
    snapshot
        .key()
        .as_str()
        .strip_prefix("sha256:")
        .expect("validated digest")
}
fn logical(path: &str) -> LogicalPath {
    LogicalPath::new(path).expect("validated cache path")
}
fn cache_error(error: impl std::fmt::Display) -> QueryError {
    QueryError::new(ErrorKind::Host, "CACHE001", "cache", error.to_string())
}
#[cfg(unix)]
fn directory(root: &std::fs::File, name: &str) -> Result<std::fs::File> {
    use rustix::fs::{Mode, OFlags, openat};
    Ok(std::fs::File::from(
        openat(
            root,
            name,
            OFlags::RDONLY
                | OFlags::DIRECTORY
                | OFlags::CLOEXEC
                | OFlags::NOFOLLOW
                | OFlags::NONBLOCK,
            Mode::empty(),
        )
        .map_err(cache_error)?,
    ))
}
#[cfg(unix)]
fn write_new(root: &std::fs::File, name: &str, bytes: &[u8]) -> Result<()> {
    use rustix::fs::{Mode, OFlags, openat};
    use std::io::Write;
    let mut file = std::fs::File::from(
        openat(
            root,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::from_bits_retain(0o400),
        )
        .map_err(cache_error)?,
    );
    file.write_all(bytes).map_err(cache_error)?;
    file.sync_all().map_err(cache_error)
}
#[cfg(unix)]
fn lock_exclusive(file: &std::fs::File) -> Result<()> {
    use rustix::fs::{FlockOperation, flock};
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    loop {
        match flock(file, FlockOperation::NonBlockingLockExclusive) {
            Ok(()) => return Ok(()),
            Err(rustix::io::Errno::WOULDBLOCK) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(10))
            }
            Err(e) => return Err(cache_error(e)),
        }
    }
}

#[cfg(unix)]
fn cache_owner(workspace: &LocalWorkspace, read_only: bool) -> Result<bool> {
    const OWNER: &[u8] = b"madb.native-preparation-cache/1\n";
    let marker = logical(".madb-cache-v1");
    match workspace.read(&marker, 128) {
        Ok(Some(bytes)) if bytes == OWNER => return Ok(true),
        Ok(None) => {}
        _ if read_only => return Ok(false),
        _ => return Err(cache_error("cache owner marker is malformed or unsafe")),
    }
    if read_only {
        return Ok(false);
    }
    let mut entries = rustix::fs::Dir::read_from(&workspace.root).map_err(cache_error)?;
    for entry in &mut entries {
        let entry = entry.map_err(cache_error)?;
        let name = entry.file_name().to_bytes();
        if name != b"." && name != b".." {
            return Err(cache_error(
                "refusing to initialize an unowned nonempty cache directory",
            ));
        }
    }
    match write_new(&workspace.root, ".madb-cache-v1", OWNER) {
        Ok(()) => {}
        Err(error) => {
            if workspace.read(&marker, 128)?.as_deref() != Some(OWNER) {
                return Err(error);
            }
        }
    }
    workspace.root.sync_all().map_err(cache_error)?;
    Ok(true)
}
#[cfg(not(unix))]
fn cache_owner(_: &LocalWorkspace, _: bool) -> Result<bool> {
    Err(cache_error("cache requires Unix host"))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    struct Scratch(PathBuf);
    impl Scratch {
        fn new() -> Self {
            static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "madb-cache-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Scratch {
        fn drop(&mut self) {
            std::fs::remove_dir_all(&self.0).unwrap();
        }
    }
    #[test]
    fn read_only_missing_cache_creates_nothing() {
        let scratch = Scratch::new();
        let path = scratch.0.join("missing");
        let cache = LocalDependencyCache::open(&path, true).unwrap();
        assert!(cache.root.is_none());
        assert!(!path.exists());
    }
    #[test]
    fn nonempty_unowned_cache_is_preserved() {
        let scratch = Scratch::new();
        std::fs::write(scratch.0.join("user.txt"), b"keep").unwrap();
        assert!(LocalDependencyCache::open(&scratch.0, false).is_err());
        assert_eq!(std::fs::read(scratch.0.join("user.txt")).unwrap(), b"keep");
        assert!(!scratch.0.join(".lock").exists());
        assert!(!scratch.0.join(".madb-cache-v1").exists());
        let readonly = LocalDependencyCache::open(&scratch.0, true).unwrap();
        assert!(readonly.root.is_none());
    }
    #[test]
    fn empty_cache_claims_once_and_readonly_does_not_create_lock() {
        let scratch = Scratch::new();
        LocalDependencyCache::open(&scratch.0, false).unwrap();
        let bytes = std::fs::read(scratch.0.join(".madb-cache-v1")).unwrap();
        LocalDependencyCache::open(&scratch.0, true).unwrap();
        assert_eq!(
            std::fs::read(scratch.0.join(".madb-cache-v1")).unwrap(),
            bytes
        );
        assert!(!scratch.0.join(".lock").exists());
    }
    #[test]
    fn cache_root_symlink_is_never_claimed() {
        let scratch = Scratch::new();
        let target = scratch.0.join("target");
        std::fs::create_dir(&target).unwrap();
        let link = scratch.0.join("link");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        assert!(LocalDependencyCache::open(&link, false).is_err());
        assert!(!target.join(".madb-cache-v1").exists());
    }
}
