use madb_app::{
    Digest, Id, LogicalPath, QueryError, Result,
    dag::{GraphControl, GraphStatus},
    inputs::{FrozenInputs, capture_inputs},
    pipeline::*,
    ports::{FileReference, ReadWorkspace},
    process::{CancelDecision, ExecutionControl, ProcessRequest},
    transaction::{
        Batch, BatchInfo, BatchLease, BatchSpec, BatchStore, CommitControl, CommitDisposition,
        CommitReceipt, IndexSnapshot, Layer,
    },
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};
fn path(s: &str) -> LogicalPath {
    LogicalPath::new(s).unwrap()
}
fn digest(s: &str) -> Digest {
    Digest::of_bytes(s.as_bytes())
}
fn reference(p: &str, b: &[u8]) -> FileReference {
    FileReference {
        path: path(p),
        digest: Digest::of_bytes(b),
        size: b.len() as u64,
    }
}
#[derive(Default)]
struct Workspace {
    files: RefCell<BTreeMap<LogicalPath, Vec<u8>>>,
    reads: Cell<usize>,
}
impl Workspace {
    fn new(files: &[(&str, &[u8])]) -> Self {
        Self {
            files: RefCell::new(files.iter().map(|(p, b)| (path(p), b.to_vec())).collect()),
            reads: Cell::new(0),
        }
    }
}
impl ReadWorkspace for Workspace {
    fn read(&self, p: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        self.reads.set(self.reads.get() + 1);
        let value = self.files.borrow().get(p).cloned();
        assert!(value.as_ref().is_none_or(|v| v.len() <= limit));
        Ok(value)
    }
}
fn freeze(work: &Workspace) -> FrozenInputs {
    let refs = work
        .files
        .borrow()
        .iter()
        .map(|(p, b)| reference(p.as_str(), b))
        .collect();
    capture_inputs(work, refs).unwrap()
}
struct Recipe {
    invocation: ArtifactInvocation,
    kind: ArtifactKind,
    files: Vec<FileReference>,
}
impl Recipe {
    fn new() -> Self {
        Self {
            invocation: ArtifactInvocation {
                id: Id::new("generate").unwrap(),
                process: ProcessRequest {
                    tool: reference("tool", b"image"),
                    arguments: vec![],
                    environment: BTreeMap::new(),
                    working_directory: path("work"),
                    stdin: b"request".to_vec(),
                },
                inputs: vec![reference("source", b"source")],
                response_path: path("manifest.json"),
            },
            kind: ArtifactKind::Generation,
            files: vec![reference("api.h", b"header")],
        }
    }
}
impl ArtifactRecipe for Recipe {
    fn invocation(&self) -> &ArtifactInvocation {
        &self.invocation
    }
    fn validate_result(&self, stdout: &[u8]) -> Result<ArtifactDeclaration> {
        assert_eq!(stdout, b"manifest");
        Ok(ArtifactDeclaration {
            kind: self.kind,
            identity: digest("generation"),
            input_digest: digest("source"),
            task_digest: digest("task"),
            profile_digest: digest("profile"),
            generation_id: (self.kind != ArtifactKind::Metadata).then(|| digest("generation")),
            files: self.files.clone(),
        })
    }
}
fn output() -> Workspace {
    Workspace::new(&[("manifest.json", b"manifest"), ("api.h", b"header")])
}
struct Node {
    value: Option<ArtifactResult>,
    owner: bool,
}
impl ArtifactNodeRun for Node {
    fn take_value(&mut self) -> Option<ArtifactResult> {
        self.value.take()
    }
}
struct Executor<'a> {
    calls: usize,
    cancel: Option<GraphControl>,
    change: Option<&'a Workspace>,
    fail: bool,
}
impl ArtifactExecutor<Recipe> for Executor<'_> {
    type Run = Node;
    fn execute(
        &mut self,
        recipe: &Recipe,
        _: FrozenInputs,
        _: &ExecutionControl,
        limit: usize,
    ) -> Node {
        self.calls += 1;
        let value = (!self.fail)
            .then(|| capture_artifact_result(&output(), recipe, b"manifest", limit).unwrap());
        if let Some(control) = &self.cancel {
            control.cancel();
        }
        if let Some(source) = self.change {
            source
                .files
                .borrow_mut()
                .insert(path("source"), b"edited".to_vec());
        }
        Node { value, owner: true }
    }
}
fn executor() -> Executor<'static> {
    Executor {
        calls: 0,
        cancel: None,
        change: None,
        fail: false,
    }
}
#[test]
fn artifact_response_manifest_is_real_captured_bytes_and_budget_is_pre_read() {
    let recipe = Recipe::new();
    let work = output();
    let error = capture_artifact_result(&work, &recipe, b"manifest", 13).unwrap_err();
    assert_eq!(error.findings[0].code, "PIPE002");
    assert_eq!(work.reads.get(), 0);
    let result = capture_artifact_result(&work, &recipe, b"manifest", 14).unwrap();
    assert_eq!(result.manifest().1, b"manifest");
    assert_eq!(result.files().total_bytes(), 14);
    work.files
        .borrow_mut()
        .insert(path("manifest.json"), b"changed!".to_vec());
    assert!(capture_artifact_result(&work, &recipe, b"manifest", 14).is_err());
}
#[test]
fn self_reference_and_alias_are_rejected_without_reads() {
    for name in ["manifest.json", "MANIFEST.JSON"] {
        let mut recipe = Recipe::new();
        recipe.files.push(reference(name, b"wrong"));
        let work = output();
        assert!(capture_artifact_result(&work, &recipe, b"manifest", 100).is_err());
        assert_eq!(work.reads.get(), 0);
    }
}
#[test]
fn metadata_can_have_only_its_actual_ir_manifest() {
    let mut recipe = Recipe::new();
    recipe.kind = ArtifactKind::Metadata;
    recipe.files.clear();
    let work = Workspace::new(&[("manifest.json", b"manifest")]);
    let result = capture_artifact_result(&work, &recipe, b"manifest", 100).unwrap();
    assert_eq!(result.files().references().count(), 1);
    assert!(result.declaration().generation_id.is_none());
}
#[test]
fn source_change_or_cancel_discards_value_but_keeps_owner() {
    for cancel in [false, true] {
        let source = Workspace::new(&[("source", b"source")]);
        let control = GraphControl::default();
        let mut exec = Executor {
            calls: 0,
            cancel: cancel.then(|| control.clone()),
            change: (!cancel).then_some(&source),
            fail: false,
        };
        let mut run = run_artifact(&Recipe::new(), &source, &mut exec, &control).unwrap();
        assert_eq!(
            run.status,
            if cancel {
                GraphStatus::Cancelled
            } else {
                GraphStatus::Failed
            }
        );
        assert!(run.result().is_none());
        assert!(run.node().unwrap().owner);
        assert!(run.node_mut().unwrap().take_value().is_none());
    }
}
#[test]
fn cancelled_before_capture_never_reads_source_or_executes() {
    let source = Workspace::new(&[("source", b"source")]);
    let control = GraphControl::default();
    control.cancel();
    let mut exec = executor();
    let run = run_artifact(&Recipe::new(), &source, &mut exec, &control).unwrap();
    assert_eq!(run.status, GraphStatus::Cancelled);
    assert_eq!(source.reads.get(), 0);
    assert_eq!(exec.calls, 0);
    assert!(run_artifact(&Recipe::new(), &source, &mut exec, &control).is_err());
}
#[test]
fn successful_capture_does_not_finish_operation_commit_admission() {
    let source = Workspace::new(&[("source", b"source")]);
    let control = PipelineControl::default();
    let run = run_artifact(&Recipe::new(), &source, &mut executor(), control.graph()).unwrap();
    assert_eq!(run.status, GraphStatus::Validated);
    assert_eq!(control.cancel(), CancelDecision::Requested);
    assert!(control.finish_without_commit().is_err());
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
}
#[test]
fn cache_revalidates_manifest_inventory_and_rejects_extra_bytes() {
    let recipe = Recipe::new();
    let good = freeze(&output());
    assert!(validate_cached_artifact(&recipe, &good).is_ok());
    let bad = output();
    bad.files
        .borrow_mut()
        .insert(path("extra"), b"ignored".to_vec());
    assert!(validate_cached_artifact(&recipe, &freeze(&bad)).is_err());
}
#[derive(Clone)]
struct Lease {
    info: BatchInfo,
    files: FrozenInputs,
}
impl BatchLease for Lease {
    fn info(&self) -> &BatchInfo {
        &self.info
    }
    fn files(&self) -> &FrozenInputs {
        &self.files
    }
}
struct Store {
    index: IndexSnapshot,
    source: Option<Lease>,
    build: Option<Lease>,
    pins: Cell<usize>,
    stages: Cell<usize>,
    commits: Cell<usize>,
    fail_pin: bool,
}
impl BatchStore for Store {
    type Staged = Batch;
    type Lease = Lease;
    fn snapshot(&self) -> Result<IndexSnapshot> {
        Ok(self.index.clone())
    }
    fn stage(&self, batch: &Batch) -> Result<Batch> {
        self.stages.set(self.stages.get() + 1);
        Ok(batch.clone())
    }
    fn pin(&self, layer: Layer) -> Result<Option<Lease>> {
        self.pins.set(self.pins.get() + 1);
        if self.fail_pin {
            return Err(QueryError::new(
                madb_app::ErrorKind::Host,
                "TEST001",
                "pin",
                "corrupt current",
            ));
        }
        Ok(match layer {
            Layer::Source => self.source.clone(),
            Layer::Build => self.build.clone(),
            Layer::Candidate => None,
        })
    }
    fn commit(
        &self,
        staged: Batch,
        expected: &IndexSnapshot,
        control: &CommitControl,
        revalidate: impl FnOnce() -> Result<()>,
    ) -> Result<CommitReceipt> {
        assert_eq!(expected, &self.index);
        let permit = control.begin()?;
        revalidate()?;
        permit.publish(|| {
            self.commits.set(self.commits.get() + 1);
            Ok(())
        })?;
        Ok(CommitReceipt {
            disposition: CommitDisposition::Committed,
            index: self.index.clone(),
            batch_id: staged.id().clone(),
        })
    }
}
fn source_store() -> Store {
    let files = freeze(&Workspace::new(&[("api.h", b"fixed header")]));
    let spec = BatchSpec {
        layer: Layer::Source,
        input_digest: digest("source"),
        task_digest: digest("task"),
        profile_digest: digest("profile"),
        generation_id: digest("generation"),
        parent: None,
    };
    let batch = Batch::new(spec.clone(), files.clone()).unwrap();
    let info = BatchInfo {
        id: digest("source-instance"),
        content_id: batch.id().clone(),
        spec,
        files: files.references().cloned().collect(),
    };
    Store {
        index: IndexSnapshot {
            revision: 1,
            source: Some(info.id.clone()),
            build: None,
            candidate: None,
        },
        source: Some(Lease { info, files }),
        build: None,
        pins: Cell::new(0),
        stages: Cell::new(0),
        commits: Cell::new(0),
        fail_pin: false,
    }
}
#[test]
fn generation_rebuild_does_not_pin_or_overlay_corrupt_current() {
    let mut store = source_store();
    store.fail_pin = true;
    let parents = PipelineParents::capture(&store, Layer::Source).unwrap();
    assert_eq!(store.pins.get(), 0);
    let root = Workspace::new(&[("api.h", b"fresh source")]);
    assert_eq!(
        parents
            .workspace(&root)
            .unwrap()
            .read(&path("api.h"), 100)
            .unwrap()
            .unwrap(),
        b"fresh source"
    );
    let result = capture_artifact_result(&output(), &Recipe::new(), b"manifest", 100).unwrap();
    let batch = parents.batch(&result).unwrap();
    assert!(compare_current(&store, parents.expected(), &batch).is_err());
    assert_eq!(store.stages.get(), 0);
}
#[test]
fn build_inputs_are_pinned_and_cannot_fall_back_to_same_named_root() {
    let store = source_store();
    let parents = PipelineParents::capture(&store, Layer::Build).unwrap();
    let root = Workspace::new(&[("api.h", b"stale root")]);
    assert_eq!(
        parents
            .workspace(&root)
            .unwrap()
            .read(&path("api.h"), 100)
            .unwrap()
            .unwrap(),
        b"fixed header"
    );
    assert_eq!(root.reads.get(), 0);
    let batch = parents
        .build_batch(
            digest("compile"),
            freeze(&Workspace::new(&[("api.o", b"object")])),
        )
        .unwrap();
    assert_eq!(batch.spec().parent.as_ref(), store.index.source.as_ref());
    assert_eq!(batch.spec().generation_id, digest("generation"));
}
#[test]
fn parent_capture_rejects_missing_and_mixed_instances() {
    let mut store = source_store();
    store.index.source = Some(digest("new source instance"));
    assert!(PipelineParents::capture(&store, Layer::Build).is_err());
    let mut store = source_store();
    store.source = None;
    store.index.source = None;
    assert!(PipelineParents::capture(&store, Layer::Build).is_err());
    assert!(PipelineParents::capture(&source_store(), Layer::Candidate).is_err());
}
#[test]
fn publication_revalidates_and_honors_cancel_after_execution() {
    let store = source_store();
    let parents = PipelineParents::capture(&store, Layer::Build).unwrap();
    let batch = parents
        .build_batch(
            digest("build"),
            freeze(&Workspace::new(&[("api.o", b"object")])),
        )
        .unwrap();
    let control = PipelineControl::default();
    control.cancel();
    let checks = Cell::new(0);
    assert!(
        parents
            .publish(&store, &batch, &control, || {
                checks.set(checks.get() + 1);
                Ok(())
            })
            .is_err()
    );
    assert_eq!(store.commits.get(), 0);
    assert_eq!(checks.get(), 1);
}
#[test]
fn compare_reports_all_three_drift_kinds_without_mutating_files() {
    let left = freeze(&Workspace::new(&[("removed", b"old"), ("changed", b"old")]));
    let right = freeze(&Workspace::new(&[("added", b"new"), ("changed", b"new")]));
    let diff = compare_files(&left, &right);
    assert_eq!(diff.missing, [path("removed")]);
    assert_eq!(diff.added, [path("added")]);
    assert_eq!(diff.changed, [path("changed")]);
    assert!(!diff.is_empty());
}
#[test]
fn managed_locations_reject_aliases_ancestors_and_input_overlap() {
    let inputs = vec![path("src/input"), path("runs/old/work/contract.json")];
    assert!(
        validate_locations(&path("store"), &path("runs"), Some(&path("cache")), &inputs).is_ok()
    );
    for (store, run, cache) in [
        ("out", "out/runs", None),
        ("Out/store", "out/runs", None),
        ("src", "runs", None),
        ("store", "runs", Some("src/input")),
        ("store", "src/input/child", None),
    ] {
        assert!(
            validate_locations(&path(store), &path(run), cache.map(path).as_ref(), &inputs)
                .is_err(),
            "{store} {run}"
        );
    }
}

#[test]
fn check_detects_lineage_change_even_with_identical_file_bytes() {
    let store = source_store();
    let current = store.source.as_ref().unwrap();
    let mut spec = current.info.spec.clone();
    spec.task_digest = digest("different task");
    let batch = Batch::new(spec, current.files.clone()).unwrap();
    let difference = compare_current(&store, &store.index, &batch).unwrap();
    assert!(difference.identity_changed);
    assert!(
        difference.missing.is_empty()
            && difference.added.is_empty()
            && difference.changed.is_empty()
    );
    assert!(!difference.is_empty());
}
#[test]
fn operation_error_or_panic_scope_closes_remaining_commit_admission() {
    let control = PipelineControl::default();
    {
        let _scope = control.scope();
    }
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    assert!(control.finish_without_commit().is_err());
}
