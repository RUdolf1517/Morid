//! Process ownership for the cancellation path. All backend execution must use
//! this path rather than low-level SpawnSpec::spawn or Job::assign.
use crate::spawn::{FailedToStart, SpawnSpec};
use std::io;
use std::process::{Child, ExitStatus};

pub struct ManagedChild {
    child: Child,
    reaped: Option<ExitStatus>,
    #[cfg(windows)]
    job: crate::job_object::Job,
}

impl ManagedChild {
    pub fn spawn(spec: &SpawnSpec) -> Result<Self, FailedToStart> {
        #[cfg(windows)]
        let job = crate::job_object::Job::new().map_err(|source| FailedToStart {
            program: spec.program.clone(),
            source,
        })?;
        #[cfg(windows)]
        let child = job.spawn(spec).map_err(|source| FailedToStart {
            program: spec.program.clone(),
            source,
        })?;
        #[cfg(unix)]
        let child = spec.spawn()?;
        Ok(Self {
            child,
            reaped: None,
            #[cfg(windows)]
            job,
        })
    }

    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Signal the contained tree and reap its leader. This returns the leader's
    /// status, NOT a service-level Cancelled event: descendant death must still
    /// be observed before publishing that terminal state. Idempotent after reap;
    /// never signal a Unix group id again after releasing its leader's PID.
    pub fn cancel(&mut self) -> io::Result<ExitStatus> {
        if let Some(status) = self.reaped {
            return Ok(status);
        }
        #[cfg(unix)]
        crate::process_group::ProcessGroup::for_child(&self.child).kill_tree()?;
        #[cfg(windows)]
        self.job.terminate(1)?;
        let status = self.child.wait()?;
        self.reaped = Some(status);
        Ok(status)
    }
}

impl Drop for ManagedChild {
    fn drop(&mut self) {
        if self.reaped.is_none() {
            let _ = self.cancel();
        }
        // Windows Job also closes here, killing any surviving members even if
        // an explicit termination failed. No inheritable duplicate is created.
    }
}
