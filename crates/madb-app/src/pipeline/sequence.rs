//! Serial admission for dynamically bound artifact requests. This keeps the
//! existing graph gate open between a checked reader result and its generator.
use super::*;
use crate::dag_control::GraphGuard;

/// One graph interval with sticky failure/cancellation and retained step owners.
/// Each step still uses the ordinary artifact capture and validation protocol.
/// This is neither a dependency resolver nor permission to publish a batch.
#[must_use]
pub struct ArtifactSequence<'a> {
    guard: GraphGuard<'a>,
    failed: bool,
}
impl<'a> ArtifactSequence<'a> {
    pub fn begin(control: &'a GraphControl) -> Result<Self> {
        Ok(Self {
            guard: control.begin()?,
            failed: false,
        })
    }

    /// A failed or cancelled earlier step prevents any later capture or launch,
    /// even when the caller ignores that step's status. The returned owner must
    /// be retained while it has unfinished process cleanup.
    pub fn run<R: ArtifactRecipe, W: ReadWorkspace, E: ArtifactExecutor<R>>(
        &mut self,
        recipe: &R,
        source: &W,
        executor: &mut E,
    ) -> Result<ArtifactRun<E::Run>> {
        if !self.failed && !self.guard.cancelled() {
            // Trait calls may unwind. Catching a panic must not reopen this
            // interval or permit another activation to overwrite its child.
            self.failed = true;
            recipe.invocation().validate()?;
            self.failed = false;
        }
        Ok(self.run_checked(recipe, source, executor))
    }

    pub(super) fn run_checked<R: ArtifactRecipe, W: ReadWorkspace, E: ArtifactExecutor<R>>(
        &mut self,
        recipe: &R,
        source: &W,
        executor: &mut E,
    ) -> ArtifactRun<E::Run> {
        let mut run = ArtifactRun {
            status: GraphStatus::Validated,
            failure: None,
            node: None,
            result: None,
            inputs: None,
        };
        let previously_failed = self.failed;
        self.failed = true;
        if previously_failed {
            run.failure = Some(error(
                "PIPE003",
                recipe.invocation().id.as_str(),
                "earlier artifact step failed; sequence admission remains closed",
            ));
        } else if !self.guard.cancelled() {
            match capture_inputs(source, recipe.invocation().inputs.clone()) {
                Err(error) => run.failure = Some(error),
                Ok(inputs) => {
                    if let Some(child) = self.guard.activate() {
                        let active = ActiveStep {
                            guard: &self.guard,
                            child: Some(child),
                        };
                        let mut node = executor.execute(
                            recipe,
                            inputs.clone(),
                            active.child.as_ref().expect("active child"),
                            128 * 1024 * 1024,
                        );
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
                        active.complete();
                    }
                    run.inputs = Some(inputs);
                }
            }
        }
        self.failed = run.failure.is_some();
        run.status = if self.guard.cancelled() {
            GraphStatus::Cancelled
        } else if self.failed {
            GraphStatus::Failed
        } else {
            GraphStatus::Validated
        };
        if run.status != GraphStatus::Validated {
            run.result = None;
        }
        run
    }

    /// Consume the interval at its final cancellation linearization point.
    /// Dropping before this closes the graph and requests active cancellation;
    /// neither action claims that external process cleanup has completed.
    pub fn finish(self) -> GraphStatus {
        if self.guard.finish() {
            GraphStatus::Cancelled
        } else if self.failed {
            GraphStatus::Failed
        } else {
            GraphStatus::Validated
        }
    }
}

// No process wait or completed-cleanup claim: normal returns unregister only.
struct ActiveStep<'a, 'g> {
    guard: &'a GraphGuard<'g>,
    child: Option<ExecutionControl>,
}
impl ActiveStep<'_, '_> {
    fn complete(mut self) {
        self.guard.deactivate();
        self.child.take();
    }
}
impl Drop for ActiveStep<'_, '_> {
    fn drop(&mut self) {
        if let Some(child) = self.child.take() {
            child.cancel();
            self.guard.deactivate();
        }
    }
}
