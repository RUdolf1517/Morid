//! Bounded, shell-free version probing through the existing contained child.
use crate::{managed::ManagedChild, spawn::SpawnSpec};
use morid_core::Error;
use std::{
    ffi::OsString,
    io::Read,
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

fn capture(mut input: impl Read) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    input.by_ref().take(65537).read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn stop_probe(child: &mut ManagedChild) -> Result<(), Error> {
    if child
        .poll_finished()
        .map_err(|e| Error::BackendFailure(e.to_string()))?
        .is_some()
    {
        return Ok(());
    }
    match child.cancel_observed(Duration::from_secs(2)) {
        Ok(_) => Ok(()),
        // Darwin can reject killpg for an exited, unreaped leader. Only the
        // existing read-only tree-absence observation may turn that into success.
        Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                if child
                    .poll_finished()
                    .map_err(|e| Error::BackendFailure(e.to_string()))?
                    .is_some()
                {
                    return Ok(());
                }
                if Instant::now() >= deadline {
                    return Err(Error::BackendFailure(error.to_string()));
                }
                std::thread::sleep(Duration::from_millis(2));
            }
        }
        Err(error) => Err(Error::BackendFailure(error.to_string())),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRead {
    pub value: Option<String>,
    pub reason: &'static str,
}
impl VersionRead {
    pub fn display(&self) -> &str {
        self.value.as_deref().unwrap_or("unavailable")
    }
}

/// Preserve printed tokens, including punctuation. get-sauce alone declares
/// line zero despite its captured leading blank; explicitly skip leading blanks
/// for that backend rather than silently changing all extensions' line indices.
pub fn token(name: &str, output: &str, line: usize, position: usize) -> VersionRead {
    let output = if matches!(name, "get-sauce" | "getsauce") {
        output.trim_start_matches(['\r', '\n'])
    } else {
        output
    };
    let value = output
        .lines()
        .nth(line)
        .and_then(|line| line.split_whitespace().nth(position))
        .map(str::to_owned);
    VersionRead {
        reason: if value.is_some() {
            "printed-token"
        } else {
            "declared-token-absent"
        },
        value,
    }
}

/// `prefix` contains only interpreter/script arguments preceding VersionArgument.
/// The executable must already have been resolved by morid-ext.
pub fn probe(
    name: &str,
    executable: &Path,
    prefix: &[OsString],
    argument: &str,
    line: usize,
    position: usize,
) -> Result<VersionRead, Error> {
    if argument.is_empty() {
        return Ok(VersionRead {
            value: None,
            reason: "version-argument-absent",
        });
    }
    let mut args = prefix.to_vec();
    args.push(argument.into());
    let spec = SpawnSpec {
        program: executable.into(),
        args,
        cwd: std::env::current_dir().map_err(|e| Error::BackendFailure(e.to_string()))?,
        env: vec![],
    };
    let mut child = ManagedChild::spawn(&spec).map_err(|e| Error::BackendFailure(e.to_string()))?;
    let (stdout, stderr) = child
        .take_probe_pipes()
        .map_err(|e| Error::BackendFailure(e.to_string()))?;
    let (sender, receiver) = mpsc::channel();
    let out_sender = sender.clone();
    let out = std::thread::Builder::new()
        .name("version-stdout".into())
        .spawn(move || {
            let _ = out_sender.send((0usize, capture(stdout)));
        })
        .map_err(|e| Error::BackendFailure(e.to_string()))?;
    let err = std::thread::Builder::new()
        .name("version-stderr".into())
        .spawn(move || {
            let _ = sender.send((1usize, capture(stderr)));
        })
        .map_err(|e| Error::BackendFailure(e.to_string()))?;
    let start = Instant::now();
    let mut captured: [Option<Vec<u8>>; 2] = [None, None];
    loop {
        while let Ok((index, result)) = receiver.try_recv() {
            let bytes = result.map_err(|e| Error::BackendFailure(e.to_string()))?;
            if bytes.len() > 65536 {
                stop_probe(&mut child)?;
                let _ = out.join();
                let _ = err.join();
                return Ok(VersionRead {
                    value: None,
                    reason: "probe-output-limit",
                });
            }
            captured[index] = Some(bytes);
        }
        if start.elapsed() > Duration::from_secs(5) {
            stop_probe(&mut child)?;
            let _ = out.join();
            let _ = err.join();
            return Ok(VersionRead {
                value: None,
                reason: "probe-timeout",
            });
        }
        if let Some(status) = child
            .poll_finished()
            .map_err(|e| Error::BackendFailure(e.to_string()))?
        {
            if let [Some(stdout), Some(stderr)] = &captured {
                out.join()
                    .map_err(|_| Error::BackendFailure("version stdout reader failed".into()))?;
                err.join()
                    .map_err(|_| Error::BackendFailure("version stderr reader failed".into()))?;
                if status.code != Some(0) {
                    return Ok(VersionRead {
                        value: None,
                        reason: "probe-exit-failure",
                    });
                }
                let stdout = String::from_utf8_lossy(stdout);
                let stderr = String::from_utf8_lossy(stderr);
                return Ok(token(
                    name,
                    if stdout.trim().is_empty() {
                        &stderr
                    } else {
                        &stdout
                    },
                    line,
                    position,
                ));
            }
        }
        std::thread::sleep(Duration::from_millis(2));
    }
}
