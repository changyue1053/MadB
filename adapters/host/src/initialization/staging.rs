use super::{conflict, filesystem as fs, host};
use madb_app::Result;
use rustix::fs::{AtFlags, OFlags, RenameFlags, mkdirat, openat, renameat_with, unlinkat};
use std::{
    collections::BTreeMap,
    fs::File,
    io::Write,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

static NEXT: AtomicU64 = AtomicU64::new(0);
pub(super) fn nonce() -> String {
    format!(
        "{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    )
}

pub(super) struct Stage {
    parent: File,
    pub directory: File,
    pub path: PathBuf,
    name: String,
    identity: fs::Identity,
    files: BTreeMap<String, fs::Identity>,
    directories: BTreeMap<String, fs::Identity>,
    published: bool,
}
pub(super) enum PublishError {
    Existing(madb_app::QueryError),
    Other(madb_app::QueryError),
}
impl PublishError {
    pub fn into_error(self) -> madb_app::QueryError {
        match self {
            Self::Existing(error) | Self::Other(error) => error,
        }
    }
}
impl From<madb_app::QueryError> for PublishError {
    fn from(error: madb_app::QueryError) -> Self {
        Self::Other(error)
    }
}
impl Stage {
    pub fn create(binding: &fs::ParentBinding) -> Result<Self> {
        binding.verify()?;
        let parent = binding.parent.try_clone().map_err(|e| host("parent", e))?;
        for _ in 0..128 {
            let name = format!(".madb-init-stage-{}", nonce());
            match mkdirat(&parent, name.as_str(), fs::PRIVATE) {
                Err(rustix::io::Errno::EXIST) => continue,
                Err(error) => return Err(host("staging", error)),
                Ok(()) => {}
            }
            let directory = fs::directory(&parent, &name)?;
            let path = PathBuf::from(binding.root.as_str()).with_file_name(&name);
            return Ok(Self {
                identity: fs::identity(&directory)?,
                parent,
                directory,
                path,
                name,
                files: BTreeMap::new(),
                directories: BTreeMap::new(),
                published: false,
            });
        }
        Err(conflict(
            "staging",
            "private staging name collisions exhausted the bounded retry",
        ))
    }
    pub fn directory(&mut self, path: &str) -> Result<()> {
        let (parent, name) = fs::parent_and_name(&self.directory, path)?;
        mkdirat(&parent, name.as_str(), fs::PRIVATE).map_err(|e| host(path, e))?;
        let file = fs::directory(&parent, &name)?;
        self.directories
            .insert(path.to_owned(), fs::identity(&file)?);
        Ok(())
    }
    pub fn write(&mut self, path: &str, bytes: &[u8]) -> Result<()> {
        self.write_mode(path, bytes, fs::FILE_MODE)
    }
    pub fn write_project_wrapper(&mut self, bytes: &[u8]) -> Result<()> {
        self.write_mode("madbw", bytes, rustix::fs::Mode::from_bits_retain(0o755))
    }
    fn write_mode(&mut self, path: &str, bytes: &[u8], mode: rustix::fs::Mode) -> Result<()> {
        let (parent, name) = fs::parent_and_name(&self.directory, path)?;
        let mut file = File::from(
            openat(
                &parent,
                name.as_str(),
                OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::NOFOLLOW | OFlags::CLOEXEC,
                mode,
            )
            .map_err(|e| conflict(path, e))?,
        );
        if path == "madbw" && mode == rustix::fs::Mode::from_bits_retain(0o755) {
            rustix::fs::fchmod(&file, mode).map_err(|e| host(path, e))?;
        }
        self.files.insert(path.to_owned(), fs::identity(&file)?);
        file.write_all(bytes).map_err(|e| host(path, e))?;
        file.sync_all().map_err(|e| host(path, e))
    }
    pub fn verify_name(&self) -> Result<()> {
        if fs::entry_identity(&self.parent, &self.name)? != self.identity {
            return Err(conflict("staging", "private staging entry was replaced"));
        }
        Ok(())
    }
    pub fn sync(&self) -> Result<()> {
        for path in self.directories.keys().rev() {
            fs::relative_directory(&self.directory, path)?
                .sync_all()
                .map_err(|e| host(path, e))?;
        }
        self.directory.sync_all().map_err(|e| host("staging", e))
    }
    /// Called only inside CommitPermit::publish. One bounded OS operation, no
    /// user callbacks or path traversal while holding its cancellation mutex.
    pub fn publish(&mut self, binding: &fs::ParentBinding) -> Result<()> {
        self.publish_exclusive(binding)
            .map_err(PublishError::into_error)
    }
    pub fn publish_exclusive(
        &mut self,
        binding: &fs::ParentBinding,
    ) -> std::result::Result<(), PublishError> {
        renameat_with(
            &self.parent,
            self.name.as_str(),
            &binding.parent,
            binding.root.basename(),
            RenameFlags::NOREPLACE,
        )
        .map_err(|e| match e {
            rustix::io::Errno::EXIST | rustix::io::Errno::NOTEMPTY => {
                PublishError::Existing(conflict(
                    binding.root.as_str(),
                    "output already exists; nothing was overwritten",
                ))
            }
            _ => PublishError::Other(host(binding.root.as_str(), e)),
        })?;
        self.published = true;
        Ok(())
    }
    pub fn verify_published(&self, binding: &fs::ParentBinding) -> Result<()> {
        binding.verify()?;
        if fs::entry_identity(&binding.parent, binding.root.basename())? != self.identity {
            return Err(conflict(
                binding.root.as_str(),
                "created root was replaced after publication",
            ));
        }
        Ok(())
    }
    /// Only owned identities are removed. An externally replaced or injected
    /// entry leaves private staging for inspection rather than deleting it.
    pub fn discard(&mut self) -> Result<()> {
        if self.published {
            return Ok(());
        }
        self.verify_name()?;
        for (path, expected) in self.files.iter().rev() {
            let (parent, name) = fs::parent_and_name(&self.directory, path)?;
            if fs::entry_identity(&parent, &name)? != *expected {
                return Err(conflict(path, "staging cleanup retained a replaced file"));
            }
            unlinkat(&parent, name.as_str(), AtFlags::empty()).map_err(|e| host(path, e))?;
        }
        self.files.clear();
        for (path, expected) in self.directories.iter().rev() {
            let (parent, name) = fs::parent_and_name(&self.directory, path)?;
            if fs::entry_identity(&parent, &name)? != *expected {
                return Err(conflict(
                    path,
                    "staging cleanup retained a replaced directory",
                ));
            }
            unlinkat(&parent, name.as_str(), AtFlags::REMOVEDIR).map_err(|e| host(path, e))?;
        }
        self.directories.clear();
        unlinkat(&self.parent, self.name.as_str(), AtFlags::REMOVEDIR)
            .map_err(|e| host("staging cleanup", e))?;
        self.published = true; // Disarmed: there is no longer an owned entry.
        Ok(())
    }
}
impl Drop for Stage {
    fn drop(&mut self) {
        let _ = self.discard();
    }
}
