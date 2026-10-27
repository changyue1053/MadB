//! One project build signal listener forwards to the currently owned C04
//! admission gate. It never executes IO while holding the registration lock.
use madb_app::use_cases::project_build::{
    BuildFailure as AttemptError, BuildResult as AttemptResult, ProjectBuildControl,
};
use madb_app::{
    ErrorKind, QueryError, Result, pipeline::PipelineControl, process::ExecutionControl,
    transaction::CommitControl,
};
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub(in crate::bootstrap) struct BuildControl {
    pub commit: CommitControl,
    requested: Arc<AtomicBool>,
    active: Arc<Mutex<Option<Active>>>,
}
#[derive(Clone)]
enum Active {
    Pipeline(PipelineControl),
    Process(ExecutionControl),
}
impl Active {
    fn cancel(&self) {
        match self {
            Self::Pipeline(control) => {
                control.cancel();
            }
            Self::Process(control) => {
                control.cancel();
            }
        }
    }
}
impl ProjectBuildControl for BuildControl {
    fn checkpoint(&self) -> AttemptResult<()> {
        BuildControl::checkpoint(self)
    }
    fn commit(&self) -> &CommitControl {
        &self.commit
    }
}
impl BuildControl {
    pub fn checkpoint(&self) -> AttemptResult<()> {
        if self.requested.load(Ordering::Acquire) {
            Err(AttemptError::Cancelled)
        } else {
            Ok(())
        }
    }
    fn lock(&self) -> MutexGuard<'_, Option<Active>> {
        self.active
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub(in crate::bootstrap) fn cancel(&self) {
        if self.commit.cancel() == madb_app::process::CancelDecision::AlreadyFinished {
            return;
        }
        self.requested.store(true, Ordering::Release);
        if let Some(active) = self.lock().as_ref() {
            active.cancel();
        }
    }
    fn register(&self, active: Active) -> AttemptResult<Stage<'_>> {
        let mut slot = self.lock();
        self.checkpoint()?;
        if slot.is_some() {
            return Err(QueryError::new(
                ErrorKind::Host,
                "EXEC001",
                "project build stage",
                "a stage is already active",
            )
            .into());
        }
        *slot = Some(active.clone());
        Ok(Stage {
            owner: self,
            active,
        })
    }
    pub(super) fn process(&self) -> AttemptResult<Stage<'_>> {
        self.register(Active::Process(ExecutionControl::default()))
    }
    pub(super) fn pipeline(&self) -> AttemptResult<Stage<'_>> {
        self.register(Active::Pipeline(PipelineControl::default()))
    }
}
pub(super) struct Stage<'a> {
    owner: &'a BuildControl,
    active: Active,
}
impl Stage<'_> {
    pub fn process(&self) -> &ExecutionControl {
        match &self.active {
            Active::Process(control) => control,
            _ => unreachable!("process stage"),
        }
    }
    pub fn pipeline(&self) -> &PipelineControl {
        match &self.active {
            Active::Pipeline(control) => control,
            _ => unreachable!("pipeline stage"),
        }
    }
}
impl Drop for Stage<'_> {
    fn drop(&mut self) {
        self.owner.lock().take();
    }
}
#[cfg(unix)]
pub(super) fn install_signals(
    control: &BuildControl,
) -> Result<madb_host::cancellation::CancellationSignals> {
    let control = control.clone();
    madb_host::cancellation::CancellationSignals::install_action(move || control.cancel())
}
#[cfg(not(unix))]
pub(super) fn install_signals(
    _: &BuildControl,
) -> Result<madb_host::cancellation::CancellationSignals> {
    Err(QueryError::new(
        ErrorKind::Host,
        "HOST006",
        "project build",
        "signal delivery is not implemented on this host",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use madb_app::process::CancelDecision;
    #[test]
    fn accepted_publication_seals_the_outer_signal_window() {
        let control = BuildControl::default();
        control.commit.begin().unwrap().publish(|| Ok(())).unwrap();
        control.cancel();
        assert!(control.checkpoint().is_ok());
        assert!(!control.commit.rejected_by_cancellation());
    }
    #[test]
    fn cancellation_before_registration_closes_every_stage() {
        let control = BuildControl::default();
        control.cancel();
        assert!(matches!(control.process(), Err(AttemptError::Cancelled)));
        assert!(matches!(control.pipeline(), Err(AttemptError::Cancelled)));
    }
    #[test]
    fn cancellation_is_forwarded_to_the_registered_existing_gate() {
        let control = BuildControl::default();
        let stage = control.process().unwrap();
        control.cancel();
        assert_eq!(stage.process().cancel(), CancelDecision::AlreadyRequested);
        drop(stage);
        assert!(matches!(control.pipeline(), Err(AttemptError::Cancelled)));
        let control = BuildControl::default();
        let stage = control.pipeline().unwrap();
        control.cancel();
        assert_eq!(stage.pipeline().cancel(), CancelDecision::AlreadyRequested);
    }
    #[test]
    fn ordinary_stage_completion_does_not_reuse_a_single_use_gate() {
        let control = BuildControl::default();
        let first = control.process().unwrap();
        assert!(control.process().is_err());
        drop(first);
        let next = control.process().unwrap();
        assert_eq!(next.process().cancel(), CancelDecision::Requested);
    }
}
