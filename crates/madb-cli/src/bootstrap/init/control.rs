//! One initialization signal listener forwards to the currently owned C04
//! admission gate. It never executes IO while holding the registration lock.
use super::{AttemptError, AttemptResult};
use madb_app::{
    ErrorKind, QueryError, Result, pipeline::PipelineControl, process::ExecutionControl,
    transaction::CommitControl,
};
use std::sync::{
    Arc, Mutex, MutexGuard,
    atomic::{AtomicBool, Ordering},
};

#[derive(Clone, Default)]
pub(super) struct InitControl {
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
    pub fn process(&self) -> AttemptResult<Stage<'_>> {
        self.register(Active::Process(ExecutionControl::default()))
    }
    pub fn pipeline(&self) -> AttemptResult<Stage<'_>> {
        self.register(Active::Pipeline(PipelineControl::default()))
    }
}
pub(super) struct Stage<'a> {
    owner: &'a InitControl,
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
