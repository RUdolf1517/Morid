//! FIFO admission with a caller-driven, nonblocking pump. Backend pipes drain
//! concurrently in morid-engine. A pump gives each active job at most 256 output
//! events; inter-job delivery order is JobId order, not a claimed wall clock.
//! A terminal event is last for its job: observe tree death AND drain both pipes
//! before publishing it. Callers own event consumption and pump cadence.
use crate::jobs::{Event, JobFailure, JobId, JobState};
use crate::limit::ConcurrencyLimit;
use morid_engine::managed::{ExitState, ManagedChild};
use morid_engine::output::Classified;
use morid_engine::spawn::SpawnSpec;
use morid_engine::stream::{OutputPoll, OutputStream, Rules};
use std::collections::{BTreeMap, VecDeque};
use std::io;

pub struct JobRequest {
    pub spawn: SpawnSpec,
    pub rules: Rules,
}

struct Active {
    process: ManagedChild,
    output: Option<OutputStream>,
    eof: bool,
    cancelled: bool,
    failure: Option<JobFailure>,
    exit: Option<ExitState>,
}

#[derive(Debug)]
pub enum QueueError {
    UnknownJob(JobId),
    InvalidState(JobState),
    IdExhausted,
    Cancellation { job: JobId, source: io::Error },
}

/// Callers must yield for retry_after on Waiting; no-progress is explicit,
/// rather than indistinguishable from an empty event batch in a busy loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PumpStatus {
    Progress,
    Waiting { retry_after: std::time::Duration },
    Idle,
}

pub struct Queue {
    metadata_jobs: std::collections::BTreeSet<JobId>,
    limit: ConcurrencyLimit,
    next_id: JobId,
    pending: VecDeque<(JobId, JobRequest)>,
    active: BTreeMap<JobId, Active>,
    states: BTreeMap<JobId, JobState>,
    events: VecDeque<Event>,
}

impl Queue {
    pub fn new(limit: ConcurrencyLimit) -> Self {
        Self {
            metadata_jobs: Default::default(),
            limit,
            next_id: 0,
            pending: VecDeque::new(),
            active: BTreeMap::new(),
            states: BTreeMap::new(),
            events: VecDeque::new(),
        }
    }

    pub fn enqueue(&mut self, request: JobRequest) -> Result<JobId, QueueError> {
        let id = self.next_id;
        self.next_id = id.checked_add(1).ok_or(QueueError::IdExhausted)?;
        self.pending.push_back((id, request));
        self.states.insert(id, JobState::Queued);
        self.events.push_back(Event::JobQueued { job: id });
        Ok(id)
    }

    pub fn enqueue_metadata(&mut self, request: JobRequest) -> Result<JobId, QueueError> {
        let id = self.enqueue(request)?;
        self.metadata_jobs.insert(id);
        Ok(id)
    }

    pub fn state(&self, id: JobId) -> Option<JobState> {
        self.states.get(&id).copied()
    }
    pub fn running_count(&self) -> usize {
        self.active.len()
    }
    pub fn is_idle(&self) -> bool {
        self.pending.is_empty() && self.active.is_empty()
    }
    pub fn drain_events(&mut self) -> Vec<Event> {
        self.events.drain(..).collect()
    }

    pub fn cancel(&mut self, id: JobId) -> Result<(), QueueError> {
        let state = self.state(id).ok_or(QueueError::UnknownJob(id))?;
        match state {
            JobState::Queued => {
                // Explicit queued-cancel exception: remove without spawning or
                // creating its destination (§3.2.4), no fake Running/PID event.
                self.metadata_jobs.remove(&id);
                self.pending.retain(|(job, _)| *job != id);
                self.states.insert(id, JobState::Cancelled);
                self.events.push_back(Event::JobFinished {
                    job: id,
                    state: JobState::Cancelled,
                });
                Ok(())
            }
            JobState::Running => {
                let active = self
                    .active
                    .get_mut(&id)
                    .ok_or(QueueError::InvalidState(state))?;
                active.cancelled = true;
                active
                    .process
                    .request_cancel()
                    .map_err(|source| QueueError::Cancellation { job: id, source })
            }
            JobState::Cancelled => Ok(()),
            _ => Err(QueueError::InvalidState(state)),
        }
    }

    /// Attempt every job even if cancellation of one fails. Errors are typed and
    /// returned; failed observations never become fabricated Cancelled events.
    pub fn cancel_all(&mut self) -> Vec<QueueError> {
        let ids = self
            .pending
            .iter()
            .map(|(id, _)| *id)
            .chain(self.active.keys().copied())
            .collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|id| self.cancel(id).err())
            .collect()
    }

    pub fn pump(&mut self) -> PumpStatus {
        let previous_events = self.events.len();
        let mut finished = Vec::new();
        for (&id, active) in &mut self.active {
            if active.cancelled {
                if let Err(error) = active.process.request_cancel() {
                    record_failure(
                        &mut self.events,
                        id,
                        &mut active.failure,
                        JobFailure::Observation {
                            detail: error.to_string(),
                        },
                    );
                }
            }
            if let Some(output) = &mut active.output {
                for _ in 0..256 {
                    match output.poll_next() {
                        OutputPoll::Item(Ok(event)) => match event.data {
                            Classified::Output(line)
                                if self.metadata_jobs.contains(&id)
                                    && event.stream == morid_engine::stream::Stream::Stdout =>
                            {
                                match crate::metadata::MediaMeta::from_yt_dlp(&line) {
                                    Ok(meta) => {
                                        self.events.push_back(Event::JobMetadata { job: id, meta })
                                    }
                                    Err(error) => {
                                        record_failure(
                                            &mut self.events,
                                            id,
                                            &mut active.failure,
                                            JobFailure::Output {
                                                stream: Some(event.stream),
                                                detail: error.to_string(),
                                            },
                                        );
                                        if let Err(error) = active.process.request_abort() {
                                            record_failure(
                                                &mut self.events,
                                                id,
                                                &mut active.failure,
                                                JobFailure::Observation {
                                                    detail: error.to_string(),
                                                },
                                            );
                                        }
                                    }
                                }
                            }
                            Classified::Output(line) => self.events.push_back(Event::JobOutput {
                                job: id,
                                stream: event.stream,
                                line,
                            }),
                            Classified::Progress(progress) => self
                                .events
                                .push_back(Event::JobProgress { job: id, progress }),
                        },
                        OutputPoll::Item(Err(error)) => {
                            record_failure(
                                &mut self.events,
                                id,
                                &mut active.failure,
                                JobFailure::Output {
                                    stream: Some(error.stream),
                                    detail: error.source.to_string(),
                                },
                            );
                            if let Err(error) = active.process.request_abort() {
                                record_failure(
                                    &mut self.events,
                                    id,
                                    &mut active.failure,
                                    JobFailure::Observation {
                                        detail: error.to_string(),
                                    },
                                );
                            }
                        }
                        OutputPoll::Pending => break,
                        OutputPoll::End => {
                            active.eof = true;
                            break;
                        }
                    }
                }
            }
            if active.exit.is_none() {
                match active.process.poll_finished() {
                    Ok(exit) => active.exit = exit,
                    Err(error) => record_failure(
                        &mut self.events,
                        id,
                        &mut active.failure,
                        JobFailure::Observation {
                            detail: error.to_string(),
                        },
                    ),
                }
            }
            if active.eof && active.exit.is_some() {
                finished.push(id);
            }
        }
        for id in finished {
            self.metadata_jobs.remove(&id);
            if let Some(active) = self.active.remove(&id) {
                if let Some(mut exit) = active.exit {
                    exit.cancelled = active.cancelled;
                    let state = if active.cancelled {
                        JobState::Cancelled
                    } else if active.failure.is_some()
                        || exit.code != Some(0)
                        || exit.signal.is_some()
                    {
                        JobState::Failed
                    } else {
                        JobState::Succeeded
                    };
                    if state == JobState::Failed && active.failure.is_none() {
                        let failure = JobFailure::BackendExit {
                            code: exit.code,
                            signal: exit.signal,
                        };
                        self.events.push_back(failure.notice(id));
                        self.events.push_back(Event::JobError { job: id, failure });
                    }
                    self.events.push_back(Event::JobExited { job: id, exit });
                    self.states.insert(id, state);
                    self.events.push_back(Event::JobFinished { job: id, state });
                }
            }
        }
        while self.active.len() < self.limit.get() {
            let Some((id, request)) = self.pending.pop_front() else {
                break;
            };
            self.states.insert(id, JobState::Running);
            if let Err(error) = std::fs::create_dir_all(&request.spawn.cwd) {
                self.failed_admission(
                    id,
                    JobFailure::Destination {
                        path: request.spawn.cwd,
                        detail: error.to_string(),
                    },
                );
                continue;
            }
            let mut process = match ManagedChild::spawn(&request.spawn) {
                Ok(child) => child,
                Err(error) => {
                    self.failed_admission(
                        id,
                        JobFailure::FailedToStart {
                            program: error.program,
                            detail: error.source.to_string(),
                        },
                    );
                    continue;
                }
            };
            self.events.push_back(Event::JobStarted {
                job: id,
                pid: process.id(),
            });
            let mut failure = None;
            let output = match process.take_output(request.rules) {
                Ok(output) => Some(output),
                Err(error) => {
                    record_failure(
                        &mut self.events,
                        id,
                        &mut failure,
                        JobFailure::Output {
                            stream: None,
                            detail: error.to_string(),
                        },
                    );
                    None
                }
            };
            let eof = output.is_none();
            self.active.insert(
                id,
                Active {
                    process,
                    output,
                    eof,
                    cancelled: false,
                    failure,
                    exit: None,
                },
            );
        }
        if self.events.len() != previous_events {
            PumpStatus::Progress
        } else if self.is_idle() {
            PumpStatus::Idle
        } else {
            PumpStatus::Waiting {
                retry_after: std::time::Duration::from_millis(5),
            }
        }
    }

    fn failed_admission(&mut self, id: JobId, failure: JobFailure) {
        self.metadata_jobs.remove(&id);
        self.events.push_back(failure.notice(id));
        self.events.push_back(Event::JobError { job: id, failure });
        self.states.insert(id, JobState::Failed);
        self.events.push_back(Event::JobFinished {
            job: id,
            state: JobState::Failed,
        });
    }
}

fn record_failure(
    events: &mut VecDeque<Event>,
    id: JobId,
    stored: &mut Option<JobFailure>,
    failure: JobFailure,
) {
    if stored.is_none() {
        events.push_back(failure.notice(id));
        events.push_back(Event::JobError {
            job: id,
            failure: failure.clone(),
        });
        *stored = Some(failure);
    }
}
