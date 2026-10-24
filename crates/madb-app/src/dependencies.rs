//! Validated dependency observations and cache identities. IO and compiler
//! policy belong to a trusted provider; wire encoding belongs to the format port.
use crate::{
    Digest, ErrorKind, Finding, Id, LogicalPath, QueryError, Result,
    input_observation::{DirectoryEntry, InputObservation, InputWatchSet, ObserveWorkspace},
    inputs::{FrozenInputs, capture_inputs, validate_inventory},
    ports::{FileReference, ReadWorkspace},
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyRequest {
    pub compiler: FileReference,
    pub target: Id,
    pub sources: Vec<LogicalPath>,
    pub include_directories: Vec<LogicalPath>,
    pub absent: Vec<LogicalPath>,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
}

impl DependencyRequest {
    /// Reject contradictory declarations before any compiler or workspace IO.
    pub fn validate(&self) -> Result<()> {
        if self.sources.is_empty()
            || self.sources.len() > 128
            || self.include_directories.len() > 128
            || self.arguments.len() > 256
            || self.environment.len() > 128
            || self.compiler.size == 0
            || self.compiler.size > crate::process::MAX_TOOL_BYTES
            || self
                .arguments
                .iter()
                .chain(self.environment.keys())
                .chain(self.environment.values())
                .any(|s| s.len() > 4096 || s.contains('\0'))
        {
            return Err(error(
                "DEP001",
                "request",
                "invalid or oversized dependency request",
            ));
        }
        let placeholder = Digest::of_bytes(b"");
        let refs: Vec<_> = self
            .sources
            .iter()
            .map(|path| FileReference {
                path: path.clone(),
                digest: placeholder.clone(),
                size: 0,
            })
            .collect();
        validate_inventory(&refs)?;
        let mut dirs: BTreeSet<_> = self.include_directories.iter().cloned().collect();
        for source in &self.sources {
            if let Some((parent, _)) = source.as_str().rsplit_once('/') {
                dirs.insert(LogicalPath::new(parent).map_err(|e| error("DEP001", "source", e.0))?);
            }
        }
        let watch = InputWatchSet::new(self.absent.clone(), dirs.into_iter().collect())?;
        watch.validate_files(self.sources.iter())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryDependency {
    pub path: LogicalPath,
    pub entries: Vec<DirectoryEntry>,
}

/// Provider-derived metadata. A caller cannot make an arbitrary compiler safe
/// by loading this document: only a fresh supported provider discovery can mint
/// a DependencySnapshot for cache lookup.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DependencyManifest {
    pub provider: Id,
    pub context: Digest,
    pub compiler: FileReference,
    pub target: Id,
    pub sources: Vec<LogicalPath>,
    pub include_directories: Vec<LogicalPath>,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub files: Vec<FileReference>,
    /// Files actually consumed in ordered compiler depfile scans; files also
    /// retains the entire finite search domain for conservative invalidation.
    pub discovered: Vec<LogicalPath>,
    pub directories: Vec<DirectoryDependency>,
    pub absent: Vec<LogicalPath>,
}
impl DependencyManifest {
    pub fn validate(&self) -> Result<()> {
        validate_inventory(&self.files)?;
        if self.sources.is_empty()
            || self.sources.len() > 128
            || self.include_directories.len() > 128
            || self.arguments.len() > 256
            || self.environment.len() > 128
            || self
                .arguments
                .iter()
                .chain(self.environment.keys())
                .chain(self.environment.values())
                .any(|text| text.len() > 4096 || text.contains('\0'))
        {
            return Err(error(
                "DEP001",
                "manifest",
                "dependency metadata exceeds supported bounds",
            ));
        }
        let paths: BTreeSet<_> = self.files.iter().map(|file| &file.path).collect();
        if self.discovered.is_empty()
            || self.discovered.len() > 4096
            || self
                .sources
                .iter()
                .chain(&self.discovered)
                .any(|source| !paths.contains(source))
            || self.sources.iter().collect::<BTreeSet<_>>().len() != self.sources.len()
            || self.discovered.iter().collect::<BTreeSet<_>>().len() != self.discovered.len()
        {
            return Err(error(
                "DEP001",
                "sources",
                "every source must occur in the discovered file inventory",
            ));
        }
        let watch = InputWatchSet::new(
            self.absent.clone(),
            self.directories
                .iter()
                .map(|dir| dir.path.clone())
                .collect(),
        )?;
        watch.validate_files(self.files.iter().map(|file| &file.path))?;
        let mut members = 0usize;
        let mut names = 0usize;
        for dir in &self.directories {
            let mut seen = BTreeSet::new();
            for entry in &dir.entries {
                members += 1;
                names += entry.name.as_str().len();
                if entry.name.as_str().contains('/')
                    || !entry.name.as_str().is_ascii()
                    || !seen.insert(entry.name.as_str().to_ascii_lowercase())
                {
                    return Err(error(
                        "DEP001",
                        dir.path.as_str(),
                        "invalid or duplicate directory dependency member",
                    ));
                }
            }
        }
        if members > 8192 || names > 2 * 1024 * 1024 {
            return Err(error(
                "DEP001",
                "directories",
                "directory dependency metadata exceeds budget",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
pub struct CacheManifest {
    pub input_key: Digest,
    pub dependency_manifest: FileReference,
    pub files: Vec<FileReference>,
}

pub trait DependencyFormat {
    fn encode_request(&self, request: &DependencyRequest) -> Result<Vec<u8>>;
    fn decode_request(&self, bytes: &[u8]) -> Result<DependencyRequest>;
    fn encode_dependencies(&self, manifest: &DependencyManifest) -> Result<Vec<u8>>;
    fn decode_dependencies(&self, bytes: &[u8]) -> Result<DependencyManifest>;
    fn encode_cache(&self, manifest: &CacheManifest) -> Result<Vec<u8>>;
    fn decode_cache(&self, bytes: &[u8]) -> Result<CacheManifest>;
}

/// This is an internal trusted adapter port, not a user-supplied complete flag.
/// Providers must actually execute supported discovery, bind all search and
/// compiler configuration, reject hidden inputs and verify repeated results.
pub trait DependencyProvider {
    fn discover(&mut self) -> Result<std::result::Result<DependencyManifest, Finding>>;
}

pub struct DependencyOutcome {
    pub snapshot: Option<DependencySnapshot>,
    pub reason: Option<Finding>,
}

/// No public new/deserialize constructor. Capture is driven by a discovery port;
/// loading a stored manifest alone never grants cache eligibility.
#[derive(Clone, Debug)]
pub struct DependencySnapshot {
    files: FrozenInputs,
    watch: InputWatchSet,
    observation: InputObservation,
    manifest: DependencyManifest,
    bytes: Vec<u8>,
    key: Digest,
}
impl DependencySnapshot {
    pub fn key(&self) -> &Digest {
        &self.key
    }
    pub fn files(&self) -> &FrozenInputs {
        &self.files
    }
    pub fn watch(&self) -> &InputWatchSet {
        &self.watch
    }
    pub fn manifest_bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn manifest(&self) -> &DependencyManifest {
        &self.manifest
    }
    pub fn verify<W: ObserveWorkspace>(&self, workspace: &W) -> Result<()> {
        self.files.verify(workspace)?;
        self.observation.verify(workspace)
    }
}

pub fn capture_dependencies<W: ObserveWorkspace, P: DependencyProvider, F: DependencyFormat>(
    workspace: &W,
    provider: &mut P,
    format: &F,
) -> Result<DependencyOutcome> {
    let mut manifest = match provider.discover()? {
        Ok(manifest) => manifest,
        Err(reason) => {
            return Ok(DependencyOutcome {
                snapshot: None,
                reason: Some(reason),
            });
        }
    };
    manifest.validate()?;
    manifest
        .files
        .sort_by(|left, right| left.path.cmp(&right.path));
    manifest
        .directories
        .sort_by(|left, right| left.path.cmp(&right.path));
    for directory in &mut manifest.directories {
        directory
            .entries
            .sort_by(|left, right| left.name.cmp(&right.name));
    }
    manifest.absent.sort();
    manifest.discovered.sort();
    let watch = InputWatchSet::new(
        manifest.absent.clone(),
        manifest
            .directories
            .iter()
            .map(|dir| dir.path.clone())
            .collect(),
    )?;
    let observation = InputObservation::capture(workspace, &watch)?;
    observation.validate_files(manifest.files.iter().map(|file| &file.path))?;
    let actual: Vec<_> = observation
        .directory_members()
        .map(|(path, entries)| DirectoryDependency {
            path: path.clone(),
            entries: entries.to_vec(),
        })
        .collect();
    if actual != manifest.directories {
        return Err(error(
            "DEP002",
            "directories",
            "discovery directory observations no longer match the workspace",
        ));
    }
    let files = capture_inputs(workspace, manifest.files.clone())?;
    observation.verify(workspace)?;
    let bytes = format.encode_dependencies(&manifest)?;
    // This identity hashes the exact canonical manifest artifact bytes. It does
    // not replace task_digest, source_digest or their existing projections.
    let key = Digest::of_bytes(&bytes);
    Ok(DependencyOutcome {
        snapshot: Some(DependencySnapshot {
            files,
            watch,
            observation,
            manifest,
            bytes,
            key,
        }),
        reason: None,
    })
}
fn error(code: &'static str, subject: impl Into<String>, message: &'static str) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, code, subject, message)
}

/// Versioned, unambiguous preparation identity. The receipt itself is excluded
/// to avoid a recursive digest; recipe execution identity is a separate layer.
pub fn preparation_context(request: &[u8], lock: &[u8], profile: &[u8]) -> Digest {
    let mut framed = b"madb.native-preparation-context/1\0".to_vec();
    for bytes in [request, lock, profile] {
        framed.extend_from_slice(&(bytes.len() as u64).to_be_bytes());
        framed.extend_from_slice(bytes);
    }
    Digest::of_bytes(&framed)
}

/// A loaded receipt is verification material only; it has no cache lookup API
/// and cannot be converted into a fresh provider's DependencySnapshot.
pub struct PreparedDependencies {
    compiler: FileReference,
    files: FrozenInputs,
    observation: InputObservation,
}
impl PreparedDependencies {
    pub fn files(&self) -> &FrozenInputs {
        &self.files
    }
    pub fn verify<W: ObserveWorkspace, T: ReadWorkspace>(
        &self,
        workspace: &W,
        tools: &T,
    ) -> Result<()> {
        verify_compiler(tools, &self.compiler)?;
        self.files.verify(workspace)?;
        self.observation.verify(workspace)
    }
    /// Rebinding is explicit: the caller must prepare a new declared request
    /// containing every exact receipt reference before normal execution.
    pub fn validate_coverage(&self, declared: &[FileReference]) -> Result<()> {
        validate_inventory(declared)?;
        let declared: BTreeMap<_, _> = declared.iter().map(|file| (&file.path, file)).collect();
        for required in self.files.references() {
            if declared.get(&required.path).copied() != Some(required) {
                return Err(error(
                    "DEP005",
                    required.path.as_str(),
                    "prepared dependency is absent or stale in declared inputs; explicitly rebind the execution request",
                ));
            }
        }
        Ok(())
    }
}

pub fn verify_prepared_dependencies<W: ObserveWorkspace, T: ReadWorkspace, F: DependencyFormat>(
    workspace: &W,
    tools: &T,
    request: &DependencyRequest,
    bytes: &[u8],
    expected_context: &Digest,
    format: &F,
) -> Result<PreparedDependencies> {
    let manifest = format.decode_dependencies(bytes)?;
    verify_compiler(tools, &manifest.compiler)?;
    let mut absent = request.absent.clone();
    absent.sort();
    let mut recorded_absent = manifest.absent.clone();
    recorded_absent.sort();
    if &manifest.context != expected_context
        || manifest.compiler != request.compiler
        || manifest.target != request.target
        || manifest.sources != request.sources
        || manifest.include_directories != request.include_directories
        || manifest.arguments != request.arguments
        || manifest.environment != request.environment
        || absent != recorded_absent
    {
        return Err(error(
            "DEP005",
            "receipt",
            "dependency receipt is not bound to this request/lock/profile; prepare it again",
        ));
    }
    let watch = InputWatchSet::new(
        manifest.absent,
        manifest
            .directories
            .iter()
            .map(|d| d.path.clone())
            .collect(),
    )?;
    let observation = InputObservation::capture(workspace, &watch)?;
    observation.validate_files(manifest.files.iter().map(|f| &f.path))?;
    let mut expected = manifest.directories;
    expected.sort_by(|a, b| a.path.cmp(&b.path));
    for dir in &mut expected {
        dir.entries.sort_by(|a, b| a.name.cmp(&b.name));
    }
    let actual: Vec<_> = observation
        .directory_members()
        .map(|(p, e)| DirectoryDependency {
            path: p.clone(),
            entries: e.to_vec(),
        })
        .collect();
    if expected != actual {
        return Err(error(
            "DEP005",
            "receipt",
            "search directory membership changed; prepare dependencies again",
        ));
    }
    let files = capture_inputs(workspace, manifest.files)?;
    observation.verify(workspace)?;
    verify_compiler(tools, &manifest.compiler)?;
    Ok(PreparedDependencies {
        compiler: manifest.compiler,
        files,
        observation,
    })
}

/// Tool images use the process budget, separately from ordinary input files.
/// Missing or patched tools cannot be accepted by merely retaining an old lock.
pub fn verify_compiler<T: ReadWorkspace>(tools: &T, compiler: &FileReference) -> Result<()> {
    if compiler.size == 0 || compiler.size > crate::process::MAX_TOOL_BYTES {
        return Err(error(
            "DEP005",
            "compiler",
            "compiler exceeds the supported tool-image budget",
        ));
    }
    let bytes = tools
        .read(&compiler.path, compiler.size as usize)?
        .ok_or_else(|| error("DEP005", "compiler", "prepared compiler image is missing"))?;
    if bytes.len() as u64 != compiler.size || Digest::of_bytes(&bytes) != compiler.digest {
        return Err(error(
            "DEP005",
            "compiler",
            "prepared compiler image differs from its locked identity",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Tool {
        bytes: Option<Vec<u8>>,
        reads: std::cell::Cell<usize>,
    }
    impl ReadWorkspace for Tool {
        fn read(&self, _: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
            self.reads.set(self.reads.get() + 1);
            assert!(limit <= crate::process::MAX_TOOL_BYTES as usize);
            Ok(self.bytes.clone())
        }
    }
    #[test]
    fn context_frames_each_identity_without_ambiguous_concatenation() {
        assert_ne!(
            preparation_context(b"a", b"bc", b"d"),
            preparation_context(b"ab", b"c", b"d")
        );
        assert_ne!(
            preparation_context(b"a", b"b", b"c"),
            preparation_context(b"a", b"b", b"d")
        );
        assert_eq!(
            preparation_context(b"a", b"b", b"c"),
            preparation_context(b"a", b"b", b"c")
        );
    }
    #[test]
    fn loaded_tool_identity_rejects_patch_missing_and_oversize_before_io() {
        let mut reference = FileReference {
            path: LogicalPath::new("clang").unwrap(),
            digest: Digest::of_bytes(b"image"),
            size: 5,
        };
        let tool = Tool {
            bytes: Some(b"image".to_vec()),
            reads: std::cell::Cell::new(0),
        };
        verify_compiler(&tool, &reference).unwrap();
        let patched = Tool {
            bytes: Some(b"patch".to_vec()),
            reads: std::cell::Cell::new(0),
        };
        assert_eq!(
            verify_compiler(&patched, &reference).unwrap_err().findings[0].code,
            "DEP005"
        );
        let missing = Tool {
            bytes: None,
            reads: std::cell::Cell::new(0),
        };
        assert!(verify_compiler(&missing, &reference).is_err());
        reference.size = crate::process::MAX_TOOL_BYTES + 1;
        assert!(verify_compiler(&tool, &reference).is_err());
        assert_eq!(tool.reads.get(), 1);
    }
}
