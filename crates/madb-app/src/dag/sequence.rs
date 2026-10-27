//! Bind a later static graph only after observing the preceding real outputs.
//! One existing cancellation interval remains open through those binding gaps.
use super::*;

#[must_use]
pub struct GraphSequence<'a> {
    guard: GraphGuard<'a>,
    failed: bool,
    remaining_bytes: usize,
    tasks: BTreeSet<Id>,
}

impl<'a> GraphSequence<'a> {
    pub fn begin(control: &'a GraphControl) -> Result<Self> {
        Ok(Self {
            guard: control.begin()?,
            failed: false,
            remaining_bytes: GRAPH_BYTES_LIMIT,
            tasks: BTreeSet::new(),
        })
    }

    /// Each graph still undergoes the ordinary capture, result and byte checks.
    /// Retain returned node owners until their process cleanup has completed.
    /// A step result is not permission to commit; finish and the existing commit
    /// gate still decide final admission. Failure/unwind closes later steps.
    pub fn run<R: GraphRecipe, W: ReadWorkspace, E: GraphExecutor<R>>(
        &mut self,
        graph: &ExecutionGraph<R>,
        source: &W,
        executor: &mut E,
    ) -> Result<GraphRun<E::Run>> {
        if self.failed {
            return Err(graph_error(
                "DAG003",
                "sequence",
                "earlier graph failed; admission remains closed",
            ));
        }
        self.failed = true;
        if self.tasks.len() + graph.tasks.len() > NODE_LIMIT
            || graph.tasks.iter().any(|task| self.tasks.contains(&task.id))
        {
            return Err(graph_error(
                "DAG002",
                "sequence",
                "sequence tasks repeat or exceed the node budget",
            ));
        }
        let run = execute_graph_step(
            graph,
            source,
            executor,
            &self.guard,
            None,
            self.remaining_bytes,
        );
        if run.status == GraphStatus::Validated {
            let results_bytes: usize = run
                .results
                .as_ref()
                .expect("validated results")
                .values()
                .map(|result| result.files().total_bytes())
                .sum();
            // run_graph and check_result already bounded each subtraction.
            self.remaining_bytes -= graph.reserved_bytes + results_bytes;
            self.tasks
                .extend(graph.tasks.iter().map(|task| task.id.clone()));
            self.failed = false;
        }
        Ok(run)
    }

    /// Final linearization uses the same graph guard as every child process.
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
