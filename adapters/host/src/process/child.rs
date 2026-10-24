use super::io_error;
use madb_app::{
    ports::PendingProcess,
    process::{ProcessError, ProcessExit, ProcessResult},
};
use rustix::process::{
    Pid, Signal, WaitId, WaitIdOptions, kill_process, kill_process_group, waitid,
};
use std::time::Duration;
use subprocess::Job;

/// The leader remains waitable until the last group signal has been sent. No
/// signal is sent using a PGID after reaping its leader (which could be reused).
pub(super) struct OwnedChild {
    child: Option<Job>,
    pid: Pid,
    pub kill_sent: bool,
}
impl OwnedChild {
    pub fn new(child: Job) -> Self {
        let raw = i32::try_from(child.pid()).expect("Unix pid fits i32");
        assert!(raw > 1, "never use PID 0/1 for an owned process group");
        Self {
            child: Some(child),
            pid: Pid::from_raw(raw).expect("positive child pid"),
            kill_sent: false,
        }
    }
    pub fn child_mut(&mut self) -> &mut Job {
        self.child.as_mut().expect("unreaped owner")
    }
    pub fn observe_exit(&mut self) -> ProcessResult<bool> {
        match waitid(
            WaitId::Pid(self.pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
        ) {
            Ok(status) => Ok(status.is_some()),
            Err(rustix::io::Errno::INTR) => Ok(false),
            Err(rustix::io::Errno::CHILD) => {
                // An external waiter broke ownership. Disarm group signalling;
                // this PID might already belong to an unrelated process.
                self.child.take();
                Err(ProcessError::new(
                    "EXEC015",
                    "wait",
                    "child ownership was lost to another waiter",
                ))
            }
            Err(error) => Err(io_error("wait", error.into())),
        }
    }
    pub fn signal(&mut self, signal: Signal) -> ProcessResult<()> {
        if self.child.is_none() {
            return Err(ProcessError::new(
                "EXEC015",
                "signal",
                "child ownership is unavailable",
            ));
        }
        if signal == Signal::KILL {
            self.kill_sent = true;
        }
        match kill_process_group(self.pid, signal) {
            Ok(()) => Ok(()),
            Err(rustix::io::Errno::SRCH) => {
                match kill_process(self.pid, signal) {
                    Ok(()) => Ok(()),
                    // Darwin may report ESRCH for a zombie. Only an owned,
                    // still-waitable exited leader makes this benign; missing
                    // ownership must never authorize another PID signal.
                    Err(rustix::io::Errno::SRCH) if self.observe_exit()? => Ok(()),
                    Err(error) => Err(io_error("signal", error.into())),
                }
            }
            #[cfg(target_os = "macos")]
            Err(rustix::io::Errno::PERM)
                if self.observe_exit()? && self.only_exited_leader_remains()? =>
            {
                Ok(())
            }
            Err(error) => Err(io_error("signal", error.into())),
        }
    }
    #[cfg(target_os = "macos")]
    fn only_exited_leader_remains(&self) -> ProcessResult<bool> {
        use libproc::processes::{ProcFilter, pids_by_type};
        // XNU can report EPERM when it filters out the group's only zombie.
        // Retain that zombie to reserve the PGID. Any other member or query
        // failure remains a cleanup gap, even if that member may also be dead.
        // libproc uses errno to disambiguate a successful empty result.
        errno::set_errno(errno::Errno(0));
        let leader = self.pid.as_raw_nonzero().get() as u32;
        pids_by_type(ProcFilter::ByProgramGroup { pgrpid: leader })
            .map(|pids| pids.iter().all(|pid| *pid == leader))
            .map_err(|e| io_error("group-observation", e))
    }
    pub fn reap(&mut self) -> ProcessResult<Option<ProcessExit>> {
        let Some(child) = self.child.as_mut() else {
            return Ok(None);
        };
        match child
            .wait_timeout(Duration::ZERO)
            .map_err(|e| io_error("reap", e))?
        {
            None => Ok(None),
            Some(status) => {
                self.child.take();
                Ok(Some(ProcessExit {
                    code: status.code().map(|code| code as i32),
                    signal: status.signal(),
                }))
            }
        }
    }
    pub fn still_owned(&self) -> bool {
        self.child.is_some()
    }
}
impl PendingProcess for OwnedChild {
    fn poll_reaped(&mut self) -> ProcessResult<bool> {
        // Before ownership leaves the run loop it has sent its final SIGKILL.
        if !self.kill_sent {
            self.signal(Signal::KILL)?;
        }
        self.reap()?;
        Ok(!self.still_owned())
    }
}
impl Drop for OwnedChild {
    fn drop(&mut self) {
        if self.still_owned() {
            if !self.kill_sent {
                let _ = self.signal(Signal::KILL);
            }
            let _ = self.reap();
        }
    }
}
