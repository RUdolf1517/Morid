//! Real service event -> independent process-table assertion, not only a mock
//! of the observation gate. All Rust process creation goes through morid-engine.
#![forbid(unsafe_code)]

use morid::jobs::{Event, JobState, RunningJob};
use morid_engine::spawn::SpawnSpec;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Files(PathBuf);
impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::write(self.0.join("stop"), b"stop");
    }
}

fn table_alive(pid: u32, files: &Files) -> io::Result<bool> {
    #[cfg(unix)]
    let (program, args) = (PathBuf::from("ps"), vec!["-o".into(), "stat=".into(), "-p".into(), pid.to_string().into()]);
    #[cfg(windows)]
    let (program, args) = {
        let root = std::env::var_os("SystemRoot").ok_or_else(|| io::Error::other("SystemRoot"))?;
        let script = format!("if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ 'alive' }} else {{ 'gone' }}");
        (PathBuf::from(root).join("System32/WindowsPowerShell/v1.0/powershell.exe"),
         vec!["-NoProfile".into(), "-NonInteractive".into(), "-Command".into(), script.into()])
    };
    let spec = SpawnSpec { program, args, cwd: files.0.clone(), env: vec![] };
    let result = spec.spawn().map_err(io::Error::other)?.wait_with_output()?;
    #[cfg(unix)]
    if !result.status.success() && result.status.code() != Some(1) {
        return Err(io::Error::other("ps process-table query failed"));
    }
    #[cfg(windows)]
    if !result.status.success() {
        return Err(io::Error::other("Windows process-table query failed"));
    }
    let state = String::from_utf8_lossy(&result.stdout);
    println!("service event process table: pid={pid}, state={:?}", state.trim());
    #[cfg(unix)]
    return Ok(state.trim().chars().next().is_some_and(|ch| ch != 'Z'));
    #[cfg(windows)]
    match state.trim() {
        "alive" => Ok(true),
        "gone" => Ok(false),
        _ => Err(io::Error::other("invalid process-table response")),
    }
}

#[test]
fn service_cancelled_event_is_followed_by_empty_process_table() -> io::Result<()> {
    let nonce = SystemTime::now().duration_since(UNIX_EPOCH).map_err(io::Error::other)?.as_nanos();
    let files = Files(std::env::temp_dir().join(format!("morid-service-{}-{nonce}", std::env::consts::OS)));
    fs::create_dir(&files.0)?;
    let script = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tools/fake_backend.py");
    let spec = SpawnSpec {
        program: if cfg!(windows) { "python".into() } else { "python3".into() },
        args: vec![script.into_os_string(), "backend".into(), files.0.clone().into_os_string()],
        cwd: files.0.clone(),
        env: vec![],
    };
    let mut job = RunningJob::start(9, &spec).map_err(io::Error::other)?;
    let events = job.drain_events();
    let [Event::JobStarted { job: 9, pid }] = events.as_slice() else { panic!("start event") };
    let leader = *pid;
    let deadline = Instant::now() + Duration::from_secs(15);
    let descendant = loop {
        if let Ok(text) = fs::read_to_string(files.0.join("grandchild.pid")) {
            break text.parse::<u32>().map_err(io::Error::other)?;
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "fake backend readiness"));
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    assert!(table_alive(leader, &files)?);
    assert!(table_alive(descendant, &files)?);
    job.cancel(Duration::from_secs(15)).map_err(|error| io::Error::other(format!("{error:?}")))?;
    assert_eq!(job.state(), JobState::Cancelled);
    assert_eq!(job.drain_events(), [Event::JobFinished { job: 9, state: JobState::Cancelled }]);
    // No polling after the terminal event: both must already be dead.
    assert!(!table_alive(leader, &files)?);
    assert!(!table_alive(descendant, &files)?);
    job.cancel(Duration::ZERO).map_err(|error| io::Error::other(format!("{error:?}")))?;
    assert!(job.drain_events().is_empty(), "one terminal event only");
    println!("SERVICE_CANCELLED_OK: event published only after tree death");
    fs::remove_dir_all(&files.0)?;
    Ok(())
}
