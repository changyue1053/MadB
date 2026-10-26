//! Bounded, immutable copies of an explicitly declared file inventory.
//!
//! These bytes do not prove dependency completeness or an atomic filesystem
//! snapshot. Filesystem kinds and safe path traversal remain host responsibilities.

use std::collections::BTreeMap;
use std::sync::Arc;

use crate::ports::{FileReference, ReadWorkspace};
use crate::{Digest, ErrorKind, LogicalPath, QueryError, Result};

const FILE_COUNT_LIMIT: usize = 4096;
const FILE_BYTES_LIMIT: u64 = 32 * 1024 * 1024;
const TOTAL_BYTES_LIMIT: u64 = 128 * 1024 * 1024;
const PATH_DEPTH_LIMIT: usize = 32;
const DIRECTORY_COUNT_LIMIT: usize = 8192;

/// Validated bytes owned independently of the workspace that supplied them.
/// Clones share immutable bytes; only declared files are captured and verified.
#[derive(Clone, Debug)]
pub struct FrozenInputs {
    files: Vec<FrozenFile>,
    directories: Vec<LogicalPath>,
    total_bytes: usize,
}

#[derive(Clone, Debug)]
struct FrozenFile {
    reference: FileReference,
    bytes: Arc<[u8]>,
}

impl FrozenInputs {
    /// Declared references and their captured bytes, ordered by logical path.
    pub fn files(&self) -> impl Iterator<Item = (&FileReference, &[u8])> {
        self.files
            .iter()
            .map(|file| (&file.reference, file.bytes.as_ref()))
    }

    /// The validated inventory, ordered by logical path.
    pub fn references(&self) -> impl Iterator<Item = &FileReference> {
        self.files.iter().map(|file| &file.reference)
    }

    /// All distinct parent directories in logical path order, excluding root.
    pub fn directories(&self) -> impl Iterator<Item = &LogicalPath> {
        self.directories.iter()
    }

    pub fn total_bytes(&self) -> usize {
        self.total_bytes
    }

    /// Re-read declared files and reject changed, missing or unreadable bytes.
    /// This neither enumerates directories nor observes undeclared files.
    /// Failure leaves the captured copies unchanged.
    pub fn verify<W: ReadWorkspace>(&self, workspace: &W) -> Result<()> {
        for (reference, expected) in self.files() {
            match workspace.read(&reference.path, expected.len()) {
                Ok(Some(actual)) if actual.as_slice() == expected => {}
                Ok(_) => {
                    return Err(changed(
                        &reference.path,
                        "declared input changed or disappeared since capture",
                    ));
                }
                Err(error) => {
                    return Err(changed(
                        &reference.path,
                        format!("cannot verify captured input: {error}"),
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Check an inventory without IO using the same budgets and path/alias rules
/// as capture_inputs. Incremental reference discovery must pass this preflight
/// before retaining or reading each enlarged inventory; it does not freeze bytes.
pub fn check_input_inventory(references: &[FileReference]) -> Result<()> {
    validate_inventory(references).map(|_| ())
}

/// Validate the complete inventory before reading, capture bounded bytes, then
/// recheck the workspace before returning. No partial snapshot escapes on error.
///
/// Paths must be ASCII, with no duplicates, case aliases or file/directory
/// conflicts. Limits are 4096 files, 32 MiB per file, 128 MiB total, 32 path
/// components and 8192 parent directories. Reads are bounded by declared sizes.
///
/// INPUT001 rejects invalid inventory structure; INPUT002 rejects budgets;
/// INPUT003 rejects missing or mismatched initial bytes. Recheck failures use
/// INPUT004; errors from an initial host read retain their original diagnostic.
pub fn capture_inputs<W: ReadWorkspace>(
    workspace: &W,
    mut references: Vec<FileReference>,
) -> Result<FrozenInputs> {
    let (directories, total_bytes) = validate_inventory(&references)?;
    references.sort_by(|left, right| left.path.cmp(&right.path));
    let mut files = Vec::with_capacity(references.len());
    for reference in references {
        // Preflight proves this conversion and the complete retained byte budget.
        let limit = reference.size as usize;
        let bytes = workspace
            .read(&reference.path, limit)?
            .ok_or_else(|| mismatch(&reference.path, "declared input file is missing"))?;
        if bytes.len() > limit {
            return Err(budget(
                reference.path.as_str(),
                "workspace returned bytes beyond the requested read budget",
            ));
        }
        if bytes.len() != limit || Digest::of_bytes(&bytes) != reference.digest {
            return Err(mismatch(
                &reference.path,
                "input bytes do not match the declared size and digest",
            ));
        }
        files.push(FrozenFile {
            reference,
            bytes: bytes.into(),
        });
    }
    let frozen = FrozenInputs {
        files,
        directories,
        total_bytes,
    };
    frozen.verify(workspace)?;
    Ok(frozen)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum EntryKind {
    File,
    Directory,
}

pub(crate) fn validate_inventory(
    references: &[FileReference],
) -> Result<(Vec<LogicalPath>, usize)> {
    if references.is_empty() {
        return Err(structure("inputs", "input inventory must not be empty"));
    }
    if references.len() > FILE_COUNT_LIMIT {
        return Err(budget("inputs", "input file count exceeds 4096"));
    }
    let mut total_bytes = 0;
    for reference in references {
        let path = reference.path.as_str();
        if reference.size > FILE_BYTES_LIMIT {
            return Err(budget(path, "declared input exceeds 32 MiB"));
        }
        total_bytes += reference.size;
        if total_bytes > TOTAL_BYTES_LIMIT {
            return Err(budget(path, "declared inputs exceed 128 MiB"));
        }
    }
    Ok((
        validate_paths(references.iter().map(|reference| &reference.path))?,
        total_bytes as usize,
    ))
}

/// Shared structural preflight for input and result inventories. No IO and no
/// invented byte identities are needed to validate an output path declaration.
pub(crate) fn validate_paths<'a>(
    paths: impl IntoIterator<Item = &'a LogicalPath>,
) -> Result<Vec<LogicalPath>> {
    let mut entries = BTreeMap::<String, (LogicalPath, EntryKind)>::new();
    let mut directory_count = 0;
    let mut file_count = 0;
    for logical in paths {
        file_count += 1;
        if file_count > FILE_COUNT_LIMIT {
            return Err(budget("paths", "file count exceeds 4096"));
        }
        let path = logical.as_str();
        if !path.is_ascii() {
            return Err(structure(path, "input paths must contain only ASCII"));
        }
        if path.split('/').count() > PATH_DEPTH_LIMIT {
            return Err(budget(path, "input path exceeds 32 components"));
        }
        for end in path
            .match_indices('/')
            .map(|(index, _)| index)
            .chain([path.len()])
        {
            let prefix = &path[..end];
            let kind = if end == path.len() {
                EntryKind::File
            } else {
                EntryKind::Directory
            };
            match entries.entry(prefix.to_ascii_lowercase()) {
                std::collections::btree_map::Entry::Occupied(existing) => {
                    let (previous, previous_kind) = existing.get();
                    if previous.as_str() != prefix {
                        return Err(structure(path, "input path contains a case alias"));
                    }
                    if *previous_kind == EntryKind::File || kind == EntryKind::File {
                        return Err(structure(
                            path,
                            "duplicate input file or file/directory ancestor conflict",
                        ));
                    }
                }
                std::collections::btree_map::Entry::Vacant(entry) => {
                    // Prefixes of a valid logical path remain valid logical paths.
                    let logical = LogicalPath::new(prefix)
                        .map_err(|error| structure(path, error.to_string()))?;
                    entry.insert((logical, kind));
                    if kind == EntryKind::Directory {
                        directory_count += 1;
                        if directory_count > DIRECTORY_COUNT_LIMIT {
                            return Err(budget(path, "input parent directories exceed 8192"));
                        }
                    }
                }
            }
        }
    }
    if file_count == 0 {
        return Err(structure("paths", "file inventory must not be empty"));
    }
    let mut directories: Vec<_> = entries
        .into_values()
        .filter_map(|(path, kind)| (kind == EntryKind::Directory).then_some(path))
        .collect();
    directories.sort();
    Ok(directories)
}

fn structure(subject: impl Into<String>, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "INPUT001", subject, message)
}

fn budget(subject: impl Into<String>, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "INPUT002", subject, message)
}

fn mismatch(path: &LogicalPath, message: &'static str) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, "INPUT003", path.as_str(), message)
}

fn changed(path: &LogicalPath, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::Conflict, "INPUT004", path.as_str(), message)
}
