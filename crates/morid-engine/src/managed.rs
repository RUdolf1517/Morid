//! Process ownership for the cancellation path. All backend execution must use
//! this path rather than low-level SpawnSpec::spawn or Job::assign.
use crate::spawn::{FailedToStart, SpawnSpec};
use std::io;
use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

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

    /// Take each pipe exactly once and start concurrent draining/classification.
    /// If reader setup fails, cancel the backend rather than leave it blocked on
    /// an undrained pipe. Reader threads own their pipe ends, not the child.
    pub fn take_output(
        &mut self,
        rules: crate::stream::Rules,
    ) -> io::Result<crate::stream::OutputStream> {
        if self.child.stdout.is_none() || self.child.stderr.is_none() {
            return Err(io::Error::other("backend pipes already taken"));
        }
        let stdout = self
            .child
            .stdout
            .take()
            .ok_or_else(|| io::Error::other("stdout missing"))?;
        let stderr = self
            .child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("stderr missing"))?;
        match crate::stream::OutputStream::start(stdout, stderr, rules) {
            Ok(output) => Ok(output),
            Err(error) => {
                let _ = self.cancel();
                Err(error)
            }
        }
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

    /// Unlike cancel(), success includes observed tree completion. Unix is
    /// conservative: even a dead orphan zombie prevents confirmation until its
    /// reaper removes it. Timeout/error never means Cancelled; callers can retry
    /// the observation without resending a signal to a reaped leader's id.
    pub fn cancel_observed(&mut self, timeout: Duration) -> io::Result<ExitStatus> {
        let status = self.cancel()?;
        let started = Instant::now();
        loop {
            #[cfg(unix)]
            let empty = crate::process_group::absent_after_kill(self.child.id())?;
            #[cfg(windows)]
            let empty = self.job.is_empty()?;
            if empty {
                return Ok(status);
            }
            if started.elapsed() >= timeout {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "tree death not observed",
                ));
            }
            std::thread::sleep(Duration::from_millis(10));
        }
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
