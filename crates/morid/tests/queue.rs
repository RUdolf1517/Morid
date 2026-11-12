//! Offline admission/lifecycle regression; completion is gated by disk ACKs.
use morid::jobs::{Event, JobState};
use morid::limit::ConcurrencyLimit;
use morid::queue::{JobRequest, Queue};
use morid_engine::spawn::SpawnSpec;
use morid_engine::stream::Rules;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[test]
fn fifo_queue_limit_and_terminal_output_order() {
    let root = std::env::temp_dir().join(format!("morid-queue-{:?}", std::thread::current().id()));
    std::fs::create_dir_all(&root).expect("fixture directory");
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/queue_backend.py");
    let mut queue = Queue::new(ConcurrencyLimit::new(1).expect("limit"));
    let mut ids = Vec::new();
    for job in ["a", "b"] {
        ids.push(
            queue
                .enqueue(JobRequest {
                    spawn: SpawnSpec {
                        program: PathBuf::from(if cfg!(windows) { "python" } else { "python3" }),
                        args: vec![
                            script.as_os_str().into(),
                            "gate".into(),
                            root.as_os_str().into(),
                            job.into(),
                        ],
                        cwd: root.join(job),
                        env: vec![],
                    },
                    rules: Rules {
                        split_lines_by: vec!["\n".into()],
                        remove_text: vec![],
                        skip_line_with_text: vec![],
                        control: morid_ext::control::Control::parse(&serde_json::json!({}))
                            .expect("control"),
                    },
                })
                .expect("enqueue"),
        );
    }
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut events = Vec::new();
    while !queue.is_idle() {
        queue.pump();
        events.extend(queue.drain_events());
        assert!(queue.running_count() <= 1);
        if root.join("pid-a").exists() {
            if queue.state(ids[0]) == Some(JobState::Running) {
                assert_eq!(queue.state(ids[1]), Some(JobState::Queued));
                assert!(!root.join("pid-b").exists());
            }
            std::fs::write(root.join("release-a"), b"").expect("release a");
        }
        if root.join("pid-b").exists() {
            assert_eq!(queue.state(ids[0]), Some(JobState::Succeeded));
            std::fs::write(root.join("release-b"), b"").expect("release b");
        }
        if Instant::now() >= deadline {
            let _ = queue.cancel_all();
            std::fs::write(root.join("stop"), b"").expect("stop watchdog");
            panic!("queue watchdog: {events:?}");
        }
        std::thread::sleep(Duration::from_millis(5));
    }
    for (id, job) in ids.into_iter().zip(["a", "b"]) {
        assert_eq!(queue.state(id), Some(JobState::Succeeded));
        let tail = events.iter().position(|event| matches!(event,
            Event::JobOutput { job: current, line, .. } if *current == id && line == &format!("tail-{job}"))).expect("EOF tail");
        let terminal = events
            .iter()
            .position(|event| {
                matches!(event,
            Event::JobFinished { job, .. } if *job == id)
            })
            .expect("terminal");
        assert!(tail < terminal);
    }
    std::fs::remove_dir_all(root).expect("cleanup");
}

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let root = std::env::temp_dir().join(format!(
            "morid-contract-{:?}-{}",
            std::thread::current().id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).expect("fixture directory");
        Self { root }
    }

    fn request(&self, mode: &str, job: &str) -> JobRequest {
        let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/queue_backend.py");
        JobRequest {
            spawn: SpawnSpec {
                program: PathBuf::from(if cfg!(windows) { "python" } else { "python3" }),
                args: vec![
                    script.into_os_string(),
                    mode.into(),
                    self.root.as_os_str().into(),
                    job.into(),
                ],
                cwd: self.root.join(job),
                env: vec![],
            },
            rules: Rules {
                split_lines_by: vec!["\n".into()],
                remove_text: vec![],
                skip_line_with_text: vec![],
                control: morid_ext::control::Control::parse(&serde_json::json!({}))
                    .expect("control"),
            },
        }
    }

    fn release(&self, job: &str) {
        std::fs::write(self.root.join(format!("release-{job}")), b"").expect("release gate");
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::write(self.root.join("stop"), b"");
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn job_of(event: &Event) -> u64 {
    match event {
        Event::JobQueued { job }
        | Event::JobStarted { job, .. }
        | Event::JobOutput { job, .. }
        | Event::JobProgress { job, .. }
        | Event::JobExited { job, .. }
        | Event::Notice { job, .. }
        | Event::JobError { job, .. }
        | Event::JobFinished { job, .. } => *job,
    }
}

fn pump_with_watchdog(queue: &mut Queue, deadline: Instant) -> Vec<Event> {
    assert!(Instant::now() < deadline, "queue made no timely completion");
    let status = queue.pump();
    if let morid::queue::PumpStatus::Waiting { retry_after } = status {
        assert!(!retry_after.is_zero());
        std::thread::sleep(retry_after);
    } else {
        // Yield to pipe readers even when the other job has a constant backlog.
        std::thread::sleep(Duration::from_millis(1));
    }
    queue.drain_events()
}

#[test]
fn queue_limit_two_completes_all_five_jobs_without_stalling() {
    let fixture = Fixture::new();
    let mut queue = Queue::new(ConcurrencyLimit::new(2).expect("limit"));
    let names = ["a", "b", "c", "d", "e"];
    let ids = names
        .iter()
        .map(|name| {
            queue
                .enqueue(fixture.request("gate", name))
                .expect("enqueue")
        })
        .collect::<Vec<_>>();
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut live = std::collections::BTreeSet::new();
    let mut started = Vec::new();
    let mut max_live = 0;
    while !queue.is_idle() {
        for event in pump_with_watchdog(&mut queue, deadline) {
            match event {
                Event::JobStarted { job, .. } => {
                    assert!(live.insert(job));
                    started.push(job);
                }
                Event::JobFinished { job, state } => {
                    assert_eq!(state, JobState::Succeeded);
                    assert!(live.remove(&job));
                }
                _ => {}
            }
            max_live = max_live.max(live.len());
            assert!(live.len() <= 2);
        }
        assert!(queue.running_count() <= 2);
        // Do not release the first wave until BOTH slots were actually occupied.
        if started.len() >= 2 {
            for name in names {
                fixture.release(name);
            }
        }
    }
    assert_eq!(max_live, 2);
    assert_eq!(started, ids);
    assert!(live.is_empty());
    for id in ids {
        assert_eq!(queue.state(id), Some(JobState::Succeeded));
    }
}

#[test]
fn queue_fifo_three_jobs_refills_freed_slot_automatically() {
    let fixture = Fixture::new();
    let mut queue = Queue::new(ConcurrencyLimit::new(1).expect("limit"));
    let names = ["first", "second", "third"];
    let ids = names
        .iter()
        .map(|name| {
            queue
                .enqueue(fixture.request("gate", name))
                .expect("enqueue")
        })
        .collect::<Vec<_>>();
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut starts = Vec::new();
    let mut finishes = Vec::new();
    while !queue.is_idle() {
        for event in pump_with_watchdog(&mut queue, deadline) {
            match event {
                Event::JobStarted { job, .. } => {
                    assert_eq!(finishes.len(), starts.len(), "slot reused before terminal");
                    let index = starts.len();
                    assert_eq!(job, ids[index]);
                    starts.push(job);
                    fixture.release(names[index]);
                }
                Event::JobFinished { job, state } => {
                    assert_eq!(state, JobState::Succeeded);
                    finishes.push(job);
                }
                _ => {}
            }
        }
    }
    assert_eq!(starts, ids);
    assert_eq!(finishes, ids);
}

#[test]
fn queue_terminal_is_last_after_both_pipe_eof_fragments() {
    let fixture = Fixture::new();
    let mut queue = Queue::new(ConcurrencyLimit::new(1).expect("limit"));
    let id = queue
        .enqueue(fixture.request("plain", "tail"))
        .expect("enqueue");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut events = Vec::new();
    while !queue.is_idle() {
        events.extend(pump_with_watchdog(&mut queue, deadline));
    }
    queue.pump();
    events.extend(queue.drain_events());
    let own = events
        .iter()
        .filter(|event| job_of(event) == id)
        .collect::<Vec<_>>();
    assert!(matches!(
        own.last(),
        Some(Event::JobFinished {
            state: JobState::Succeeded,
            ..
        })
    ));
    assert_eq!(
        own.iter()
            .filter(|event| matches!(event, Event::JobFinished { .. }))
            .count(),
        1
    );
    for (expected_stream, expected_line) in [
        (morid_engine::stream::Stream::Stdout, "stdout-tail-tail"),
        (morid_engine::stream::Stream::Stderr, "tail-tail"),
    ] {
        assert!(own[..own.len()-1].iter().any(|event| matches!(event,
            Event::JobOutput { stream, line, .. } if *stream == expected_stream && line == expected_line)));
    }
}

#[test]
fn queue_cancel_before_admission_has_no_start_pid_or_destination() {
    let fixture = Fixture::new();
    let mut queue = Queue::new(ConcurrencyLimit::new(1).expect("limit"));
    let id = queue
        .enqueue(fixture.request("plain", "never-started"))
        .expect("enqueue");
    queue.cancel(id).expect("queued cancellation");
    queue.cancel(id).expect("idempotent cancellation");
    queue.pump();
    assert!(queue.is_idle());
    assert_eq!(queue.state(id), Some(JobState::Cancelled));
    let events = queue.drain_events();
    assert!(matches!(
        events.as_slice(),
        [
            Event::JobQueued { .. },
            Event::JobFinished {
                state: JobState::Cancelled,
                ..
            }
        ]
    ));
    assert!(!fixture.root.join("never-started").exists());
    assert!(!fixture.root.join("pid-never-started").exists());
}

/// Each pump serves at most 256 events per job. Quiet output must be served
/// while the flood remains alive, not after it exits or releases its pipe.
#[test]
fn queue_flood_does_not_starve_quiet_job_and_idle_poll_reports_waiting() {
    let fixture = Fixture::new();
    let mut queue = Queue::new(ConcurrencyLimit::new(2).expect("limit"));
    let flood = queue
        .enqueue(fixture.request("flood", "flood"))
        .expect("enqueue");
    let quiet = queue
        .enqueue(fixture.request("gate", "quiet"))
        .expect("enqueue");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut quiet_seen = false;
    let mut flood_seen = false;
    while !(quiet_seen && flood_seen) {
        let batch = pump_with_watchdog(&mut queue, deadline);
        let flood_events = batch
            .iter()
            .filter(|event| matches!(event, Event::JobOutput { job, .. } if *job == flood))
            .count();
        assert!(flood_events <= 256, "flood exceeds per-turn budget");
        flood_seen |= flood_events > 0;
        quiet_seen |= batch.iter().any(|event| matches!(event, Event::JobOutput { job, line, .. } if *job == quiet && line == "out-quiet"));
        assert_eq!(queue.state(flood), Some(JobState::Running));
        assert_eq!(queue.state(quiet), Some(JobState::Running));
    }
    fixture.release("flood");
    // Drain flood to completion; quiet is held at its gate, with empty pipes.
    while queue.state(flood) == Some(JobState::Running) {
        let _ = pump_with_watchdog(&mut queue, deadline);
    }
    let mut waiting_seen = false;
    for _ in 0..100 {
        if let morid::queue::PumpStatus::Waiting { retry_after } = queue.pump() {
            assert!(!retry_after.is_zero());
            waiting_seen = true;
            break;
        }
        let _ = queue.drain_events();
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(waiting_seen, "no-progress pump must report Waiting");
    fixture.release("quiet");
    while !queue.is_idle() {
        let _ = pump_with_watchdog(&mut queue, deadline);
    }
    assert_eq!(queue.state(flood), Some(JobState::Succeeded));
    assert_eq!(queue.state(quiet), Some(JobState::Succeeded));
}
