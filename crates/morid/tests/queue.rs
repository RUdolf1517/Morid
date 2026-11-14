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

#[test]
fn lifecycle_admission_failures_are_typed_and_do_not_block_next_job() {
    use morid::jobs::JobFailure;
    let fixture = Fixture::new();
    let blocked = fixture.root.join("not-a-directory");
    std::fs::write(&blocked, b"file").expect("blocked destination");
    let mut queue = Queue::new(ConcurrencyLimit::new(1).expect("limit"));
    let mut destination = fixture.request("plain", "destination");
    destination.spawn.cwd = blocked.join("nested");
    let first = queue.enqueue(destination).expect("enqueue");
    let mut missing = fixture.request("plain", "missing");
    missing.spawn.program = fixture.root.join("no-such-executable");
    let second = queue.enqueue(missing).expect("enqueue");
    let third = queue
        .enqueue(fixture.request("plain", "success"))
        .expect("enqueue");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut events = Vec::new();
    while !queue.is_idle() {
        events.extend(pump_with_watchdog(&mut queue, deadline));
    }
    assert_eq!(queue.state(first), Some(JobState::Failed));
    assert_eq!(queue.state(second), Some(JobState::Failed));
    assert_eq!(queue.state(third), Some(JobState::Succeeded));
    let failure = |id| {
        events
            .iter()
            .find_map(|event| match event {
                Event::JobError { job, failure } if *job == id => Some(failure),
                _ => None,
            })
            .expect("typed failure")
    };
    assert!(matches!(failure(first), JobFailure::Destination { .. }));
    assert_eq!(failure(first).exit_code(), 6);
    assert!(matches!(failure(second), JobFailure::FailedToStart { .. }));
    assert_eq!(failure(second).exit_code(), 3);
    for id in [first, second] {
        assert!(!events.iter().any(|event| matches!(event, Event::JobStarted { job, .. } | Event::JobExited { job, .. } if *job == id)));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, Event::JobFinished { job, .. } if *job == id))
                .count(),
            1
        );
    }
    assert!(events.iter().any(|event| matches!(event, Event::Notice { job, key: "job.destination_failed", args } if *job == first && args.contains_key("path") && args.contains_key("detail"))));
    assert!(events.iter().any(|event| matches!(event, Event::Notice { job, key: "job.failed_to_start", args } if *job == second && args.contains_key("program"))));
    assert!(!fixture.root.join("pid-destination").exists());
    assert!(!fixture.root.join("pid-missing").exists());
}

#[test]
fn lifecycle_nonzero_exit_preserves_tails_cwd_progress_and_typed_status() {
    use morid::jobs::JobFailure;
    let fixture = Fixture::new();
    let mut queue = Queue::new(ConcurrencyLimit::default());
    let mut request = fixture.request("fail", "failed");
    request.rules.control = morid_ext::control::Control::parse(&serde_json::json!({
        "lhs": {"contains": "downloaded_bytes:"}
    }))
    .expect("progress control");
    let id = queue.enqueue(request).expect("enqueue");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut events = Vec::new();
    while !queue.is_idle() {
        events.extend(pump_with_watchdog(&mut queue, deadline));
    }
    assert_eq!(queue.state(id), Some(JobState::Failed));
    let actual_cwd = std::fs::read_to_string(fixture.root.join("cwd-failed")).expect("cwd");
    assert_eq!(
        std::fs::canonicalize(actual_cwd).expect("actual cwd"),
        std::fs::canonicalize(fixture.root.join("failed")).expect("expected cwd")
    );
    assert!(events.iter().any(|event| matches!(event, Event::JobProgress { job, progress } if *job == id && progress.percent == Some(50.0))));
    for tail in ["stdout-tail-failed", "tail-failed"] {
        assert!(events
            .iter()
            .any(|event| matches!(event, Event::JobOutput { line, .. } if line == tail)));
    }
    assert!(events.iter().any(|event| matches!(
        event,
        Event::JobError {
            failure: JobFailure::BackendExit {
                code: Some(7),
                signal: None
            },
            ..
        }
    )));
    assert!(events.iter().any(|event| matches!(event, Event::Notice { key: "job.backend_exit", args, .. } if args.get("code").map(String::as_str) == Some("7"))));
    assert!(events.iter().any(|event| matches!(event, Event::JobExited { exit, .. } if exit.code == Some(7) && !exit.cancelled && exit.duration > Duration::ZERO)));
    assert!(matches!(
        events.last(),
        Some(Event::JobFinished {
            state: JobState::Failed,
            ..
        })
    ));
    assert!(
        queue.cancel(id).is_err(),
        "cannot rewrite Failed to Cancelled"
    );
    assert!(queue.drain_events().is_empty());
}

fn assert_pid_dead(pid: u32, fixture: &Fixture) {
    #[cfg(unix)]
    let (program, args) = (
        PathBuf::from("ps"),
        vec![
            "-o".into(),
            "stat=".into(),
            "-p".into(),
            pid.to_string().into(),
        ],
    );
    #[cfg(windows)]
    let (program, args) = (
        PathBuf::from(std::env::var_os("SystemRoot").expect("SystemRoot")).join("System32/WindowsPowerShell/v1.0/powershell.exe"),
        vec!["-NoProfile".into(), "-NonInteractive".into(), "-Command".into(), format!("if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ 'alive' }} else {{ 'gone' }}").into()],
    );
    let result = SpawnSpec {
        program,
        args,
        cwd: fixture.root.clone(),
        env: vec![],
    }
    .spawn()
    .expect("process-table query")
    .wait_with_output()
    .expect("process table");
    let state = String::from_utf8_lossy(&result.stdout);
    println!(
        "queue post-terminal table pid={pid}, state={:?}",
        state.trim()
    );
    #[cfg(unix)]
    {
        assert!(result.status.success() || result.status.code() == Some(1));
        assert!(!state.trim().chars().next().is_some_and(|ch| ch != 'Z'));
    }
    #[cfg(windows)]
    {
        assert!(result.status.success());
        assert_eq!(state.trim(), "gone");
    }
}

#[test]
fn lifecycle_cancel_all_observes_running_trees_and_never_starts_queued_job() {
    let fixture = Fixture::new();
    let mut queue = Queue::new(ConcurrencyLimit::new(2).expect("limit"));
    let first = queue
        .enqueue(fixture.request("tree-gate", "one"))
        .expect("enqueue");
    let second = queue
        .enqueue(fixture.request("tree-gate", "two"))
        .expect("enqueue");
    let queued = queue
        .enqueue(fixture.request("plain", "queued"))
        .expect("enqueue");
    let deadline = Instant::now() + Duration::from_secs(30);
    let names = ["one", "child-one", "two", "child-two"];
    let mut events = Vec::new();
    let pids = loop {
        events.extend(pump_with_watchdog(&mut queue, deadline));
        let pids = names
            .iter()
            .map(|name| {
                std::fs::read_to_string(fixture.root.join(format!("pid-{name}")))
                    .ok()?
                    .parse::<u32>()
                    .ok()
            })
            .collect::<Option<Vec<_>>>();
        if let Some(pids) = pids {
            break pids;
        }
    };
    queue.cancel(first).expect("per-job cancel");
    assert_eq!(
        queue.state(first),
        Some(JobState::Running),
        "signal is not completion"
    );
    assert!(queue.cancel_all().is_empty());
    while !queue.is_idle() {
        events.extend(pump_with_watchdog(&mut queue, deadline));
    }
    for pid in pids {
        assert_pid_dead(pid, &fixture);
    }
    for id in [first, second, queued] {
        assert_eq!(queue.state(id), Some(JobState::Cancelled));
        let own = events
            .iter()
            .filter(|event| job_of(event) == id)
            .collect::<Vec<_>>();
        assert_eq!(
            own.iter()
                .filter(|event| matches!(event, Event::JobFinished { .. }))
                .count(),
            1
        );
        assert!(matches!(
            own.last(),
            Some(Event::JobFinished {
                state: JobState::Cancelled,
                ..
            })
        ));
        queue.cancel(id).expect("idempotent");
    }
    assert!(!events
        .iter()
        .any(|event| matches!(event, Event::JobStarted { job, .. } if *job == queued)));
    assert!(!fixture.root.join("queued").exists());
}

#[test]
fn transport_command_reply_event_stream_and_clean_shutdown() {
    use morid::service::{Command, Service, ServiceEvent};
    let fixture = Fixture::new();
    let mut service = Service::start(ConcurrencyLimit::new(1).expect("limit")).expect("worker");
    let id = service
        .client
        .enqueue(fixture.request("plain", "transport"))
        .expect("command send")
        .recv_timeout(Duration::from_secs(5))
        .expect("reply")
        .expect("accepted");
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut events = Vec::new();
    loop {
        assert!(Instant::now() < deadline);
        if let ServiceEvent::Job(event) = service
            .events
            .recv_timeout(Duration::from_secs(5))
            .expect("event")
        {
            let done = matches!(event, Event::JobFinished { job, state: JobState::Succeeded } if job == id);
            events.push(event);
            if done {
                break;
            }
        }
    }
    assert!(matches!(events.first(), Some(Event::JobQueued { job }) if *job == id));
    assert!(events
        .iter()
        .any(|event| matches!(event, Event::JobOutput { line, .. } if line == "tail-transport")));
    service
        .client
        .send(Command::Cancel(u64::MAX))
        .expect("invalid command sent");
    assert!(matches!(
        service
            .events
            .recv_timeout(Duration::from_secs(5))
            .expect("rejection"),
        ServiceEvent::CommandRejected {
            job: Some(u64::MAX),
            ..
        }
    ));
    service.shutdown().expect("shutdown command");
    assert!(matches!(
        service
            .events
            .recv_timeout(Duration::from_secs(5))
            .expect("shutdown"),
        ServiceEvent::Stopped { clean: true }
    ));
    service.join().expect("worker join");
    assert!(service.events.try_recv().is_err());
}
