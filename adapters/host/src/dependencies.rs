//! Restricted, real Clang dependency discovery. This provider deliberately has
//! no fallback that promotes an unknown compiler/search closure to a cache hit.
mod depfile;
mod preparation;
mod profile;
mod search;
use depfile::parse_depfile;
pub use preparation::{
    NativePreparation, NativePreparationControl, PREPROCESSED_FILE, RECEIPT_FILE, prepare_native,
    prepare_native_controlled,
};
use profile::{native_image, supported_request, supported_source};
use search::search_domain;
#[cfg(test)]
#[path = "dependencies/tests.rs"]
mod tests;

use crate::{
    LocalWorkspace,
    inputs::{PreparedRun, RetainedRun},
};
use madb_app::{
    Digest, ErrorKind, Finding, Id, LogicalPath, QueryError, Result,
    dependencies::{
        DependencyFormat, DependencyManifest, DependencyProvider, DependencyRequest,
        DependencySnapshot, DirectoryDependency, capture_dependencies,
    },
    input_observation::{DirectoryEntryKind, InputObservation, InputWatchSet, ObserveWorkspace},
    inputs::{FrozenInputs, capture_inputs},
    ports::{FileReference, ReadWorkspace},
    process::{
        ExecutionControl, ProcessError, ProcessExecution, ProcessPolicy, ProcessRequest,
        ProcessResult,
    },
};
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

/// Retains the actual process owner even when the result was cancelled/rejected.
#[must_use]
pub struct DependencyDiscovery {
    pub snapshot: Option<DependencySnapshot>,
    pub reason: Option<Finding>,
    pub retained_path: Option<PathBuf>,
    run: Option<RetainedRun<DependencySnapshot>>,
}
impl DependencyDiscovery {
    pub fn execution(&self) -> Option<&ProcessResult<ProcessExecution>> {
        self.run.as_ref().map(RetainedRun::execution)
    }
    pub fn poll_cleanup(&mut self) -> ProcessResult<bool> {
        self.run
            .as_mut()
            .map_or(Ok(true), RetainedRun::poll_cleanup)
    }
}

/// `context` identifies the caller's locked compiler installation/profile. The
/// native binary and system runtime installation must remain trusted and stable;
/// this is not an OS sandbox nor proof of arbitrary executable behaviour.
#[allow(clippy::too_many_arguments)]
pub fn discover_clang<F: DependencyFormat>(
    source_root: &Path,
    tool_root: &Path,
    run_parent: &Path,
    run_name: &LogicalPath,
    request: &DependencyRequest,
    context: Digest,
    format: &F,
    control: &ExecutionControl,
) -> Result<DependencyDiscovery> {
    request.validate()?;
    if let Some(reason) = supported_request(request) {
        return Ok(unsupported(reason));
    }
    let source = LocalWorkspace::open(source_root)?;
    let tools = LocalWorkspace::open(tool_root)?;
    let image = tools
        .read(
            &request.compiler.path,
            request.compiler.size.min(256 * 1024 * 1024) as usize,
        )?
        .ok_or_else(|| error("DEP003", "compiler", "compiler image missing"))?;
    if image.len() as u64 != request.compiler.size
        || Digest::of_bytes(&image) != request.compiler.digest
    {
        return Err(error(
            "DEP003",
            "compiler",
            "compiler image does not match the locked identity",
        ));
    }
    if !native_image(&image) {
        return Ok(unsupported(
            "compiler must be a locked native Clang image, not a script",
        ));
    }
    let (files, observation, directories) = search_domain(&source, request)?;
    for (file, bytes) in files.files() {
        if let Some(reason) = supported_source(bytes) {
            return Ok(unsupported(format!("{}: {reason}", file.path.as_str())));
        }
    }
    let prepared = PreparedRun::create(run_parent, run_name, files.clone())
        .map_err(retained_preparation_error)?;
    let retained_path = Some(prepared.path().to_path_buf());
    let mut arguments = compiler_arguments(request);
    arguments.extend([
        "-M".to_owned(),
        "-MT".to_owned(),
        "madb-dependencies".to_owned(),
    ]);
    // Clang emits one Make rule per translation unit, in argument order. Both
    // scans run inside the same controlled process/result admission gate.
    for _ in 0..2 {
        arguments.extend(request.sources.iter().map(input_argument));
    }
    let process = process_request(request, arguments);
    let mut run =
        prepared.execute_validated(tool_root, policy(), &process, control, |_, execution, _| {
            let rules = parse_depfile(&execution.report.stdout.bytes).map_err(process_error)?;
            if rules.len() != request.sources.len() * 2
                || rules[..request.sources.len()] != rules[request.sources.len()..]
            {
                return Err(process_error(error(
                    "DEP002",
                    "depfile",
                    "ordered repeated dependency discoveries disagree",
                )));
            }
            if rules[..request.sources.len()]
                .iter()
                .zip(&request.sources)
                .any(|(rule, source)| !rule.contains(source))
            {
                return Err(process_error(error(
                    "DEP002",
                    "depfile",
                    "ordered dependency rule does not name its translation unit",
                )));
            }
            let discovered: BTreeSet<_> = rules[..request.sources.len()]
                .iter()
                .flatten()
                .cloned()
                .collect();
            let known: BTreeSet<_> = files.references().map(|file| &file.path).collect();
            if discovered.iter().any(|path| !known.contains(path))
                || request
                    .sources
                    .iter()
                    .any(|path| !discovered.contains(path))
            {
                return Err(process_error(error(
                    "DEP003",
                    "depfile",
                    "compiler read outside the frozen source/search domain",
                )));
            }
            files.verify(&source).map_err(process_error)?;
            observation.verify(&source).map_err(process_error)?;
            let manifest = DependencyManifest {
                provider: Id::new("clang-freestanding/1").expect("constant"),
                context,
                compiler: request.compiler.clone(),
                target: request.target.clone(),
                sources: request.sources.clone(),
                include_directories: request.include_directories.clone(),
                arguments: request.arguments.clone(),
                environment: request.environment.clone(),
                files: files.references().cloned().collect(),
                discovered: discovered.into_iter().collect(),
                directories,
                absent: request.absent.clone(),
            };
            let mut provider = CompletedDiscovery(Some(manifest));
            capture_dependencies(&source, &mut provider, format)
                .map_err(process_error)?
                .snapshot
                .ok_or_else(|| {
                    process_error(error(
                        "DEP003",
                        "discovery",
                        "supported discovery produced no snapshot",
                    ))
                })
        });
    let snapshot = run.take_value();
    let reason = if snapshot.is_none() {
        Some(Finding::new(
            "DEP004",
            "discovery",
            "compiler discovery failed, was cancelled, or did not finish cleanup; retained execution contains details",
        ))
    } else {
        None
    };
    Ok(DependencyDiscovery {
        snapshot,
        reason,
        retained_path,
        run: Some(run),
    })
}

/// Real native preparation output. This is preprocessed source, not a generated
/// language SDK, OutputSet or BuildEvidence. Only the exact discovery recipe is
/// accepted; arbitrary helper generators do not inherit its cache eligibility.
#[allow(clippy::too_many_arguments)]
pub fn preprocess_clang(
    source_root: &Path,
    tool_root: &Path,
    run_parent: &Path,
    run_name: &LogicalPath,
    request: &DependencyRequest,
    snapshot: &DependencySnapshot,
    output: &LogicalPath,
    control: &ExecutionControl,
) -> Result<RetainedRun<FrozenInputs>> {
    if supported_request(request).is_some()
        || request.sources.len() != 1
        || !matches_request(request, snapshot.manifest())
    {
        return Err(error(
            "DEP003",
            "preprocess",
            "preprocessing requires the exact supported single-source discovery request",
        ));
    }
    let source = LocalWorkspace::open(source_root)?;
    snapshot.verify(&source)?;
    let prepared = PreparedRun::create(run_parent, run_name, snapshot.files().clone())
        .map_err(retained_preparation_error)?;
    let mut arguments = compiler_arguments(request);
    arguments.extend([
        "-E".to_owned(),
        "-P".to_owned(),
        input_argument(&request.sources[0]),
    ]);
    Ok(prepared.execute_validated(
        tool_root,
        policy(),
        &process_request(request, arguments),
        control,
        |_, execution, _| {
            snapshot.verify(&source).map_err(process_error)?;
            let bytes = execution.report.stdout.bytes.clone();
            let file = FileReference {
                path: output.clone(),
                digest: Digest::of_bytes(&bytes),
                size: bytes.len() as u64,
            };
            capture_inputs(&MemoryFile(file.path.clone(), bytes), vec![file]).map_err(process_error)
        },
    ))
}

fn matches_request(r: &DependencyRequest, m: &DependencyManifest) -> bool {
    r.target == m.target
        && r.compiler == m.compiler
        && r.sources == m.sources
        && r.include_directories == m.include_directories
        && r.arguments == m.arguments
        && r.environment == m.environment
        && {
            let mut a = r.absent.clone();
            a.sort();
            a == m.absent
        }
}
fn policy() -> ProcessPolicy {
    ProcessPolicy {
        stdout_limit: 16 * 1024 * 1024,
        allowed_environment: ["LANG", "LC_ALL", "SOURCE_DATE_EPOCH"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        ..ProcessPolicy::default()
    }
}
fn process_request(r: &DependencyRequest, arguments: Vec<String>) -> ProcessRequest {
    ProcessRequest {
        tool: r.compiler.clone(),
        working_directory: LogicalPath::new("work").expect("constant"),
        arguments,
        environment: r.environment.clone(),
        stdin: vec![],
    }
}
fn compiler_arguments(r: &DependencyRequest) -> Vec<String> {
    let mut args = vec![
        "--no-default-config".into(),
        "-nostdinc".into(),
        "-Werror=date-time".into(),
    ];
    args.push(format!("--target={}", r.target.as_str()));
    args.extend(r.arguments.clone());
    for dir in &r.include_directories {
        args.push(format!("-I../inputs/{}", dir.as_str()));
    }
    args
}
fn input_argument(path: &LogicalPath) -> String {
    format!("../inputs/{}", path.as_str())
}
fn unsupported(message: impl Into<String>) -> DependencyDiscovery {
    DependencyDiscovery {
        snapshot: None,
        reason: Some(Finding::new("DEP003", "unsupported-closure", message)),
        retained_path: None,
        run: None,
    }
}
fn error(code: &'static str, subject: &str, message: impl Into<String>) -> QueryError {
    QueryError::new(ErrorKind::InvalidInput, code, subject, message)
}
fn process_error(error: QueryError) -> ProcessError {
    ProcessError::new(
        "EXEC020",
        "dependencies",
        "dependency preparation validation failed",
    )
    .caused_by(error)
}
struct CompletedDiscovery(Option<DependencyManifest>);
impl DependencyProvider for CompletedDiscovery {
    fn discover(&mut self) -> Result<std::result::Result<DependencyManifest, Finding>> {
        Ok(Ok(self.0.take().expect("one discovery capture")))
    }
}
struct MemoryFile(LogicalPath, Vec<u8>);
impl ReadWorkspace for MemoryFile {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        if path != &self.0 {
            return Ok(None);
        }
        if self.1.len() > limit {
            return Err(error("DEP001", "output", "output exceeds declared size"));
        }
        Ok(Some(self.1.clone()))
    }
}

fn retained_preparation_error(mut failure: crate::inputs::PreparationError) -> QueryError {
    if let Some(path) = failure.retained_path {
        failure.error.findings.push(Finding::new(
            "DEP004",
            "retained-preparation",
            format!(
                "partial owned preparation is retained at {}",
                path.display()
            ),
        ));
    }
    failure.error
}
