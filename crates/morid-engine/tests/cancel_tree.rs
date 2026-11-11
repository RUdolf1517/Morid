//! §3.7a acceptance: use the managed spawn/cancel path, then inspect the OS
//! process table. A separate naive-kill control proves this fixture can escape
//! leader-only termination. Test oracles are not backend launchers/terminators.
#![forbid(unsafe_code)]

use morid_engine::managed::ManagedChild;
use morid_engine::spawn::SpawnSpec;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Files(PathBuf);
impl Files {
    fn new() -> io::Result<Self> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(io::Error::other)?
            .as_nanos();
        let path = std::env::temp_dir().join(format!("morid-cancel-{}-{nonce}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Self(path))
    }

    fn spec(&self) -> io::Result<SpawnSpec> {
        Ok(SpawnSpec {
            program: std::env::current_exe()?,
            args: ["--ignored", "--exact", "backend", "--nocapture"]
                .map(Into::into)
                .to_vec(),
            cwd: self.0.clone(),
            env: vec![("MORID_CANCEL_ROOT".into(), self.0.clone().into_os_string())],
        })
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        // Retain this stop marker on failures, so an escaped payload can exit
        // cooperatively. Independent deadlines also bound orphan lifetimes.
        let _ = fs::write(self.0.join("stop"), b"stop");
    }
}

struct Leader(Child);
impl Drop for Leader {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn grandchild_pid(root: &Path) -> io::Result<u32> {
    let deadline = Instant::now() + Duration::from_secs(15);
    loop {
        match fs::read_to_string(root.join("grandchild.pid")) {
            Ok(text) => return text.parse().map_err(io::Error::other),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        if Instant::now() >= deadline {
            return Err(io::Error::new(io::ErrorKind::TimedOut, "grandchild readiness"));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[cfg(unix)]
fn alive(pid: u32) -> io::Result<bool> {
    let output = Command::new("ps")
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(io::Error::other(String::from_utf8_lossy(&output.stderr).into_owned()));
    }
    let state = String::from_utf8_lossy(&output.stdout);
    // Orphan zombies can remain until init reaps them: they are dead, cannot
    // execute code and do not satisfy "live descendant". Do not conceal them.
    println!("process table: pid={pid}, stat={:?}", state.trim());
    Ok(state.trim().chars().next().is_some_and(|ch| ch != 'Z'))
}

#[cfg(windows)]
fn alive(pid: u32) -> io::Result<bool> {
    let root = std::env::var_os("SystemRoot").ok_or_else(|| io::Error::other("SystemRoot"))?;
    let oracle = PathBuf::from(root).join("System32/WindowsPowerShell/v1.0/powershell.exe");
    let script = format!(
        "if (Get-Process -Id {pid} -ErrorAction SilentlyContinue) {{ 'alive' }} else {{ 'gone' }}"
    );
    let output = Command::new(oracle)
        .args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", &script])
        .stdin(Stdio::null())
        .output()?;
    if !output.status.success() {
        return Err(io::Error::other(String::from_utf8_lossy(&output.stderr).into_owned()));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    println!("process table: pid={pid}, state={:?}", text.trim());
    match text.trim() {
        "alive" => Ok(true),
        "gone" => Ok(false),
        other => Err(io::Error::other(format!("unexpected table response: {other:?}"))),
    }
}

fn wait_dead(pid: u32) -> io::Result<()> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while alive(pid)? {
        if Instant::now() >= deadline {
            return Err(io::Error::new(io::ErrorKind::TimedOut, format!("live PID {pid}")));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

#[test]
fn cancel_path_leaves_no_live_descendants() -> io::Result<()> {
    let files = Files::new()?;
    let mut child = ManagedChild::spawn(&files.spec()?).map_err(io::Error::other)?;
    let leader = child.id();
    let descendant = grandchild_pid(&files.0)?;
    assert!(alive(leader)?);
    assert!(alive(descendant)?);
    let status = child.cancel()?;
    assert!(!status.success());
    wait_dead(leader)?;
    wait_dead(descendant)?;
    assert_eq!(child.cancel()?, status, "repeated cancel must not resignal reused PID");
    println!("CANCEL_TREE_OK: leader {leader} and grandchild {descendant}: no live descendants");
    fs::remove_dir_all(&files.0)?;
    Ok(())
}

#[test]
fn naive_leader_kill_control_leaves_grandchild_alive() -> io::Result<()> {
    let files = Files::new()?;
    let mut child = Leader(files.spec()?.spawn().map_err(io::Error::other)?);
    let descendant = grandchild_pid(&files.0)?;
    child.0.kill()?;
    child.0.wait()?;
    assert!(alive(descendant)?, "fixture must survive a leader-only kill");
    println!("NAIVE_KILL_CONTROL_OK: grandchild {descendant} survived leader-only kill");
    fs::write(files.0.join("stop"), b"stop")?;
    wait_dead(descendant)?;
    fs::remove_dir_all(&files.0)?;
    Ok(())
}

fn payload_root() -> io::Result<PathBuf> {
    std::env::var_os("MORID_CANCEL_ROOT")
        .map(PathBuf::from)
        .ok_or_else(|| io::Error::other("missing payload root"))
}

#[test]
#[ignore = "explicit fake backend; starts grandchild without a containment handshake"]
fn backend() -> io::Result<()> {
    let root = payload_root()?;
    let mut child = Leader(Command::new(std::env::current_exe()?)
        .args(["--ignored", "--exact", "grandchild", "--nocapture"])
        .env("MORID_CANCEL_ROOT", &root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?);
    let deadline = Instant::now() + Duration::from_secs(60);
    while !root.join("stop").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    child.0.kill()?;
    child.0.wait()?;
    Ok(())
}

#[test]
#[ignore = "explicit grandchild; no dependence on parent liveness or inherited pipe EOF"]
fn grandchild() -> io::Result<()> {
    let root = payload_root()?;
    fs::write(root.join("pid.tmp"), std::process::id().to_string())?;
    fs::rename(root.join("pid.tmp"), root.join("grandchild.pid"))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    while !root.join("stop").exists() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}
