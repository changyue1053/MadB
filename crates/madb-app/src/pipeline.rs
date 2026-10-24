//! Generation, metadata and packaging execute their actual port payloads.
//! Capturing a stage is separate from publishing one of the three batch layers.
use std::collections::BTreeMap;

use crate::{
    Digest, ErrorKind, Id, LogicalPath, QueryError, Result,
    dag::{GraphControl, GraphStatus},
    inputs::{FrozenInputs, capture_inputs, validate_inventory, validate_paths},
    ports::{FileReference, ReadWorkspace},
    process::{CancelDecision, ExecutionControl, ProcessRequest},
    transaction::{
        Batch, BatchLease, BatchSpec, BatchStore, CommitControl, CommitReceipt, IndexSnapshot,
        Layer,
    },
};

/// Generator and packager invocations have dynamically declared file inventories
/// in their genuine response payload, rather than pretending to be BuildTasks.
pub struct ArtifactInvocation {
    pub id: Id,
    pub process: ProcessRequest,
    pub inputs: Vec<FileReference>,
    pub response_path: LogicalPath,
}
impl ArtifactInvocation {
    pub fn validate(&self) -> Result<()> {
        if self.process.working_directory.as_str() != "work" {
            return Err(error(
                "PIPE001",
                "working_directory",
                "artifact cwd must be work",
            ));
        }
        validate_inventory(&self.inputs)?;
        validate_paths([&self.response_path])?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArtifactKind {
    Metadata,
    Generation,
    Candidate,
}
#[derive(Clone, Debug)]
pub struct ArtifactDeclaration {
    pub kind: ArtifactKind,
    pub identity: Digest,
    pub input_digest: Digest,
    pub task_digest: Digest,
    pub profile_digest: Digest,
    pub generation_id: Option<Digest>,
    /// Response file is deliberately excluded; it is captured from exact stdout.
    pub files: Vec<FileReference>,
}
pub trait ArtifactRecipe {
    fn invocation(&self) -> &ArtifactInvocation;
    fn validate_result(&self, stdout: &[u8]) -> Result<ArtifactDeclaration>;
}

#[derive(Clone, Debug)]
pub struct ArtifactResult {
    declaration: ArtifactDeclaration,
    files: FrozenInputs,
    manifest: LogicalPath,
}
impl ArtifactResult {
    pub fn declaration(&self) -> &ArtifactDeclaration {
        &self.declaration
    }
    pub fn files(&self) -> &FrozenInputs {
        &self.files
    }
    pub fn manifest(&self) -> (&FileReference, &[u8]) {
        self.files
            .files()
            .find(|(reference, _)| reference.path == self.manifest)
            .expect("artifact capture includes its manifest")
    }
}

/// Validate payload identity, then capture its exact file set and the physical
/// stdout manifest. A response cannot omit, hash itself, or alter its manifest.
/// Host confinement and exact tree enumeration remain separate checks.
pub fn capture_artifact_result<W: ReadWorkspace, R: ArtifactRecipe>(
    workspace: &W,
    recipe: &R,
    stdout: &[u8],
    limit: usize,
) -> Result<ArtifactResult> {
    recipe.invocation().validate()?;
    let declaration = recipe.validate_result(stdout)?;
    let manifest = recipe.invocation().response_path.clone();
    if declaration.files.iter().any(|file| file.path == manifest) {
        return Err(error(
            "PIPE002",
            manifest.as_str(),
            "artifact manifest cannot reference itself",
        ));
    }
    if declaration.kind != ArtifactKind::Metadata && declaration.files.is_empty() {
        return Err(error(
            "PIPE002",
            "files",
            "generation and candidate inventories must not be empty",
        ));
    }
    let mut files = declaration.files.clone();
    files.push(FileReference {
        path: manifest.clone(),
        digest: Digest::of_bytes(stdout),
        size: stdout.len() as u64,
    });
    let (_, size) = validate_inventory(&files)?;
    if size > limit {
        return Err(error(
            "PIPE002",
            "files",
            "artifact exceeds remaining result budget",
        ));
    }
    let files = capture_inputs(workspace, files)?;
    Ok(ArtifactResult {
        declaration,
        files,
        manifest,
    })
}

pub trait ArtifactNodeRun {
    fn take_value(&mut self) -> Option<ArtifactResult>;
}
pub trait ArtifactExecutor<R: ArtifactRecipe> {
    type Run: ArtifactNodeRun;
    fn execute(
        &mut self,
        recipe: &R,
        inputs: FrozenInputs,
        control: &ExecutionControl,
        result_limit: usize,
    ) -> Self::Run;
}
#[must_use]
pub struct ArtifactRun<N> {
    pub status: GraphStatus,
    pub failure: Option<QueryError>,
    node: Option<N>,
    result: Option<ArtifactResult>,
    inputs: Option<FrozenInputs>,
}
impl<N> ArtifactRun<N> {
    pub fn node(&self) -> Option<&N> {
        self.node.as_ref()
    }
    pub fn node_mut(&mut self) -> Option<&mut N> {
        self.node.as_mut()
    }
    pub fn result(&self) -> Option<&ArtifactResult> {
        self.result.as_ref()
    }
    pub fn inputs(&self) -> Option<&FrozenInputs> {
        self.inputs.as_ref()
    }
}

/// Keep source capture, execution, verification and result acceptance in one
/// graph admission interval. The returned owner survives failure or cancellation.
pub fn run_artifact<R: ArtifactRecipe, W: ReadWorkspace, E: ArtifactExecutor<R>>(
    recipe: &R,
    source: &W,
    executor: &mut E,
    control: &GraphControl,
) -> Result<ArtifactRun<E::Run>> {
    recipe.invocation().validate()?;
    let guard = control.begin()?;
    let mut run = ArtifactRun {
        status: GraphStatus::Validated,
        failure: None,
        node: None,
        result: None,
        inputs: None,
    };
    if !guard.cancelled() {
        match capture_inputs(source, recipe.invocation().inputs.clone()) {
            Err(error) => run.failure = Some(error),
            Ok(inputs) => {
                if let Some(child) = guard.activate() {
                    let mut node =
                        executor.execute(recipe, inputs.clone(), &child, 128 * 1024 * 1024);
                    guard.deactivate();
                    run.result = node.take_value();
                    run.node = Some(node);
                    if run.result.is_none() {
                        run.failure = Some(error(
                            "PIPE003",
                            recipe.invocation().id.as_str(),
                            "artifact did not produce an accepted result; inspect retained process diagnostics",
                        ));
                    } else if let Err(error) = inputs.verify(source) {
                        run.failure = Some(error);
                    }
                }
                run.inputs = Some(inputs);
            }
        }
    }
    run.status = if guard.finish() {
        GraphStatus::Cancelled
    } else if run.failure.is_some() {
        GraphStatus::Failed
    } else {
        GraphStatus::Validated
    };
    if run.status != GraphStatus::Validated {
        run.result = None;
    }
    Ok(run)
}

/// Cancellation remains live after execution, until commit (or check acceptance).
/// CommitControl is the final arbiter; closing a child graph does not close it.
#[derive(Clone, Default)]
pub struct PipelineControl {
    graph: GraphControl,
    commit: CommitControl,
}
impl PipelineControl {
    /// Keep this scope guard across execution and publication. Error/panic
    /// exits close unused commit admission and cancel an active child graph.
    pub fn scope(&self) -> PipelineScope<'_> {
        PipelineScope(self)
    }
    pub fn graph(&self) -> &GraphControl {
        &self.graph
    }
    pub fn commit(&self) -> &CommitControl {
        &self.commit
    }
    pub fn cancel(&self) -> CancelDecision {
        let decision = self.commit.cancel();
        if decision != CancelDecision::AlreadyFinished {
            self.graph.cancel();
        }
        decision
    }
    /// Metadata and --check accept observations without publishing any index.
    pub fn finish_without_commit(&self) -> Result<()> {
        self.commit.begin()?.publish(|| Ok(()))
    }
}

/// Does not delete run directories or claim process cleanup. It only closes
/// logical admission when an operation exits before a normal terminal path.
pub struct PipelineScope<'a>(&'a PipelineControl);
impl Drop for PipelineScope<'_> {
    fn drop(&mut self) {
        self.0.graph.cancel();
        // An already used commit retains its terminal state. A newly opened
        // permit closes without publishing when dropped, including cancellation.
        if let Ok(permit) = self.0.commit.begin() {
            drop(permit);
        }
    }
}

/// Pinned parents and index generation remain owned through execution and CAS.
/// Capturing both leases refuses a mixed source/build observation.
pub struct PipelineParents<L> {
    expected: IndexSnapshot,
    source: Option<L>,
    build: Option<L>,
}
impl<L: BatchLease> PipelineParents<L> {
    pub fn capture<S: BatchStore<Lease = L>>(store: &S, target: Layer) -> Result<Self> {
        let expected = store.snapshot()?;
        // New generation is independent of the old current payload. A corrupt
        // current may be replaced by a freshly validated source batch. --check
        // reads current explicitly through compare_current and never repairs it.
        let source = if target == Layer::Source {
            None
        } else {
            store.pin(Layer::Source)?
        };
        let build = if target == Layer::Candidate {
            store.pin(Layer::Build)?
        } else {
            None
        };
        for (layer, lease) in [
            (Layer::Source, source.as_ref()),
            (Layer::Build, build.as_ref()),
        ] {
            if ((layer == Layer::Source && target != Layer::Source) || target == Layer::Candidate)
                && lease.map(|lease| &lease.info().id) != expected.get(layer)
            {
                return Err(error(
                    "PIPE004",
                    "parents",
                    "current parent changed while acquiring leases",
                ));
            }
        }
        if target != Layer::Source && source.is_none() {
            return Err(error(
                "PIPE004",
                "source",
                "a committed generation is required before build or package",
            ));
        }
        if target == Layer::Candidate {
            let build = build.as_ref().ok_or_else(|| {
                error(
                    "PIPE004",
                    "build",
                    "a committed build is required before packaging",
                )
            })?;
            let source = source.as_ref().expect("required above");
            if build.info().spec.parent.as_ref() != Some(&source.info().id)
                || !same_lineage(&build.info().spec, &source.info().spec)
            {
                return Err(error(
                    "PIPE004",
                    "parents",
                    "build does not consume the pinned generation",
                ));
            }
        }
        Ok(Self {
            expected,
            source,
            build,
        })
    }
    pub fn expected(&self) -> &IndexSnapshot {
        &self.expected
    }
    pub fn source(&self) -> Option<&L> {
        self.source.as_ref()
    }
    pub fn build(&self) -> Option<&L> {
        self.build.as_ref()
    }
    pub fn workspace<'a, W: ReadWorkspace>(
        &'a self,
        external: &'a W,
    ) -> Result<PipelineWorkspace<'a, W>> {
        let mut files = BTreeMap::new();
        for lease in [self.source.as_ref(), self.build.as_ref()]
            .into_iter()
            .flatten()
        {
            for (reference, bytes) in lease.files().files() {
                if let Some((previous, previous_bytes)) =
                    files.insert(&reference.path, (reference, bytes))
                    && (previous != reference || previous_bytes != bytes)
                {
                    return Err(error(
                        "PIPE004",
                        reference.path.as_str(),
                        "parent inventories contain conflicting logical files",
                    ));
                }
            }
        }
        Ok(PipelineWorkspace { external, files })
    }
    pub fn batch(&self, artifact: &ArtifactResult) -> Result<Batch> {
        let declaration = artifact.declaration();
        let (layer, parent) =
            match declaration.kind {
                ArtifactKind::Generation => (Layer::Source, None),
                ArtifactKind::Candidate => (
                    Layer::Candidate,
                    Some(self.build.as_ref().ok_or_else(|| {
                        error("PIPE004", "build", "candidate requires pinned build")
                    })?),
                ),
                ArtifactKind::Metadata => {
                    return Err(error(
                        "PIPE004",
                        "metadata",
                        "metadata preparation does not publish a generation",
                    ));
                }
            };
        let spec = BatchSpec {
            layer,
            input_digest: declaration.input_digest.clone(),
            task_digest: declaration.task_digest.clone(),
            profile_digest: declaration.profile_digest.clone(),
            generation_id: declaration.generation_id.clone().ok_or_else(|| {
                error(
                    "PIPE004",
                    "generation_id",
                    "artifact lacks generation identity",
                )
            })?,
            parent: parent.map(|lease| lease.info().id.clone()),
        };
        if let Some(parent) = parent
            && !same_lineage(&spec, &parent.info().spec)
        {
            return Err(error(
                "PIPE004",
                "lineage",
                "candidate differs from its pinned build lineage",
            ));
        }
        Batch::new(spec, artifact.files().clone())
    }
    pub fn build_batch(&self, task_digest: Digest, files: FrozenInputs) -> Result<Batch> {
        let source = self
            .source
            .as_ref()
            .ok_or_else(|| error("PIPE004", "source", "build requires pinned generation"))?;
        let spec = BatchSpec {
            layer: Layer::Build,
            parent: Some(source.info().id.clone()),
            task_digest,
            ..source.info().spec.clone()
        };
        Batch::new(spec, files)
    }
    /// Stage only after semantic validation; CAS and final revalidation remain
    /// mandatory even for cache hits. The parent leases outlive this operation.
    pub fn publish<S: BatchStore<Lease = L>>(
        &self,
        store: &S,
        batch: &Batch,
        control: &PipelineControl,
        revalidate: impl FnOnce() -> Result<()>,
    ) -> Result<CommitReceipt> {
        let expected_parent = match batch.spec().layer {
            Layer::Source => None,
            Layer::Build => self.source.as_ref(),
            Layer::Candidate => self.build.as_ref(),
        };
        if batch.spec().parent.as_ref() != expected_parent.map(|lease| &lease.info().id) {
            return Err(error(
                "PIPE004",
                "parent",
                "batch is not bound to the pinned parent",
            ));
        }
        if let Some(parent) = expected_parent
            && !same_lineage(batch.spec(), &parent.info().spec)
        {
            return Err(error(
                "PIPE004",
                "lineage",
                "batch differs from the pinned parent lineage",
            ));
        }
        let staged = store.stage(batch)?;
        store.commit(staged, &self.expected, control.commit(), revalidate)
    }
}
fn same_lineage(left: &BatchSpec, right: &BatchSpec) -> bool {
    left.input_digest == right.input_digest
        && left.profile_digest == right.profile_digest
        && left.generation_id == right.generation_id
}

/// Reads committed parent bytes preferentially and never falls back to a
/// same-named source file. Unrelated explicitly declared external files remain
/// available through the injected source port.
pub struct PipelineWorkspace<'a, W> {
    external: &'a W,
    files: BTreeMap<&'a LogicalPath, (&'a FileReference, &'a [u8])>,
}
impl<W: ReadWorkspace> ReadWorkspace for PipelineWorkspace<'_, W> {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        if let Some((_, bytes)) = self.files.get(path) {
            if bytes.len() > limit {
                return Err(error(
                    "PIPE004",
                    path.as_str(),
                    "pinned input exceeds requested byte bound",
                ));
            }
            return Ok(Some(bytes.to_vec()));
        }
        self.external.read(path, limit)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactDifference {
    pub identity_changed: bool,
    pub missing: Vec<LogicalPath>,
    pub added: Vec<LogicalPath>,
    pub changed: Vec<LogicalPath>,
}
impl ArtifactDifference {
    pub fn is_empty(&self) -> bool {
        !self.identity_changed
            && self.missing.is_empty()
            && self.added.is_empty()
            && self.changed.is_empty()
    }
}
/// Compare captured inventories and real bytes. No store writes or index changes.
pub fn compare_files(expected: &FrozenInputs, actual: &FrozenInputs) -> ArtifactDifference {
    let left: BTreeMap<_, _> = expected.files().map(|(r, b)| (&r.path, (r, b))).collect();
    let right: BTreeMap<_, _> = actual.files().map(|(r, b)| (&r.path, (r, b))).collect();
    ArtifactDifference {
        identity_changed: false,
        missing: left
            .keys()
            .filter(|path| !right.contains_key(*path))
            .map(|p| (*p).clone())
            .collect(),
        added: right
            .keys()
            .filter(|path| !left.contains_key(*path))
            .map(|p| (*p).clone())
            .collect(),
        changed: left
            .iter()
            .filter_map(|(path, (r, b))| {
                right
                    .get(path)
                    .filter(|(other, bytes)| r != other || b != bytes)
                    .map(|_| (*path).clone())
            })
            .collect(),
    }
}
fn error(code: &'static str, subject: &str, message: &str) -> QueryError {
    QueryError::new(ErrorKind::Conflict, code, subject, message)
}

/// Inspect current only for --check. Missing/corrupt current is a refusal, never
/// an invitation to create directories or repair bytes during a check.
pub fn compare_current<S: BatchStore>(
    store: &S,
    expected: &IndexSnapshot,
    batch: &Batch,
) -> Result<ArtifactDifference> {
    let current = store.pin(batch.spec().layer)?.ok_or_else(|| {
        error(
            "PIPE005",
            "current",
            "no current artifact exists for comparison",
        )
    })?;
    if Some(&current.info().id) != expected.get(batch.spec().layer) {
        return Err(error("PIPE005", "current", "current changed during check"));
    }
    let mut difference = compare_files(current.files(), batch.files());
    difference.identity_changed = &current.info().spec != batch.spec();
    Ok(difference)
}

/// Store and cache must never cover source/configuration input trees. A run
/// parent may contain an earlier retained metadata result, but cannot itself
/// be an input file. New runs are always separately and exclusively allocated.
pub fn validate_locations(
    store: &LogicalPath,
    run_parent: &LogicalPath,
    cache: Option<&LogicalPath>,
    protected_inputs: &[LogicalPath],
) -> Result<()> {
    let mut domains = vec![store, run_parent];
    if let Some(cache) = cache {
        domains.push(cache);
    }
    for (index, left) in domains.iter().enumerate() {
        for right in domains.iter().skip(index + 1) {
            if overlaps(left, right) {
                return Err(error(
                    "PIPE006",
                    "locations",
                    "store, run and cache domains must be disjoint",
                ));
            }
        }
    }
    for input in protected_inputs {
        if overlaps(store, input) || cache.is_some_and(|cache| overlaps(cache, input)) {
            return Err(error(
                "PIPE006",
                input.as_str(),
                "store or cache overlaps a protected input",
            ));
        }
        if is_ancestor(input, run_parent) {
            return Err(error(
                "PIPE006",
                input.as_str(),
                "run parent conflicts with a protected input file",
            ));
        }
    }
    // Prefix spelling must stay consistent even for siblings such as OUT/a
    // and out/b. Logical paths may intentionally name directories here.
    let mut prefixes = BTreeMap::<String, String>::new();
    for path in domains.into_iter().chain(protected_inputs.iter()) {
        let spelling = path.as_str();
        if !spelling.is_ascii() {
            return Err(error("PIPE006", spelling, "managed paths require ASCII"));
        }
        for end in spelling
            .match_indices('/')
            .map(|(i, _)| i)
            .chain([spelling.len()])
        {
            let prefix = &spelling[..end];
            if let Some(previous) = prefixes.insert(prefix.to_ascii_lowercase(), prefix.to_owned())
                && previous != prefix
            {
                return Err(error(
                    "PIPE006",
                    spelling,
                    "managed paths contain a case-aliased prefix",
                ));
            }
        }
    }
    Ok(())
}
fn is_ancestor(left: &LogicalPath, right: &LogicalPath) -> bool {
    let left = left.as_str().to_ascii_lowercase();
    let right = right.as_str().to_ascii_lowercase();
    right == left
        || right
            .strip_prefix(&left)
            .is_some_and(|rest| rest.starts_with('/'))
}
fn overlaps(left: &LogicalPath, right: &LogicalPath) -> bool {
    is_ancestor(left, right) || is_ancestor(right, left)
}

/// Cache bytes are untrusted until the current recipe revalidates the genuine
/// manifest and the complete inventory. An extra cached file is corruption.
pub fn validate_cached_artifact<R: ArtifactRecipe>(
    recipe: &R,
    cached: &FrozenInputs,
) -> Result<ArtifactResult> {
    let (_, payload) = cached
        .files()
        .find(|(reference, _)| reference.path == recipe.invocation().response_path)
        .ok_or_else(|| {
            error(
                "PIPE002",
                "cache",
                "cached artifact lacks its response manifest",
            )
        })?;
    let result = capture_artifact_result(
        &CapturedWorkspace(cached),
        recipe,
        payload,
        128 * 1024 * 1024,
    )?;
    if !compare_files(cached, result.files()).is_empty() {
        return Err(error(
            "PIPE002",
            "cache",
            "cached inventory differs from the validated manifest",
        ));
    }
    Ok(result)
}
struct CapturedWorkspace<'a>(&'a FrozenInputs);
impl ReadWorkspace for CapturedWorkspace<'_> {
    fn read(&self, path: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        let Some((_, bytes)) = self
            .0
            .files()
            .find(|(reference, _)| &reference.path == path)
        else {
            return Ok(None);
        };
        if bytes.len() > limit {
            return Err(error(
                "PIPE002",
                path.as_str(),
                "captured file exceeds read bound",
            ));
        }
        Ok(Some(bytes.to_vec()))
    }
}

/// Observed directories/absence conditions must remain outside all writable
/// domains. Otherwise preparing the operation could invalidate its own inputs.
pub fn validate_observed_locations(
    store: &LogicalPath,
    run_parent: &LogicalPath,
    cache: Option<&LogicalPath>,
    observed_domains: &[LogicalPath],
) -> Result<()> {
    validate_locations(store, run_parent, cache, observed_domains)?;
    for observed in observed_domains {
        if overlaps(run_parent, observed) {
            return Err(error(
                "PIPE006",
                observed.as_str(),
                "run domain overlaps an observed input condition",
            ));
        }
    }
    Ok(())
}
