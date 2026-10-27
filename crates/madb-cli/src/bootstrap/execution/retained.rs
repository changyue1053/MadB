//! Concrete process owners survive execution errors through result persistence.
use super::Journal;
use madb_app::{
    dag::GraphRun,
    pipeline::ArtifactRun,
    process::{ProcessExecution, ProcessResult},
};
use madb_host::{artifact::LocalArtifactRun, dag::LocalNodeRun};
use std::ops::Deref;

pub(in crate::bootstrap) enum RetainedExecution {
    Artifact(Box<ArtifactRun<LocalArtifactRun>>),
    Graph(GraphRun<LocalNodeRun>),
    Process(ProcessExecution),
}
impl RetainedExecution {
    pub(in crate::bootstrap) fn poll(&mut self) -> Vec<String> {
        match self {
            Self::Artifact(run) => run
                .node_mut()
                .map(|node| vec![format!("artifact poll={:?}", node.poll_cleanup())])
                .unwrap_or_default(),
            Self::Graph(run) => run
                .nodes_mut()
                .iter_mut()
                .map(|(id, node)| format!("task={id} poll={:?}", node.poll_cleanup()))
                .collect(),
            Self::Process(execution) => execution
                .process
                .as_mut()
                .map(|run| vec![format!("process poll={:?}", run.poll_cleanup())])
                .unwrap_or_default(),
        }
    }
    pub(in crate::bootstrap) fn pending(&self) -> bool {
        let pending = |value: Option<&ProcessResult<ProcessExecution>>| {
            value
                .and_then(|v| v.as_ref().ok())
                .and_then(|v| v.process.as_ref())
                .is_some_and(|v| v.pending_cleanup.is_some())
        };
        match self {
            Self::Artifact(run) => pending(run.node().and_then(LocalArtifactRun::execution)),
            Self::Graph(run) => run
                .nodes()
                .iter()
                .any(|(_, node)| pending(node.execution())),
            Self::Process(execution) => execution
                .process
                .as_ref()
                .is_some_and(|p| p.pending_cleanup.is_some()),
        }
    }
}
/// All `?` paths preserve the same concrete owner; legacy CLI calls without a
/// journal retain their previous scope. No new background reaper is introduced.
pub(super) struct RetainOnReturn<'a, T> {
    run: Option<T>,
    journal: Journal<'a>,
    wrap: fn(T) -> RetainedExecution,
}
impl<'a, T> RetainOnReturn<'a, T> {
    pub(super) fn new(run: T, journal: Journal<'a>, wrap: fn(T) -> RetainedExecution) -> Self {
        Self {
            run: Some(run),
            journal,
            wrap,
        }
    }
}
impl<T> Deref for RetainOnReturn<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self.run.as_ref().expect("owned run")
    }
}
impl<T> Drop for RetainOnReturn<'_, T> {
    fn drop(&mut self) {
        if let Some(journal) = self.journal {
            journal
                .borrow_mut()
                .retained
                .push((self.wrap)(self.run.take().expect("owned run")));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use madb_app::{ports::PendingProcess, process::*};
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };
    struct Pending {
        polls: Arc<AtomicUsize>,
        drops: Arc<AtomicUsize>,
    }
    impl PendingProcess for Pending {
        fn poll_reaped(&mut self) -> ProcessResult<bool> {
            Ok(self.polls.fetch_add(1, Ordering::SeqCst) >= 1)
        }
    }
    impl Drop for Pending {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }
    #[test]
    fn early_error_transfers_pending_owner_until_explicit_poll_after_report_work() {
        let polls = Arc::new(AtomicUsize::new(0));
        let drops = Arc::new(AtomicUsize::new(0));
        let journal = std::cell::RefCell::new(super::super::ExecutionJournal::default());
        let process = ProcessExecution {
            admission: ResultAdmission::Unavailable,
            process: Some(ProcessRun {
                report: ProcessReport {
                    termination: Termination::Stopped {
                        reason: StopReason::Deadline,
                        forced: true,
                        exit: None,
                    },
                    stdout: CapturedOutput::default(),
                    stderr: CapturedOutput::default(),
                    stdin_written: 0,
                    cleanup: ProcessCleanup::default(),
                    diagnostic: None,
                },
                pending_cleanup: Some(Box::new(Pending {
                    polls: polls.clone(),
                    drops: drops.clone(),
                })),
            }),
        };
        let early_error: Result<(), ()> = {
            let _run = RetainOnReturn::new(process, Some(&journal), RetainedExecution::Process);
            Err(())
        };
        assert!(early_error.is_err());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        let mut retained = journal.into_inner().retained;
        assert!(retained[0].pending());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        let first = retained[0].poll();
        assert!(first[0].contains("false"));
        assert!(retained[0].pending());
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        // This point represents the caller retaining ownership during evidence
        // persistence. Only an actual successful poll acknowledges reaping.
        let second = retained[0].poll();
        assert!(second[0].contains("true"));
        assert!(!retained[0].pending());
        assert_eq!(drops.load(Ordering::SeqCst), 1);
    }
}
