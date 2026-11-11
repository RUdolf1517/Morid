//! Shell-free process creation. Containment/cancellation is a separate primitive.
use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};

/// Explicit argv, working directory and environment overrides. The inherited
/// environment is retained (PATH/SystemRoot are needed by backend executables).
#[derive(Debug, Clone)]
pub struct SpawnSpec {
    pub program: PathBuf,
    pub args: Vec<OsString>,
    pub cwd: PathBuf,
    pub env: Vec<(OsString, OsString)>,
}

#[derive(Debug)]
pub struct FailedToStart {
    pub program: PathBuf,
    pub source: io::Error,
}

impl fmt::Display for FailedToStart {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "failed to start {}: {}",
            self.program.display(),
            self.source
        )
    }
}
impl std::error::Error for FailedToStart {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

impl SpawnSpec {
    /// Does not create cwd: the service must do that before starting a job.
    /// stdin is EOF, stdout/stderr are independent pipes. No shell is involved.
    /// This low-level API alone makes no Windows tree-containment guarantee.
    pub fn spawn(&self) -> Result<Child, FailedToStart> {
        let mut command = self.command();
        #[cfg(unix)]
        crate::process_group::configure(&mut command);
        command.spawn().map_err(|source| FailedToStart {
            program: self.program.clone(),
            source,
        })
    }

    pub(crate) fn command(&self) -> Command {
        let mut command = Command::new(&self.program);
        command
            .args(&self.args)
            .current_dir(&self.cwd)
            .envs(self.env.iter().map(|(key, value)| (key, value)))
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    #[test]
    fn spawn_preserves_argv_cwd_env_closes_stdin_and_separates_pipes() -> io::Result<()> {
        let cwd = std::env::current_dir()?;
        let spec = SpawnSpec {
            program: std::env::current_exe()?,
            args: [
                "--ignored",
                "--exact",
                "spawn::tests::spawn_payload",
                "--nocapture",
            ]
            .into_iter()
            .map(OsString::from)
            .collect(),
            cwd: cwd.clone(),
            env: vec![
                (
                    "MORID_SPAWN_SENTINEL".into(),
                    "space Кириллица ; $ & |".into(),
                ),
                ("MORID_SPAWN_CWD".into(), cwd.into_os_string()),
            ],
        };
        let child = spec.spawn().map_err(io::Error::other)?;
        let output = child.wait_with_output()?;
        assert!(output.status.success(), "{:?}", output);
        assert!(String::from_utf8_lossy(&output.stdout).contains("stdout-only"));
        assert!(!String::from_utf8_lossy(&output.stdout).contains("stderr-only"));
        assert!(String::from_utf8_lossy(&output.stderr).contains("stderr-only"));
        assert!(!String::from_utf8_lossy(&output.stderr).contains("stdout-only"));
        Ok(())
    }

    #[test]
    fn missing_executable_is_failed_to_start() -> io::Result<()> {
        let spec = SpawnSpec {
            program: std::env::current_exe()?.join("not-an-executable"),
            args: vec![],
            cwd: std::env::current_dir()?,
            env: vec![],
        };
        let failure = spec
            .spawn()
            .expect_err("path below an executable cannot start");
        assert_eq!(failure.program, spec.program);
        Ok(())
    }

    #[test]
    #[ignore = "explicit child payload for spawn test"]
    fn spawn_payload() -> io::Result<()> {
        assert_eq!(
            std::env::var("MORID_SPAWN_SENTINEL").as_deref(),
            Ok("space Кириллица ; $ & |")
        );
        assert_eq!(
            std::env::current_dir()?.as_os_str(),
            std::env::var_os("MORID_SPAWN_CWD").expect("parent supplied cwd")
        );
        assert_eq!(
            std::env::args_os().skip(1).collect::<Vec<_>>(),
            [
                "--ignored",
                "--exact",
                "spawn::tests::spawn_payload",
                "--nocapture"
            ]
            .map(OsString::from)
        );
        let mut input = Vec::new();
        std::io::stdin().read_to_end(&mut input)?;
        assert!(input.is_empty(), "stdin must be EOF");
        println!("stdout-only");
        eprintln!("stderr-only");
        Ok(())
    }
}
