//! Executable limitation, not a whole-tree cancellation acceptance test.
//! PowerShell is used only as a Windows process-table oracle, never as a backend
//! launcher or terminator. No taskkill, network, extra crate or unsafe is needed.
#![cfg(windows)]
#![forbid(unsafe_code)]

use morid_engine::job_object::Job;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Fixture {
    root: PathBuf,
    backend: Child,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Also runs on assertion failures. Both payloads have independent bounded
        // lifetimes in case this controller is itself terminated by the runner.
        let _ = fs::write(self.root.join("stop"), b"stop");
        let _ = self.backend.kill();
        let _ = self.backend.wait();
        let deadline = Instant::now() + Duration::from_secs(10);
        while !self.root.join("grandchild.done").exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn payload(name: &str, root: &Path) -> io::Result<Child> {
    Command::new(std::env::current_exe()?)
        .args(["--ignored", "--exact", name, "--nocapture"])
        .env("MORID_RACE_ROOT", root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
}

fn wait_file(path: &Path) -> io::Result<String> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match fs::read_to_string(path) {
            Ok(text) if !text.is_empty() => return Ok(text),
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                path.display().to_string(),
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn process_table_contains(pid: u32) -> io::Result<bool> {
    let windows =
        std::env::var_os("SystemRoot").ok_or_else(|| io::Error::other("SystemRoot is missing"))?;
    let oracle = PathBuf::from(windows).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    // Only a typed numeric PID is inserted, never paths/user text. The result is
    // deliberately invariant across Windows UI languages.
    let script = format!(
        "if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ 'alive' }} else {{ 'gone' }}"
    );
    let output = Command::new(oracle)
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            &script,
        ])
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(
            String::from_utf8_lossy(&output.stderr).into_owned(),
        ));
    }
    match String::from_utf8_lossy(&output.stdout).trim() {
        "alive" => Ok(true),
        "gone" => Ok(false),
        other => Err(io::Error::other(format!(
            "unexpected process-table response: {other:?}"
        ))),
    }
}

fn wait_absent(pid: u32) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while process_table_contains(pid)? {
        if Instant::now() >= deadline {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                format!("PID {pid} survived cleanup"),
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

#[test]
fn assign_after_spawn_leaves_preassignment_grandchild_alive() -> io::Result<()> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(io::Error::other)?
        .as_nanos();
    let root = std::env::temp_dir().join(format!("morid-job-race-{}-{nonce}", std::process::id()));
    fs::create_dir(&root)?;
    let backend = payload("race_backend", &root)?;
    let mut fixture = Fixture { root, backend };
    let backend_pid = fixture.backend.id();
    // Deterministically widen the spawn->assign window: assignment is withheld
    // until the grandchild has executed and published its own PID. No race luck.
    let pid = wait_file(&fixture.root.join("grandchild.pid"))?
        .parse::<u32>()
        .map_err(io::Error::other)?;
    assert!(process_table_contains(backend_pid)?);
    assert!(process_table_contains(pid)?);
    let job = Job::new()?;
    job.assign(&fixture.backend)?;
    job.terminate(1)?;
    let status = fixture.backend.wait()?;
    assert!(!status.success());
    wait_absent(backend_pid)?;
    // This is the actual residual-property assertion: parent termination does
    // NOT kill a descendant that started outside this job before assignment.
    assert!(
        process_table_contains(pid)?,
        "preassignment grandchild unexpectedly died"
    );
    println!("residual race: backend {backend_pid} gone; preassignment grandchild {pid} alive");
    fs::write(fixture.root.join("stop"), b"stop")?;
    wait_absent(pid)?;
    println!("cleanup: grandchild {pid} absent from Windows process table");
    Ok(())
}

#[test]
#[ignore = "explicit backend payload for assignment race test"]
fn race_backend() -> io::Result<()> {
    let root = PathBuf::from(
        std::env::var_os("MORID_RACE_ROOT").ok_or_else(|| io::Error::other("missing root"))?,
    );
    let mut grandchild = payload("race_grandchild", &root)?;
    let deadline = Instant::now() + Duration::from_secs(45);
    while !root.join("stop").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = grandchild.kill();
    grandchild.wait()?;
    Ok(())
}

#[test]
#[ignore = "explicit grandchild payload for assignment race test"]
fn race_grandchild() -> io::Result<()> {
    let root = PathBuf::from(
        std::env::var_os("MORID_RACE_ROOT").ok_or_else(|| io::Error::other("missing root"))?,
    );
    fs::write(root.join("grandchild.pid.tmp"), std::process::id().to_string())?;
    fs::rename(root.join("grandchild.pid.tmp"), root.join("grandchild.pid"))?;
    let deadline = Instant::now() + Duration::from_secs(45);
    while !root.join("stop").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    fs::write(root.join("grandchild.done"), b"done")?;
    Ok(())
}
