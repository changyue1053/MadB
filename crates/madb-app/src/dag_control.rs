//! One graph admission interval, including gaps between its child processes.
use std::sync::{Arc, Mutex, MutexGuard};

use crate::{
    ErrorKind, QueryError, Result,
    process::{CancelDecision, ExecutionControl},
};

#[derive(Clone, Copy, Default, Eq, PartialEq)]
enum Phase {
    #[default]
    Ready,
    Running,
    Finished,
}

#[derive(Default)]
struct State {
    phase: Phase,
    cancelled: bool,
    active: Option<ExecutionControl>,
}

/// Single-use graph admission. Cancellation closes the whole graph even when
/// its most recent child has finished. It does not interrupt synchronous IO.
/// The mutex protects state transitions only; no port or filesystem calls run
/// under it. Child cancellation is a bounded atomic operation.
#[derive(Clone, Default)]
pub struct GraphControl {
    state: Arc<Mutex<State>>,
}
impl GraphControl {
    pub fn cancel(&self) -> CancelDecision {
        let mut state = self.lock();
        if state.phase == Phase::Finished {
            return CancelDecision::AlreadyFinished;
        }
        if state.cancelled {
            return CancelDecision::AlreadyRequested;
        }
        state.cancelled = true;
        if let Some(active) = &state.active {
            active.cancel();
        }
        CancelDecision::Requested
    }

    pub(crate) fn begin(&self) -> Result<GraphGuard<'_>> {
        let mut state = self.lock();
        if state.phase != Phase::Ready {
            return Err(QueryError::new(
                ErrorKind::Conflict,
                "DAG001",
                "admission",
                "graph control has already been used",
            ));
        }
        state.phase = Phase::Running;
        Ok(GraphGuard {
            control: self,
            finished: false,
        })
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        // No external callbacks or fallible work can poison this short lock.
        // If unwinding nevertheless did so, retaining cancellation is safer
        // than reopening admission or panicking during the guard's Drop.
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

pub(crate) struct GraphGuard<'a> {
    control: &'a GraphControl,
    finished: bool,
}
impl GraphGuard<'_> {
    pub(crate) fn cancelled(&self) -> bool {
        self.control.lock().cancelled
    }

    pub(crate) fn activate(&self) -> Option<ExecutionControl> {
        let mut state = self.control.lock();
        if state.cancelled {
            return None;
        }
        let active = ExecutionControl::default();
        state.active = Some(active.clone());
        Some(active)
    }

    pub(crate) fn deactivate(&self) {
        self.control.lock().active = None;
    }

    /// The final cancellation/acceptance linearization point.
    pub(crate) fn finish(mut self) -> bool {
        let mut state = self.control.lock();
        state.phase = Phase::Finished;
        state.active = None;
        self.finished = true;
        state.cancelled
    }
}
impl Drop for GraphGuard<'_> {
    fn drop(&mut self) {
        if !self.finished {
            let mut state = self.control.lock();
            state.phase = Phase::Finished;
            if let Some(active) = state.active.take() {
                active.cancel();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancellation_before_and_after_active_registration_closes_graph_admission() {
        for cancel_first in [false, true] {
            let control = GraphControl::default();
            let guard = control.begin().unwrap();
            if cancel_first {
                assert_eq!(control.cancel(), CancelDecision::Requested);
                assert!(guard.activate().is_none());
            } else {
                let node = guard.activate().unwrap();
                assert_eq!(control.cancel(), CancelDecision::Requested);
                assert_eq!(node.cancel(), CancelDecision::AlreadyRequested);
            }
            assert_eq!(control.cancel(), CancelDecision::AlreadyRequested);
            assert!(guard.finish());
            assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
            assert!(control.begin().is_err());
        }
    }

    #[test]
    fn between_nodes_and_final_acceptance_use_the_same_graph_gate() {
        let control = GraphControl::default();
        let guard = control.begin().unwrap();
        guard.activate().unwrap();
        guard.deactivate();
        assert_eq!(control.cancel(), CancelDecision::Requested);
        assert!(guard.activate().is_none());
        assert!(guard.finish());

        let control = GraphControl::default();
        let guard = control.begin().unwrap();
        assert!(!guard.finish());
        assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
    }

    #[test]
    fn unwind_guard_closes_gate_and_requests_active_cleanup() {
        let control = GraphControl::default();
        let node;
        {
            let guard = control.begin().unwrap();
            node = guard.activate().unwrap();
        }
        assert_eq!(node.cancel(), CancelDecision::AlreadyRequested);
        assert_eq!(control.cancel(), CancelDecision::AlreadyFinished);
        assert!(control.begin().is_err());
    }
}
