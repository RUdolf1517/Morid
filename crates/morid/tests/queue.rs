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
