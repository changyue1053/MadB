use super::{
    conflict, failure, filesystem as fs, host,
    staging::{PublishError, Stage, nonce},
};
use madb_app::use_cases::initialize::{
    AbsolutePath, FileIdentity, InitEvidencePort, SavedEvidenceObservation,
};
use madb_app::{Digest, ErrorKind, Result, ports::FileReference};
use rustix::fs::{AtFlags, OFlags, RenameFlags, mkdirat, openat, renameat_with, unlinkat};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs::File,
    io::Write,
};

/// One independently created private evidence root. `store_file` admits exact
/// caller-pinned bytes; saving a result rereads all required files through the
/// retained directory and never treats expected hashes as observations.
pub struct LocalInitEvidence {
    binding: fs::ParentBinding,
    directory: File,
    identity: fs::Identity,
    stored: BTreeMap<String, FileReference>,
    directories: BTreeSet<String>,
    saved: bool,
}
impl LocalInitEvidence {
    pub fn create(root: &AbsolutePath) -> Result<Self> {
        Self::create_attempt(root).map_err(PublishError::into_error)
    }
    fn create_attempt(root: &AbsolutePath) -> std::result::Result<Self, PublishError> {
        let binding = fs::ParentBinding::open(root)?;
        let mut stage = Stage::create(&binding)?;
        stage.sync()?;
        binding.verify()?;
        stage.verify_name()?;
        stage.publish_exclusive(&binding)?;
        let directory = stage
            .directory
            .try_clone()
            .map_err(|e| host("evidence", e))?;
        let identity = fs::identity(&directory)?;
        stage.verify_published(&binding)?;
        binding
            .parent
            .sync_all()
            .map_err(|e| host("evidence parent", e))?;
        Ok(Self {
            binding,
            directory,
            identity,
            stored: BTreeMap::new(),
            directories: BTreeSet::new(),
            saved: false,
        })
    }
    /// The composition root chooses an already accepted internal parent; this
    /// creates a fresh attempt name without a new user-facing path option.
    pub fn allocate(parent: &AbsolutePath) -> Result<Self> {
        for _ in 0..128 {
            let root = AbsolutePath::new(&format!(
                "{}/madb-init-evidence-{}",
                parent.as_str(),
                nonce()
            ))?;
            match Self::create_attempt(&root) {
                Err(PublishError::Existing(_)) => continue,
                Err(PublishError::Other(error)) => return Err(error),
                Ok(created) => return Ok(created),
            }
        }
        Err(conflict(
            "evidence",
            "fresh evidence directory name collision budget exhausted",
        ))
    }
    pub fn root(&self) -> &AbsolutePath {
        &self.binding.root
    }
    pub fn result_was_saved(&self) -> bool {
        self.saved
    }
    /// Copy only registered, independently reread evidence into a new private
    /// root. A previous immutable result is never copied or overwritten.
    pub fn fork_verified(&self) -> Result<Self> {
        use madb_app::ports::ReadWorkspace;
        self.verify_root()?;
        let parent = std::path::Path::new(self.root().as_str())
            .parent()
            .and_then(|p| p.to_str())
            .ok_or_else(|| conflict("evidence", "evidence parent is absent"))?;
        let mut fork = Self::allocate(&AbsolutePath::new(parent)?)?;
        let reader = crate::LocalWorkspace {
            root: self
                .directory
                .try_clone()
                .map_err(|e| host("evidence", e))?,
        };
        for reference in self.stored.values() {
            fs::read_reference(&self.directory, reference)?;
            let bytes = reader
                .read(&reference.path, reference.size as usize)?
                .ok_or_else(|| {
                    conflict(reference.path.as_str(), "registered evidence is missing")
                })?;
            if bytes.len() as u64 != reference.size || Digest::of_bytes(&bytes) != reference.digest
            {
                return Err(conflict(
                    reference.path.as_str(),
                    "evidence changed while copying",
                ));
            }
            fs::read_reference(&self.directory, reference)?;
            fork.store_file(reference, &bytes)?;
        }
        self.verify_root()?;
        Ok(fork)
    }
    fn verify_root(&self) -> Result<()> {
        self.binding.verify()?;
        if fs::entry_identity(&self.binding.parent, self.binding.root.basename())? != self.identity
        {
            return Err(conflict("evidence", "evidence root was replaced"));
        }
        Ok(())
    }
    pub fn store_file(&mut self, reference: &FileReference, bytes: &[u8]) -> Result<()> {
        let path = reference.path.as_str();
        if self.saved || path.eq_ignore_ascii_case("init-result.json") {
            return Err(conflict(
                path,
                "result path is reserved and completed evidence cannot be extended",
            ));
        }
        if bytes.len() as u64 != reference.size || Digest::of_bytes(bytes) != reference.digest {
            return Err(failure(
                ErrorKind::InvalidInput,
                path,
                "evidence bytes differ from their supplied identity",
            ));
        }
        let lower = path.to_ascii_lowercase();
        for existing in self.stored.keys().chain(self.directories.iter()) {
            let old = existing.to_ascii_lowercase();
            if old == lower
                || old.starts_with(&format!("{lower}/"))
                || (self.stored.contains_key(existing) && lower.starts_with(&format!("{old}/")))
            {
                return Err(conflict(path, "duplicate or aliased evidence path"));
            }
        }
        let mut parents = BTreeSet::new();
        fs::add_parents(path, &mut parents);
        for parent in &parents {
            if self
                .directories
                .iter()
                .any(|old| old != parent && old.eq_ignore_ascii_case(parent))
            {
                return Err(conflict(path, "evidence parent has a spelling alias"));
            }
        }
        self.verify_root()?;
        for path in parents {
            if self.directories.contains(&path) {
                continue;
            }
            let (parent, name) = fs::parent_and_name(&self.directory, &path)?;
            mkdirat(&parent, name.as_str(), fs::PRIVATE).map_err(|e| conflict(&path, e))?;
            fs::directory(&parent, &name)?
                .sync_all()
                .map_err(|e| host(&path, e))?;
            self.directories.insert(path);
        }
        let (parent, name) = fs::parent_and_name(&self.directory, path)?;
        write_new(&parent, &name, bytes)?;
        self.stored.insert(path.to_owned(), reference.clone());
        fs::read_reference(&self.directory, reference)?;
        self.verify_root()
    }
}
impl InitEvidencePort for LocalInitEvidence {
    fn save_result(
        &mut self,
        root: &AbsolutePath,
        result_bytes: &[u8],
        required_files: &[FileIdentity],
    ) -> Result<SavedEvidenceObservation> {
        if self.saved || root != self.root() {
            return Err(conflict(
                "evidence",
                "evidence root differs or result was already saved",
            ));
        }
        self.verify_root()?;
        let mut seen = BTreeSet::new();
        for expected in required_files {
            let path = expected.path.as_str();
            if !seen.insert(path) || path.eq_ignore_ascii_case("init-result.json") {
                return Err(failure(
                    ErrorKind::InvalidInput,
                    path,
                    "duplicate or reserved required evidence path",
                ));
            }
            let actual = self.stored.get(path).ok_or_else(|| {
                conflict(path, "required evidence was not stored by this attempt")
            })?;
            if expected.sha256 != actual.digest || expected.size != actual.size {
                return Err(conflict(
                    path,
                    "required evidence differs from the frozen stored identity",
                ));
            }
        }
        if seen.len() != self.stored.len() {
            return Err(conflict(
                "evidence",
                "required evidence must name the complete stored inventory",
            ));
        }
        let all: Vec<_> = self.stored.values().cloned().collect();
        fs::verify_tree(&self.directory, &all, &self.directories)?;
        let result = FileReference {
            path: fs::logical("init-result.json")?,
            digest: Digest::of_bytes(result_bytes),
            size: result_bytes.len() as u64,
        };
        write_new(&self.directory, "init-result.json", result_bytes)?;
        self.saved = true; // Even a subsequent verification failure never retries/overwrites.
        let mut observed = Vec::new();
        for expected in required_files {
            let reference = self
                .stored
                .get(expected.path.as_str())
                .expect("required inventory checked");
            let actual = fs::read_reference(&self.directory, reference)?;
            observed.push(FileIdentity {
                path: actual.path,
                sha256: actual.digest,
                size: actual.size,
            });
        }
        let actual = fs::read_reference(&self.directory, &result)?;
        observed.push(FileIdentity {
            path: actual.path,
            sha256: actual.digest,
            size: actual.size,
        });
        self.verify_root()?;
        observed.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(SavedEvidenceObservation {
            root: self.root().clone(),
            files: observed,
        })
    }
}

fn write_new(parent: &File, name: &str, bytes: &[u8]) -> Result<()> {
    let temporary = format!(".madb-evidence-pending-{}", nonce());
    let mut file = File::from(
        openat(
            parent,
            temporary.as_str(),
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
            fs::FILE_MODE,
        )
        .map_err(|e| conflict(name, e))?,
    );
    let result = (|| {
        file.write_all(bytes).map_err(|e| host(name, e))?;
        file.sync_all().map_err(|e| host(name, e))?;
        if fs::entry_identity(parent, &temporary)? != fs::identity(&file)? {
            return Err(conflict(name, "private evidence temporary was replaced"));
        }
        renameat_with(
            parent,
            temporary.as_str(),
            parent,
            name,
            RenameFlags::NOREPLACE,
        )
        .map_err(|e| match e {
            rustix::io::Errno::EXIST | rustix::io::Errno::NOTEMPTY => {
                conflict(name, "evidence file already exists; not overwritten")
            }
            _ => host(name, e),
        })?;
        parent.sync_all().map_err(|e| host(name, e))
    })();
    if result.is_err() && fs::entry_identity(parent, &temporary).ok() == fs::identity(&file).ok() {
        let _ = unlinkat(parent, temporary.as_str(), AtFlags::empty());
    }
    result
}
