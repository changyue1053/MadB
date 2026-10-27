// Preserve the single-step fixtures and their regressions while exercising the
// same kernel across dynamically bound consecutive artifacts.
include!("pipeline_execution.rs");

#[test]
fn sequence_failure_is_sticky_even_if_caller_ignores_the_step_status() {
    let source = Workspace::new(&[("source", b"source")]);
    let control = GraphControl::default();
    let mut sequence = ArtifactSequence::begin(&control).unwrap();
    let mut exec = executor();
    exec.fail = true;
    let first = sequence.run(&Recipe::new(), &source, &mut exec).unwrap();
    assert_eq!(first.status, GraphStatus::Failed);
    assert!(first.node().unwrap().owner);
    exec.fail = false;
    let reads = source.reads.get();
    let second = sequence.run(&Recipe::new(), &source, &mut exec).unwrap();
    assert_eq!(second.status, GraphStatus::Failed);
    assert!(second.node().is_none());
    assert_eq!(exec.calls, 1);
    assert_eq!(source.reads.get(), reads);
    assert_eq!(sequence.finish(), GraphStatus::Failed);
    assert!(ArtifactSequence::begin(&control).is_err());
}
#[test]
fn only_pipeline_cancellation_closes_the_gap_and_final_acceptance() {
    for after_second in [false, true] {
        let source = Workspace::new(&[("source", b"source")]);
        let pipeline = PipelineControl::default();
        let mut sequence = ArtifactSequence::begin(pipeline.graph()).unwrap();
        let mut exec = executor();
        let first = sequence.run(&Recipe::new(), &source, &mut exec).unwrap();
        assert!(first.result().is_some());
        if !after_second {
            pipeline.cancel();
        }
        let second = sequence.run(&Recipe::new(), &source, &mut exec).unwrap();
        if after_second {
            assert!(second.result().is_some());
            pipeline.cancel();
            assert_eq!(exec.calls, 2);
        } else {
            assert!(second.node().is_none());
            assert!(second.result().is_none());
            assert_eq!(exec.calls, 1);
        }
        assert_eq!(sequence.finish(), GraphStatus::Cancelled);
    }
}
#[test]
fn invalid_step_closes_sequence_and_drop_closes_single_use_graph() {
    let source = Workspace::new(&[("source", b"source")]);
    let control = GraphControl::default();
    let mut exec = executor();
    {
        let mut sequence = ArtifactSequence::begin(&control).unwrap();
        let mut invalid = Recipe::new();
        invalid.invocation.process.working_directory = path("other");
        assert!(sequence.run(&invalid, &source, &mut exec).is_err());
        let ignored = sequence.run(&Recipe::new(), &source, &mut exec).unwrap();
        assert_eq!(ignored.status, GraphStatus::Failed);
        assert_eq!(exec.calls, 0);
    }
    assert!(ArtifactSequence::begin(&control).is_err());
    assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
}
struct NeverRunner;
impl madb_app::ports::ProcessRunner for NeverRunner {
    fn run(
        &self,
        _: &ProcessRequest,
        _: &madb_app::process::CancellationProbe<'_>,
    ) -> madb_app::process::ProcessResult<madb_app::process::ProcessRun> {
        panic!("cancelled child must not start")
    }
}
struct PipelineCanceller(PipelineControl);
impl ArtifactExecutor<Recipe> for PipelineCanceller {
    type Run = Node;
    fn execute(
        &mut self,
        recipe: &Recipe,
        _: FrozenInputs,
        child: &ExecutionControl,
        _: usize,
    ) -> Node {
        assert_eq!(self.0.cancel(), CancelDecision::Requested);
        // Only the combination token was cancelled. Observe the registered
        // child's ordinary execution admission, without cancelling it directly.
        let result =
            madb_app::process::execute_process(&NeverRunner, &recipe.invocation.process, child)
                .unwrap();
        assert_eq!(
            result.admission,
            madb_app::process::ResultAdmission::RejectedByCancellation
        );
        assert!(result.process.is_none());
        Node {
            value: None,
            owner: true,
        }
    }
}
#[test]
fn only_pipeline_cancellation_reaches_the_registered_child() {
    let source = Workspace::new(&[("source", b"source")]);
    let pipeline = PipelineControl::default();
    let mut sequence = ArtifactSequence::begin(pipeline.graph()).unwrap();
    let mut exec = PipelineCanceller(pipeline.clone());
    let result = sequence.run(&Recipe::new(), &source, &mut exec).unwrap();
    assert_eq!(result.status, GraphStatus::Cancelled);
    assert!(result.node().unwrap().owner);
    assert!(result.result().is_none());
    assert_eq!(sequence.finish(), GraphStatus::Cancelled);
}

struct UnwindRecipe {
    inner: Recipe,
    invocation_panics: bool,
}
impl ArtifactRecipe for UnwindRecipe {
    fn invocation(&self) -> &ArtifactInvocation {
        assert!(!self.invocation_panics, "invocation unwind");
        &self.inner.invocation
    }
    fn validate_result(&self, bytes: &[u8]) -> Result<ArtifactDeclaration> {
        self.inner.validate_result(bytes)
    }
}
struct UnwindSource {
    inner: Workspace,
    at: usize,
}
impl ReadWorkspace for UnwindSource {
    fn read(&self, p: &LogicalPath, limit: usize) -> Result<Option<Vec<u8>>> {
        assert_ne!(self.inner.reads.get() + 1, self.at, "source unwind");
        self.inner.read(p, limit)
    }
}
struct UnwindNode {
    value: Option<ArtifactResult>,
    panics: bool,
}
impl ArtifactNodeRun for UnwindNode {
    fn take_value(&mut self) -> Option<ArtifactResult> {
        assert!(!self.panics, "take_value unwind");
        self.value.take()
    }
}
struct UnwindExecutor {
    phase: &'static str,
    child: Option<ExecutionControl>,
    calls: usize,
}
impl<R: ArtifactRecipe> ArtifactExecutor<R> for UnwindExecutor {
    type Run = UnwindNode;
    fn execute(
        &mut self,
        recipe: &R,
        _: FrozenInputs,
        child: &ExecutionControl,
        limit: usize,
    ) -> UnwindNode {
        self.child = Some(child.clone());
        self.calls += 1;
        assert_ne!(self.phase, "execute", "executor unwind");
        UnwindNode {
            value: Some(capture_artifact_result(&output(), recipe, b"manifest", limit).unwrap()),
            panics: self.phase == "take",
        }
    }
}
#[test]
fn caught_unwind_in_each_external_step_keeps_failure_sticky_and_cancels_owned_child() {
    for phase in ["invocation", "capture", "execute", "take", "verify"] {
        let control = GraphControl::default();
        let mut sequence = ArtifactSequence::begin(&control).unwrap();
        let recipe = UnwindRecipe {
            inner: Recipe::new(),
            invocation_panics: phase == "invocation",
        };
        let source = UnwindSource {
            inner: Workspace::new(&[("source", b"source")]),
            at: match phase {
                "capture" => 1,
                "verify" => 3,
                _ => usize::MAX,
            },
        };
        let mut exec = UnwindExecutor {
            phase,
            child: None,
            calls: 0,
        };
        let observed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = sequence.run(&recipe, &source, &mut exec);
        }));
        assert!(observed.is_err(), "{phase}");
        if let Some(child) = &exec.child {
            let observation = madb_app::process::execute_process(
                &NeverRunner,
                &Recipe::new().invocation.process,
                child,
            )
            .unwrap();
            assert_eq!(
                observation.admission,
                madb_app::process::ResultAdmission::RejectedByCancellation,
                "{phase}"
            );
            assert!(observation.process.is_none());
        }
        let calls = exec.calls;
        exec.phase = "normal";
        let next = sequence
            .run(
                &Recipe::new(),
                &Workspace::new(&[("source", b"source")]),
                &mut exec,
            )
            .unwrap();
        assert_eq!(next.status, GraphStatus::Failed, "{phase}");
        assert!(next.node().is_none());
        assert_eq!(exec.calls, calls);
        assert_eq!(sequence.finish(), GraphStatus::Failed);
        assert!(ArtifactSequence::begin(&control).is_err());
    }
}
