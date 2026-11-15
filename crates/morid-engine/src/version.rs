//! Bounded, shell-free version probing through the existing contained child.
use crate::{
    managed::ManagedChild,
    output::Classified,
    spawn::SpawnSpec,
    stream::{OutputPoll, Rules, Stream},
};
use morid_core::Error;
use std::{
    ffi::OsString,
    path::Path,
    time::{Duration, Instant},
};

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
    let mut output = child
        .take_output(Rules {
            split_lines_by: vec!["\r\n".into(), "\n".into()],
            remove_text: vec![],
            skip_line_with_text: vec![],
            control: morid_ext::control::Control::disabled(),
        })
        .map_err(|e| Error::BackendFailure(e.to_string()))?;
    let start = Instant::now();
    let mut stdout = String::new();
    let mut stderr = String::new();
    let mut end = false;
    loop {
        if start.elapsed() > Duration::from_secs(5) {
            child
                .cancel_observed(Duration::from_secs(2))
                .map_err(|e| Error::BackendFailure(e.to_string()))?;
            return Ok(VersionRead {
                value: None,
                reason: "probe-timeout",
            });
        }
        match output.poll_next() {
            OutputPoll::Item(Ok(event)) => {
                if let Classified::Output(line) = event.data {
                    let text = if event.stream == Stream::Stdout {
                        &mut stdout
                    } else {
                        &mut stderr
                    };
                    if text.len() + line.len() > 65536 {
                        child
                            .cancel_observed(Duration::from_secs(2))
                            .map_err(|e| Error::BackendFailure(e.to_string()))?;
                        return Ok(VersionRead {
                            value: None,
                            reason: "probe-output-limit",
                        });
                    }
                    text.push_str(&line);
                    text.push('\n');
                }
            }
            OutputPoll::Item(Err(e)) => return Err(Error::BackendFailure(e.to_string())),
            OutputPoll::End => end = true,
            OutputPoll::Pending => {}
        }
        if let Some(status) = child
            .poll_finished()
            .map_err(|e| Error::BackendFailure(e.to_string()))?
        {
            if end {
                if status.code != Some(0) {
                    return Ok(VersionRead {
                        value: None,
                        reason: "probe-exit-failure",
                    });
                }
                // you-get writes its version to stderr. Do not merge pipe lines
                // nondeterministically; stdout is preferred when it has text.
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
