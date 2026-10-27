//! One initialization signal listener forwards to the currently owned C04
//! admission gate. It never executes IO while holding the registration lock.
use super::{AttemptError, AttemptResult};
use crate::bootstrap::project::control::BuildControl;
use madb_app::{
    ErrorKind, QueryError, Result, pipeline::PipelineControl, process::ExecutionControl,
    transaction::CommitControl,
};
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub(in crate::bootstrap) struct InitControl {
    pub commit: CommitControl,
    requested: Arc<AtomicBool>,
    active: Arc<Mutex<Option<Active>>>,
}
#[derive(Clone)]
enum Active {
    Pipeline(PipelineControl),
    Process(ExecutionControl),
    Project(BuildControl),
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
            Self::Project(control) => {
                control.cancel();
            }
        }
    }
}
impl InitControl {
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
    fn cancel(&self) {
        self.requested.store(true, Ordering::Release);
        self.commit.cancel();
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
                "initialization stage",
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
    pub fn project(&self) -> AttemptResult<Stage<'_>> {
        self.register(Active::Project(BuildControl::default()))
    }
    pub fn process(&self) -> AttemptResult<Stage<'_>> {
        self.register(Active::Process(ExecutionControl::default()))
    }
    pub fn pipeline(&self) -> AttemptResult<Stage<'_>> {
        self.register(Active::Pipeline(PipelineControl::default()))
    }
}
pub(in crate::bootstrap) struct Stage<'a> {
    owner: &'a InitControl,
    active: Active,
}
impl Stage<'_> {
    pub fn project(&self) -> &BuildControl {
        match &self.active {
            Active::Project(control) => control,
            _ => unreachable!("project stage"),
        }
    }

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
    control: &InitControl,
) -> Result<madb_host::cancellation::CancellationSignals> {
    let control = control.clone();
    madb_host::cancellation::CancellationSignals::install_action(move || control.cancel())
}
#[cfg(not(unix))]
pub(super) fn install_signals(
    _: &InitControl,
) -> Result<madb_host::cancellation::CancellationSignals> {
    Err(QueryError::new(
        ErrorKind::Host,
        "HOST006",
        "initialization",
        "signal delivery is not implemented on this host",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use madb_app::process::CancelDecision;
    #[test]
    fn outer_signal_rejects_the_current_project_commit_gate() {
        let control = InitControl::default();
        let step = control.project().unwrap();
        let permit = step.project().commit.begin().unwrap();
        control.cancel();
        assert!(permit.publish(|| Ok(())).is_err());
        assert!(step.project().commit.rejected_by_cancellation());
    }
    #[test]
    fn accepted_project_step_keeps_its_fact_but_outer_cancel_blocks_next_step() {
        let control = InitControl::default();
        let stage = control.project().unwrap();
        stage
            .project()
            .commit
            .begin()
            .unwrap()
            .publish(|| Ok(()))
            .unwrap();
        control.cancel();
        assert!(stage.project().checkpoint().is_ok());
        assert!(!stage.project().commit.rejected_by_cancellation());
        drop(stage);
        assert!(matches!(control.project(), Err(AttemptError::Cancelled)));
        assert!(matches!(control.process(), Err(AttemptError::Cancelled)));
    }
    #[test]
    fn cancel_between_project_steps_is_sticky_before_registration() {
        let control = InitControl::default();
        let stage = control.project().unwrap();
        stage
            .project()
            .commit
            .begin()
            .unwrap()
            .publish(|| Ok(()))
            .unwrap();
        drop(stage);
        control.cancel();
        assert!(matches!(control.project(), Err(AttemptError::Cancelled)));
    }
    #[test]
    fn cancellation_before_registration_closes_every_stage() {
        let control = InitControl::default();
        control.cancel();
        assert!(matches!(control.process(), Err(AttemptError::Cancelled)));
        assert!(matches!(control.pipeline(), Err(AttemptError::Cancelled)));
    }
    #[test]
    fn cancellation_is_forwarded_to_the_registered_existing_gate() {
        let control = InitControl::default();
        let stage = control.process().unwrap();
        control.cancel();
        assert_eq!(stage.process().cancel(), CancelDecision::AlreadyRequested);
        drop(stage);
        assert!(matches!(control.pipeline(), Err(AttemptError::Cancelled)));
        let control = InitControl::default();
        let stage = control.pipeline().unwrap();
        control.cancel();
        assert_eq!(stage.pipeline().cancel(), CancelDecision::AlreadyRequested);
    }
    #[test]
    fn ordinary_stage_completion_does_not_reuse_a_single_use_gate() {
        let control = InitControl::default();
        let first = control.process().unwrap();
        assert!(control.process().is_err());
        drop(first);
        let next = control.process().unwrap();
        assert_eq!(next.process().cancel(), CancelDecision::Requested);
    }
}
