//! Explicit filesystem conditions used around one graph execution. These
//! observations neither discover dependencies nor authorize caching or commits.
use std::collections::{BTreeMap, BTreeSet};

use crate::{ErrorKind, LogicalPath, QueryError, Result, ports::ReadWorkspace};

const ABSENT_LIMIT: usize = 4096;
const DIRECTORY_LIMIT: usize = 256;
const PREFIX_LIMIT: usize = 8192;
const MEMBER_LIMIT: usize = 8192;
const OBSERVED_PREFIX_LIMIT: usize = PREFIX_LIMIT + MEMBER_LIMIT;
const NAME_BYTES_LIMIT: usize = 2 * 1024 * 1024;
const PATH_DEPTH_LIMIT: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DirectoryEntryKind {
    File,
    Directory,
}

/// A single immediate child, never a relative subtree. Host rejects links and
/// unsupported kinds; app additionally checks names, aliases and returned bounds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryEntry {
    pub name: LogicalPath,
    pub kind: DirectoryEntryKind,
}

/// Bounded read-only observations. Exists must inspect the final entry without
/// following links: even a dangling link is present. Missing ancestors may mean
/// false, but unreadable, linked or non-directory ancestors must be errors.
/// Listing returns every immediate entry, rejecting links/special files rather
/// than filtering them. Limits apply before allocations and are checked by app.
pub trait ObserveWorkspace: ReadWorkspace {
    fn exists(&self, path: &LogicalPath) -> Result<bool>;
    fn list_directory(
        &self,
        path: &LogicalPath,
        entry_limit: usize,
        name_bytes_limit: usize,
    ) -> Result<Vec<DirectoryEntry>>;
}

/// Caller-declared absent paths and direct directory memberships. Nested
/// directories are legal and remain separate nonrecursive observations.
#[derive(Clone, Debug)]
pub struct InputWatchSet {
    absent: Vec<LogicalPath>,
    directories: Vec<LogicalPath>,
}
impl InputWatchSet {
    pub fn new(mut absent: Vec<LogicalPath>, mut directories: Vec<LogicalPath>) -> Result<Self> {
        if absent.len() > ABSENT_LIMIT || directories.len() > DIRECTORY_LIMIT {
            return Err(budget(
                "watch",
                "watch exceeds 4096 absent paths or 256 directories",
            ));
        }
        absent.sort();
        directories.sort();
        if absent.windows(2).any(|pair| pair[0] == pair[1])
            || directories.windows(2).any(|pair| pair[0] == pair[1])
        {
            return Err(structure("watch", "duplicate watch declaration"));
        }
        let watch = Self {
            absent,
            directories,
        };
        watch.validate_files(std::iter::empty())?;
        Ok(watch)
    }
    pub fn absent(&self) -> &[LogicalPath] {
        &self.absent
    }
    pub fn directories(&self) -> &[LogicalPath] {
        &self.directories
    }

    /// Include the selected external files before any observed graph IO. Files
    /// imply existing parent directories; an absent path may not hide one.
    pub(crate) fn validate_files<'a>(
        &self,
        files: impl IntoIterator<Item = &'a LogicalPath>,
    ) -> Result<()> {
        let mut paths = PathRequirements::new(PREFIX_LIMIT);
        for path in &self.absent {
            paths.insert(path, ABSENT)?;
        }
        for path in &self.directories {
            paths.insert(path, DIRECTORY)?;
        }
        for path in files {
            paths.insert(path, FILE)?;
        }
        Ok(())
    }
}

/// Captured conditions, not an atomic snapshot or a persistent receipt. A
/// directory's file contents are intentionally absent from this value.
#[derive(Clone, Debug)]
pub struct InputObservation {
    absent: Vec<LogicalPath>,
    directories: BTreeMap<LogicalPath, Vec<DirectoryEntry>>,
}
impl InputObservation {
    /// Capture all declared conditions and immediately recheck them. Host errors
    /// during the first capture retain their diagnostic; no partial value escapes.
    pub fn capture<W: ObserveWorkspace + ?Sized>(
        workspace: &W,
        watch: &InputWatchSet,
    ) -> Result<Self> {
        for path in watch.absent() {
            if workspace.exists(path)? {
                return Err(QueryError::new(
                    ErrorKind::InvalidInput,
                    "OBS003",
                    path.as_str(),
                    "expected absent path already exists",
                ));
            }
        }
        let mut remaining = ListingBudget::default();
        let mut directories = BTreeMap::new();
        for path in watch.directories() {
            directories.insert(path.clone(), listing(workspace, path, &mut remaining)?);
        }
        let observation = Self {
            absent: watch.absent.clone(),
            directories,
        };
        observation.validate_files(std::iter::empty())?;
        observation.verify(workspace)?;
        Ok(observation)
    }

    /// Refuse captured contradictions, including omissions from a directory
    /// advertised as complete. This is consistency of observations, not an
    /// assertion that all reads happened atomically.
    pub(crate) fn validate_files<'a>(
        &self,
        files: impl IntoIterator<Item = &'a LogicalPath>,
    ) -> Result<()> {
        let files: Vec<_> = files.into_iter().collect();
        let mut paths = PathRequirements::new(OBSERVED_PREFIX_LIMIT);
        for absent in &self.absent {
            paths.insert(absent, ABSENT)?;
        }
        for directory in self.directories.keys() {
            paths.insert(directory, DIRECTORY)?;
        }
        for file in &files {
            paths.insert(file, FILE)?;
        }
        for (directory, entries) in &self.directories {
            for entry in entries {
                let full =
                    LogicalPath::new(format!("{}/{}", directory.as_str(), entry.name.as_str()))
                        .map_err(|_| {
                            structure(
                                directory.as_str(),
                                "observed member path exceeds logical path limits",
                            )
                        })?;
                paths.insert(
                    &full,
                    match entry.kind {
                        DirectoryEntryKind::File => FILE,
                        DirectoryEntryKind::Directory => DIRECTORY,
                    },
                )?;
            }
        }
        for (directory, entries) in &self.directories {
            let prefix = format!("{}/", directory.as_str());
            for (required, kind) in self
                .directories
                .keys()
                .map(|path| (path, DirectoryEntryKind::Directory))
                .chain(files.iter().map(|path| (*path, DirectoryEntryKind::File)))
            {
                let Some(relative) = required.as_str().strip_prefix(&prefix) else {
                    continue;
                };
                let (child, kind) = match relative.split_once('/') {
                    Some((child, _)) => (child, DirectoryEntryKind::Directory),
                    None => (relative, kind),
                };
                let actual = entries
                    .binary_search_by(|entry| entry.name.as_str().cmp(child))
                    .ok()
                    .map(|index| &entries[index]);
                if actual.is_none_or(|entry| entry.kind != kind) {
                    return Err(structure(
                        required.as_str(),
                        "complete directory observation omits a required child or its kind",
                    ));
                }
            }
        }
        Ok(())
    }

    /// Recheck membership, kinds and absence. Reads are bounded and sequential;
    /// no guarantee is made about ABA changes or mutations after this returns.
    pub fn verify<W: ObserveWorkspace + ?Sized>(&self, workspace: &W) -> Result<()> {
        for path in &self.absent {
            match workspace.exists(path) {
                Ok(false) => {}
                Ok(true) => return Err(changed(path, "expected absent path appeared")),
                Err(error) => return Err(changed(path, format!("cannot verify absence: {error}"))),
            }
        }
        let mut remaining = ListingBudget::default();
        for (path, expected) in &self.directories {
            let actual = listing(workspace, path, &mut remaining)
                .map_err(|error| changed(path, format!("cannot verify directory: {error}")))?;
            if &actual != expected {
                return Err(changed(path, "directory membership or entry kind changed"));
            }
        }
        Ok(())
    }
}

struct ListingBudget {
    members: usize,
    name_bytes: usize,
}
impl Default for ListingBudget {
    fn default() -> Self {
        Self {
            members: MEMBER_LIMIT,
            name_bytes: NAME_BYTES_LIMIT,
        }
    }
}
fn listing<W: ObserveWorkspace + ?Sized>(
    workspace: &W,
    path: &LogicalPath,
    remaining: &mut ListingBudget,
) -> Result<Vec<DirectoryEntry>> {
    let mut entries = workspace.list_directory(path, remaining.members, remaining.name_bytes)?;
    if entries.len() > remaining.members {
        return Err(budget(
            path.as_str(),
            "workspace returned too many directory members",
        ));
    }
    let mut names = BTreeSet::new();
    let mut bytes = 0usize;
    for entry in &entries {
        let name = entry.name.as_str();
        if !name.is_ascii() || name.contains('/') {
            return Err(structure(
                path.as_str(),
                "directory members must be single ASCII components",
            ));
        }
        if !names.insert(name.to_ascii_lowercase()) {
            return Err(structure(
                path.as_str(),
                "duplicate or case-aliased directory member",
            ));
        }
        bytes = bytes
            .checked_add(name.len())
            .filter(|size| *size <= remaining.name_bytes)
            .ok_or_else(|| {
                budget(
                    path.as_str(),
                    "workspace returned directory names beyond the byte budget",
                )
            })?;
    }
    remaining.members -= entries.len();
    remaining.name_bytes -= bytes;
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

// Each prefix has a set of allowed states. Ancestors of an absent path may
// themselves be absent or directories; ancestors of existing inputs must exist.
// Intersecting requirements detects contradictions without treating every watch
// as a file (which would reject legal nested directory observations).
const ABSENT: u8 = 1;
const FILE: u8 = 2;
const DIRECTORY: u8 = 4;
struct PathRequirements {
    entries: BTreeMap<String, (String, u8)>,
    limit: usize,
}
impl PathRequirements {
    fn new(limit: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            limit,
        }
    }
    fn insert(&mut self, path: &LogicalPath, terminal: u8) -> Result<()> {
        let path = path.as_str();
        if !path.is_ascii() {
            return Err(structure(path, "watch paths must contain only ASCII"));
        }
        if path.split('/').count() > PATH_DEPTH_LIMIT {
            return Err(budget(path, "observation path exceeds 32 components"));
        }
        for end in path
            .match_indices('/')
            .map(|(index, _)| index)
            .chain([path.len()])
        {
            let prefix = &path[..end];
            let allowed = if end == path.len() {
                terminal
            } else if terminal == ABSENT {
                ABSENT | DIRECTORY
            } else {
                DIRECTORY
            };
            match self.entries.entry(prefix.to_ascii_lowercase()) {
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    let (spelling, previous) = entry.get_mut();
                    if spelling != prefix {
                        return Err(structure(path, "observation path contains a case alias"));
                    }
                    *previous &= allowed;
                    if *previous == 0 {
                        return Err(structure(
                            path,
                            "contradictory absence, file or directory requirements",
                        ));
                    }
                }
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert((prefix.to_owned(), allowed));
                }
            }
            if self.entries.len() > self.limit {
                return Err(budget(path, "observation path prefixes exceed their limit"));
            }
        }
        Ok(())
    }
}
fn structure(subject: impl Into<String>, message: &'static str) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "OBS001", subject, message)
}
fn budget(subject: impl Into<String>, message: &'static str) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "OBS002", subject, message)
}
fn changed(path: &LogicalPath, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::Conflict, "OBS003", path.as_str(), message)
}
