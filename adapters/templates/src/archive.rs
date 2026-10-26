use crate::{AssetError, AssetErrorKind};
use madb_model::{Digest, LogicalPath};
use madb_plan::project::{
    FileIdentity, MAX_TEMPLATE_CONTENT_BYTES, MAX_TEMPLATE_FILE_BYTES, MAX_TEMPLATE_FILES,
    check_inventory_paths,
};
use std::collections::BTreeMap;
use std::ops::Range;

const BLOCK: usize = 512;
const MAX_ARCHIVE_BYTES: usize = 133 * 1024 * 1024;
const MAX_FILE_BYTES: usize = MAX_TEMPLATE_FILE_BYTES as usize;
const MAX_CONTENT_BYTES: usize = MAX_TEMPLATE_CONTENT_BYTES as usize;
const MAX_FILES: usize = MAX_TEMPLATE_FILES;

/// A range into an immutable checked archive; callers cannot change a path or
/// substitute different payload bytes after admission.
#[derive(Clone, Debug)]
pub struct ArchiveEntry {
    identity: FileIdentity,
    range: Range<usize>,
}

impl ArchiveEntry {
    pub fn identity(&self) -> &FileIdentity {
        &self.identity
    }
}

#[derive(Debug)]
pub struct CheckedArchive {
    bytes: Vec<u8>,
    digest: Digest,
    entries: Vec<ArchiveEntry>,
}

impl CheckedArchive {
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn digest(&self) -> &Digest {
        &self.digest
    }
    pub fn entries(&self) -> &[ArchiveEntry] {
        &self.entries
    }
    pub fn entry_bytes(&self, path: &LogicalPath) -> Option<&[u8]> {
        self.entries
            .binary_search_by(|entry| entry.identity.path.cmp(path))
            .ok()
            .map(|index| &self.bytes[self.entries[index].range.clone()])
    }

    /// The manifest is independently pinned by the trusted catalog. Its own
    /// inventory excludes itself; all other entries, selected or not, must
    /// match the inventory exactly. No filesystem root is guessed here.
    pub fn check_inventory(
        self,
        manifest: &FileIdentity,
        files: &[FileIdentity],
    ) -> Result<CheckedInventory, AssetError> {
        if files.len() + 1 != self.entries.len() || files.len() >= MAX_FILES {
            return Err(inventory_error(
                "files",
                "complete archive entry set differs from inventory",
            ));
        }
        let mut previous: Option<&LogicalPath> = None;
        let mut expected = BTreeMap::new();
        expected.insert(&manifest.path, manifest);
        for file in files {
            if previous.is_some_and(|path| path >= &file.path) {
                return Err(inventory_error(
                    file.path.as_str(),
                    "inventory must be strictly ASCII path sorted",
                ));
            }
            if expected.insert(&file.path, file).is_some() {
                return Err(inventory_error(
                    file.path.as_str(),
                    "manifest cannot list itself or duplicate an entry",
                ));
            }
            previous = Some(&file.path);
        }
        for entry in &self.entries {
            if expected.get(&entry.identity.path).copied() != Some(&entry.identity) {
                return Err(inventory_error(
                    entry.identity.path.as_str(),
                    "entry bytes do not match the complete pinned inventory",
                ));
            }
        }
        Ok(CheckedInventory {
            archive: self,
            manifest: manifest.clone(),
        })
    }
}

/// Complete byte inventory, not a decoded or executable template. Contracts
/// must still decode the manifest and selected profile closure.
#[derive(Debug)]
pub struct CheckedInventory {
    archive: CheckedArchive,
    manifest: FileIdentity,
}

impl CheckedInventory {
    pub fn archive(&self) -> &CheckedArchive {
        &self.archive
    }
    pub fn manifest(&self) -> &FileIdentity {
        &self.manifest
    }
    pub fn manifest_bytes(&self) -> &[u8] {
        self.archive
            .entry_bytes(&self.manifest.path)
            .expect("checked inventory includes its manifest")
    }
    /// This validates actual raw File/FileRef identity against the single
    /// archive root; schema and semantic checks remain the caller's work.
    pub fn referenced_bytes(&self, reference: &FileIdentity) -> Result<&[u8], AssetError> {
        if reference.path == self.manifest.path {
            return Err(inventory_error(
                reference.path.as_str(),
                "asset references must belong to manifest.files, not the manifest itself",
            ));
        }
        let bytes = self.archive.entry_bytes(&reference.path).ok_or_else(|| {
            inventory_error(
                reference.path.as_str(),
                "reference is outside complete package inventory",
            )
        })?;
        if bytes.len() as u64 != reference.size || Digest::of_bytes(bytes) != reference.sha256 {
            return Err(inventory_error(
                reference.path.as_str(),
                "reference bytes differ from pinned identity",
            ));
        }
        Ok(bytes)
    }
}

/// Admit exactly the approved ordinary-file USTAR subset. A package digest is
/// over every original byte, including headers, padding and the exact trailer.
/// The caller supplies trusted expected identity, not a value read from this
/// archive. Acquisition and trust of that value are deliberately separate.
pub fn parse_archive(
    bytes: Vec<u8>,
    expected_sha256: &Digest,
    expected_size: u64,
) -> Result<CheckedArchive, AssetError> {
    if bytes.len() > MAX_ARCHIVE_BYTES
        || bytes.len() < 2 * BLOCK
        || !bytes.len().is_multiple_of(BLOCK)
    {
        return Err(archive_error(
            "archive",
            "archive size is outside the USTAR budget or block framing",
        ));
    }
    let digest = Digest::of_bytes(&bytes);
    if bytes.len() as u64 != expected_size || &digest != expected_sha256 {
        return Err(AssetError::new(
            AssetErrorKind::ArchiveIdentity,
            "archive",
            "original archive bytes differ from trusted identity",
        ));
    }
    let mut entries: Vec<ArchiveEntry> = Vec::new();
    let mut total = 0usize;
    let mut offset = 0usize;
    loop {
        let header = bytes
            .get(offset..offset + BLOCK)
            .ok_or_else(|| archive_error("archive", "missing exact two-block trailer"))?;
        if header.iter().all(|byte| *byte == 0) {
            if offset + 2 * BLOCK != bytes.len() || bytes[offset..].iter().any(|byte| *byte != 0) {
                return Err(archive_error(
                    "archive",
                    "trailer must be exactly two zero blocks without trailing bytes",
                ));
            }
            break;
        }
        if entries.len() == MAX_FILES {
            return Err(archive_error(
                "archive",
                "ordinary file count exceeds budget",
            ));
        }
        let (path, size) = parse_header(header)?;
        if entries
            .last()
            .is_some_and(|entry| entry.identity.path >= path)
        {
            return Err(archive_error(
                path.as_str(),
                "archive entries must be strictly ASCII path sorted",
            ));
        }
        total = total
            .checked_add(size)
            .filter(|sum| *sum <= MAX_CONTENT_BYTES)
            .ok_or_else(|| archive_error(path.as_str(), "total file content exceeds budget"))?;
        let start = offset + BLOCK;
        let end = start
            .checked_add(size)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| archive_error(path.as_str(), "truncated file payload"))?;
        let padded = size.div_ceil(BLOCK) * BLOCK;
        let next = start
            .checked_add(padded)
            .filter(|end| *end <= bytes.len())
            .ok_or_else(|| archive_error(path.as_str(), "truncated file padding"))?;
        if bytes[end..next].iter().any(|byte| *byte != 0) {
            return Err(archive_error(path.as_str(), "file padding must be zero"));
        }
        let identity = FileIdentity {
            path,
            sha256: Digest::of_bytes(&bytes[start..end]),
            size: size as u64,
        };
        entries.push(ArchiveEntry {
            identity,
            range: start..end,
        });
        offset = next;
    }
    if entries.is_empty() {
        return Err(archive_error(
            "archive",
            "package has no manifest or ordinary files",
        ));
    }
    let inventory: Vec<_> = entries.iter().map(|entry| entry.identity.clone()).collect();
    check_inventory_paths(&inventory)
        .map_err(|error| archive_error(error.subject, error.message))?;
    Ok(CheckedArchive {
        bytes,
        digest,
        entries,
    })
}

fn parse_header(header: &[u8]) -> Result<(LogicalPath, usize), AssetError> {
    if &header[257..263] != b"ustar\0" || &header[263..265] != b"00" || header[156] != b'0' {
        return Err(archive_error(
            "header",
            "only ordinary POSIX USTAR entries are supported",
        ));
    }
    if header[157..257]
        .iter()
        .chain(&header[265..329])
        .chain(&header[500..512])
        .any(|byte| *byte != 0)
    {
        return Err(archive_error(
            "header",
            "link, owner names and unused bytes must be empty",
        ));
    }
    for field in [&header[329..337], &header[337..345]] {
        if field.iter().any(|byte| *byte != 0) && canonical_octal(field, "device")? != 0 {
            return Err(archive_error(
                "header",
                "device numbers must be empty or zero",
            ));
        }
    }
    if canonical_octal(&header[100..108], "mode")? != 0o644
        || canonical_octal(&header[108..116], "uid")? != 0
        || canonical_octal(&header[116..124], "gid")? != 0
        || canonical_octal(&header[136..148], "mtime")? != 0
    {
        return Err(archive_error(
            "header",
            "mode/uid/gid/mtime are not the fixed template metadata",
        ));
    }
    let size = canonical_octal(&header[124..136], "size")?;
    if size > MAX_FILE_BYTES as u64 {
        return Err(archive_error(
            "header",
            "individual file size exceeds budget",
        ));
    }
    let checksum = &header[148..156];
    if checksum[6..] != [0, b' ']
        || !checksum[..6]
            .iter()
            .all(|byte| (b'0'..=b'7').contains(byte))
    {
        return Err(archive_error("header", "noncanonical checksum field"));
    }
    let expected = checksum[..6]
        .iter()
        .fold(0u64, |value, digit| value * 8 + u64::from(digit - b'0'));
    let actual = header
        .iter()
        .enumerate()
        .map(|(index, byte)| {
            if (148..156).contains(&index) {
                u64::from(b' ')
            } else {
                u64::from(*byte)
            }
        })
        .sum::<u64>();
    if expected != actual {
        return Err(archive_error("header", "USTAR checksum mismatch"));
    }
    let name = string_field(&header[..100], "name")?;
    let prefix = string_field(&header[345..500], "prefix")?;
    if name.is_empty() {
        return Err(archive_error("name", "entry name is empty"));
    }
    let path = if prefix.is_empty() {
        name.to_owned()
    } else {
        format!("{prefix}/{name}")
    };
    let path = LogicalPath::new(path).map_err(|error| archive_error("path", error.to_string()))?;
    if path.as_str().split('/').count() > 32 {
        return Err(archive_error(path.as_str(), "path exceeds 32 components"));
    }
    Ok((path, size as usize))
}

fn string_field<'a>(field: &'a [u8], name: &str) -> Result<&'a str, AssetError> {
    let end = field
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(field.len());
    if field[end..].iter().any(|byte| *byte != 0) || !field[..end].is_ascii() {
        return Err(archive_error(name, "string field is not zero-padded ASCII"));
    }
    std::str::from_utf8(&field[..end]).map_err(|_| archive_error(name, "string field is not ASCII"))
}

fn canonical_octal(field: &[u8], name: &str) -> Result<u64, AssetError> {
    let (terminator, digits) = field
        .split_last()
        .expect("fixed nonempty USTAR numeric field");
    if *terminator != 0 || !digits.iter().all(|byte| (b'0'..=b'7').contains(byte)) {
        return Err(archive_error(
            name,
            "numeric field must be zero-filled ASCII octal with NUL terminator",
        ));
    }
    Ok(digits
        .iter()
        .fold(0u64, |value, digit| value * 8 + u64::from(digit - b'0')))
}

fn archive_error(subject: impl Into<String>, message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::ArchiveFormat, subject, message)
}
fn inventory_error(subject: impl Into<String>, message: impl Into<String>) -> AssetError {
    AssetError::new(AssetErrorKind::Inventory, subject, message)
}
