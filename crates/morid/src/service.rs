//! One command receiver and one event stream, independent of CLI/GUI rendering.
//! A worker owns the queue; clients cannot mutate its state. Commands and job
//! output each have a bounded per-turn processing budget. Idle workers block on
//! commands; active workers use Queue's retry delay, never an empty busy loop.
//!
//! Events use std's unbounded channel so a slow renderer cannot block a Cancel
//! command behind a full event buffer. Consumers must continuously drain it;
//! disk-backed/bounded log retention belongs to the consumer, not this transport.
//! Shutdown is explicit and observed. Dropping the receiver is abort/best-effort
//! cleanup, not evidence of cancellation. No terminal event is fabricated then.
use crate::jobs::{Event, JobId};
use crate::limit::ConcurrencyLimit;
use crate::queue::{JobRequest, PumpStatus, Queue, QueueError};
use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

pub enum Command {
    Enqueue {
        request: JobRequest,
        reply: Sender<Result<JobId, Rejection>>,
    },
    Cancel(JobId),
    CancelAll,
    Shutdown,
}

#[derive(Debug)]
pub enum Rejection {
    Stopping,
    Queue(QueueError),
}

#[derive(Debug)]
pub enum ServiceEvent {
    Job(Event),
    CommandRejected {
        job: Option<JobId>,
        reason: Rejection,
    },
    /// False means the shutdown deadline expired, NOT tree death confirmation.
    Stopped {
        clean: bool,
    },
}

#[derive(Clone)]
pub struct Client(Sender<Command>);

impl Client {
    pub fn send(&self, command: Command) -> Result<(), mpsc::SendError<Command>> {
        self.0.send(command)
    }

    /// The asynchronous reply avoids deadlocking the worker on an enqueue ack.
    pub fn enqueue(
        &self,
        request: JobRequest,
    ) -> Result<Receiver<Result<JobId, Rejection>>, mpsc::SendError<Command>> {
        let (reply, receiver) = mpsc::channel();
        self.send(Command::Enqueue { request, reply })?;
        Ok(receiver)
    }
}

pub struct Service {
    pub client: Client,
    pub events: Receiver<ServiceEvent>,
    worker: Option<JoinHandle<()>>,
}

impl Service {
    pub fn start(limit: ConcurrencyLimit) -> io::Result<Self> {
        let (commands, receiver) = mpsc::channel();
        let (sender, events) = mpsc::channel();
        let worker = thread::Builder::new()
            .name("morid-service".into())
            .spawn(move || run(Queue::new(limit), receiver, sender))?;
        Ok(Self {
            client: Client(commands),
            events,
            worker: Some(worker),
        })
    }

    /// Request shutdown, consume events through Stopped, then join. Joining
    /// itself does not consume events: the receiver remains available afterwards.
    pub fn shutdown(&self) -> Result<(), mpsc::SendError<Command>> {
        self.client.send(Command::Shutdown)
    }

    pub fn join(&mut self) -> thread::Result<()> {
        if let Some(worker) = self.worker.take() {
            worker.join()
        } else {
            Ok(())
        }
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.shutdown();
        // Do not block a UI destructor on conservative orphan-zombie observation.
        // The worker sees receiver disconnect on its next send and drops Queue,
        // whose ManagedChild owners perform best-effort tree cleanup.
    }
}

fn reject(events: &Sender<ServiceEvent>, job: Option<JobId>, reason: Rejection) {
    let _ = events.send(ServiceEvent::CommandRejected { job, reason });
}

fn cancel_all(queue: &mut Queue, events: &Sender<ServiceEvent>) {
    for error in queue.cancel_all() {
        reject(events, None, Rejection::Queue(error));
    }
}

fn run(mut queue: Queue, commands: Receiver<Command>, events: Sender<ServiceEvent>) {
    let mut stopping: Option<Instant> = None;
    let mut pending = None;
    loop {
        for _ in 0..64 {
            let command = match pending
                .take()
                .map(Ok)
                .unwrap_or_else(|| commands.try_recv())
            {
                Ok(command) => command,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    if stopping.is_none() {
                        stopping = Some(Instant::now());
                        cancel_all(&mut queue, &events);
                    }
                    break;
                }
            };
            match command {
                Command::Enqueue { request, reply } => {
                    let result = if stopping.is_some() {
                        Err(Rejection::Stopping)
                    } else {
                        queue.enqueue(request).map_err(Rejection::Queue)
                    };
                    // Dropped reply does not undo an accepted job; JobQueued is
                    // still emitted on the canonical event stream.
                    let _ = reply.send(result);
                }
                Command::Cancel(job) => {
                    if let Err(error) = queue.cancel(job) {
                        reject(&events, Some(job), Rejection::Queue(error));
                    }
                }
                Command::CancelAll => cancel_all(&mut queue, &events),
                Command::Shutdown => {
                    if stopping.is_none() {
                        stopping = Some(Instant::now());
                        cancel_all(&mut queue, &events);
                    }
                }
            }
        }
        let status = queue.pump();
        for event in queue.drain_events() {
            if events.send(ServiceEvent::Job(event)).is_err() {
                return;
            }
        }
        if let Some(start) = stopping {
            if queue.is_idle() || start.elapsed() >= Duration::from_secs(15) {
                let _ = events.send(ServiceEvent::Stopped {
                    clean: queue.is_idle(),
                });
                return;
            }
        }
        let delay = match status {
            PumpStatus::Idle if stopping.is_none() => {
                match commands.recv() {
                    Ok(command) => pending = Some(command),
                    Err(_) => {
                        stopping = Some(Instant::now());
                        cancel_all(&mut queue, &events);
                    }
                }
                continue;
            }
            PumpStatus::Waiting { retry_after } => retry_after,
            // A hot queue must yield to pipe readers and command producers too.
            _ => Duration::from_millis(1),
        };
        match commands.recv_timeout(delay) {
            Ok(command) => pending = Some(command),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => thread::sleep(delay),
        }
    }
}
