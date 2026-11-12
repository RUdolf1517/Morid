//! Service cancellation terminal-state gate. A sent signal or leader exit is
//! deliberately insufficient; only the engine's observed tree death unlocks it.
use morid_engine::managed::ExitState;
use morid_engine::managed::ManagedChild;
use morid_engine::output::Progress;
use morid_engine::spawn::{FailedToStart, SpawnSpec};
use morid_engine::stream::Stream;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

pub type JobId = u64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Event {
    JobQueued {
        job: JobId,
    },
    JobStarted {
        job: JobId,
        pid: u32,
    },
    JobOutput {
        job: JobId,
        stream: Stream,
        line: String,
    },
    JobProgress {
        job: JobId,
        progress: Progress,
    },
    JobExited {
        job: JobId,
        exit: ExitState,
    },
    Notice {
        job: JobId,
        key: &'static str,
        args: std::collections::BTreeMap<String, String>,
    },
    JobError {
        job: JobId,
        failure: JobFailure,
    },
    JobFinished {
        job: JobId,
        state: JobState,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JobFailure {
    Destination {
        path: PathBuf,
        detail: String,
    },
    FailedToStart {
        program: PathBuf,
        detail: String,
    },
    Output {
        stream: Option<Stream>,
        detail: String,
    },
    BackendExit {
        code: Option<i32>,
        signal: Option<i32>,
    },
    Observation {
        detail: String,
    },
}

impl JobFailure {
    pub fn notice(&self, job: JobId) -> Event {
        let key = match self {
            Self::Destination { .. } => "job.destination_failed",
            Self::FailedToStart { .. } => "job.failed_to_start",
            Self::Output { .. } => "job.output_failed",
            Self::BackendExit { .. } => "job.backend_exit",
            Self::Observation { .. } => "job.observation_failed",
        };
        // Technical details are arguments, never the translated Notice text.
        let mut args = std::collections::BTreeMap::new();
        args.insert("detail".into(), format!("{self:?}"));
        Event::Notice { job, key, args }
    }

    pub fn exit_code(&self) -> u8 {
        if matches!(self, Self::Destination { .. }) {
            6
        } else {
            3
        }
    }
}

#[derive(Debug)]
pub enum CancelError {
    InvalidState(JobState),
    Observation(io::Error),
}

pub struct RunningJob {
    id: JobId,
    state: JobState,
    process: ManagedChild,
    events: Vec<Event>,
}

impl RunningJob {
    pub fn start(id: JobId, spec: &SpawnSpec) -> Result<Self, FailedToStart> {
        let process = ManagedChild::spawn(spec)?;
        let events = vec![Event::JobStarted {
            job: id,
            pid: process.id(),
        }];
        Ok(Self {
            id,
            state: JobState::Running,
            process,
            events,
        })
    }

    pub fn state(&self) -> JobState {
        self.state
    }

    pub fn drain_events(&mut self) -> Vec<Event> {
        std::mem::take(&mut self.events)
    }

    pub fn cancel(&mut self, timeout: Duration) -> Result<(), CancelError> {
        let process = &mut self.process;
        finish_cancellation(self.id, &mut self.state, &mut self.events, || {
            process.cancel_observed(timeout).map(|_| ())
        })
    }
}

fn finish_cancellation(
    id: JobId,
    state: &mut JobState,
    events: &mut Vec<Event>,
    observe: impl FnOnce() -> io::Result<()>,
) -> Result<(), CancelError> {
    if *state == JobState::Cancelled {
        return Ok(());
    }
    if *state != JobState::Running {
        return Err(CancelError::InvalidState(*state));
    }
    observe().map_err(CancelError::Observation)?;
    *state = JobState::Cancelled;
    events.push(Event::JobFinished {
        job: id,
        state: JobState::Cancelled,
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cancelled_event_requires_successful_death_observation() {
        let mut state = JobState::Running;
        let mut events = Vec::new();
        let failed = finish_cancellation(7, &mut state, &mut events, || {
            Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "descendant still present",
            ))
        });
        assert!(matches!(failed, Err(CancelError::Observation(_))));
        assert_eq!(state, JobState::Running);
        assert!(
            events.is_empty(),
            "a failed observation must not publish Cancelled"
        );
        finish_cancellation(7, &mut state, &mut events, || Ok(())).expect("death observed");
        assert_eq!(
            events,
            [Event::JobFinished {
                job: 7,
                state: JobState::Cancelled
            }]
        );
        finish_cancellation(7, &mut state, &mut events, || {
            panic!("must not repeat cancellation")
        })
        .expect("idempotent");
        assert_eq!(events.len(), 1);
    }

    #[test]
    fn cancellation_does_not_rewrite_another_terminal_state() {
        for mut state in [JobState::Queued, JobState::Succeeded, JobState::Failed] {
            let mut events = Vec::new();
            assert!(matches!(
                finish_cancellation(8, &mut state, &mut events, || panic!("invalid state")),
                Err(CancelError::InvalidState(_))
            ));
            assert!(events.is_empty());
        }
    }
}
