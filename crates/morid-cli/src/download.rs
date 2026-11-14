//! CLI adapter for planning and the service event stream. Never spawns directly.
use morid::jobs::{Event, JobFailure, JobState};
use morid::limit::ConcurrencyLimit;
use morid::planner::{self, Kind, Request, Settings};
use morid::queue::JobRequest;
use morid::service::{Command, Service, ServiceEvent};
use morid_core::{DownloadOptions, EngineId, Error, Url};
use morid_engine::spawn::SpawnSpec;
use morid_engine::stream::{Rules, Stream};
use morid_ext::{read_path, Executables, Target};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Duration;

fn usage(message: impl Into<String>) -> Error {
    Error::Usage(message.into())
}
fn write(out: &mut dyn Write, value: &Value) -> Result<(), Error> {
    writeln!(out, "{value}").map_err(|_| Error::Internal("cannot write output".into()))
}

pub fn run(
    args: &[String],
    directory: &Path,
    target: &Target<'_>,
    search: &dyn Executables,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Error> {
    let list = args[0] == "list";
    let value = |flag: &str| -> Result<Option<String>, Error> {
        let Some(index) = args.iter().position(|arg| arg == flag) else {
            return Ok(None);
        };
        args.get(index + 1)
            .cloned()
            .map(Some)
            .ok_or_else(|| usage(format!("missing value for {flag}")))
    };
    let config = value("--config")?
        .map(PathBuf::from)
        .unwrap_or_else(super::data_directory);
    let mut settings = if let Some(path) = value("--settings")? {
        serde_json::from_str::<Settings>(
            &std::fs::read_to_string(path).map_err(|e| usage(e.to_string()))?,
        )
        .map_err(|e| usage(format!("settings JSON: {e}")))?
    } else {
        Settings::default()
    };
    if settings.data_directory == Path::new(".") {
        settings.data_directory = config.clone();
    }
    if settings.destination == Path::new("Downloads") {
        settings.destination = std::env::var_os("HOME")
            .or_else(|| std::env::var_os("USERPROFILE"))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Downloads");
    }
    if let Some(engine) = value("--engine")? {
        settings.engine = engine;
    }
    if let Some(to) = value("--to")? {
        settings.destination = to.into();
    }
    if args.iter().any(|v| v == "--no-archive") && value("--archive")?.is_some() {
        return Err(usage("--archive conflicts with --no-archive"));
    }
    if let Some(archive) = value("--archive")? {
        settings.archive = Some(archive);
        settings.internal_archive = false;
    }
    if args.iter().any(|v| v == "--no-archive") {
        settings.archive = None;
        settings.internal_archive = false;
    }
    EngineId::parse(&settings.engine)?;
    let mut options = Vec::new();
    let mut urls = Vec::new();
    let mut batch = false;
    let mut json_output = false;
    let mut dry = false;
    let mut verbose = false;
    let mut quiet = false;
    let mut playlist = false;
    let mut index = 1;
    while index < args.len() {
        let arg = &args[index];
        match arg.as_str() {
            "--settings" | "--config" | "--engine" | "--to" | "--archive" => {
                index += 1;
                if index >= args.len() {
                    return Err(usage("missing option value"));
                }
            }
            "--options" | "-f" | "--format" | "--kind" | "--batch" => {
                let next = args
                    .get(index + 1)
                    .ok_or_else(|| usage(format!("missing value for {arg}")))?;
                match arg.as_str() {
                    "--options" => options.extend(DownloadOptions::parse(next).0),
                    "-f" | "--format" => options.extend(["-f".into(), next.clone()]),
                    "--kind" => match next.as_str() {
                        "audio" => options.extend(DownloadOptions::parse("-x -f bestaudio").0),
                        "video" | "best" => {}
                        _ => return Err(usage("kind must be audio, video or best")),
                    },
                    _ => {
                        batch = true;
                        urls.extend(
                            std::fs::read_to_string(next)
                                .map_err(|e| usage(e.to_string()))?
                                .lines()
                                .filter(|v| !v.trim().is_empty() && !v.starts_with('#'))
                                .map(str::to_owned),
                        );
                    }
                }
                index += 1;
            }
            "--json" => json_output = true,
            "--dry-run" => dry = true,
            "--verbose" => verbose = true,
            "--quiet" => quiet = true,
            "--playlist" => playlist = true,
            "--no-archive" | "--from-file" | "--wait" | "--no-color" => {}
            "--lang" => {
                index += 1;
                if args.get(index).map(String::as_str) != Some("en") {
                    return Err(usage("only --lang en is supported in Phase 3"));
                }
            }
            other if other.starts_with('-') => {
                return Err(usage(format!("unknown option {other}")))
            }
            _ => urls.push(arg.clone()),
        }
        index += 1;
    }
    if urls.is_empty() || (!batch && urls.len() != 1) || (list && batch) {
        return Err(usage("expected one URL, or download --batch FILE"));
    }
    let extension = read_path(
        &directory.join(format!("{}.json", settings.engine)),
        target,
        search,
    )?;
    for warning in &extension.warnings {
        writeln!(
            err,
            "{}: warning: {}: {}",
            warning.file, warning.key, warning.message
        )
        .map_err(|_| usage("cannot write diagnostics"))?;
    }
    let mut plans = Vec::new();
    for url in urls {
        plans.push(planner::plan(
            &extension,
            &settings,
            &Request {
                url: Url::parse(&url)?,
                options: DownloadOptions(options.clone()),
                ui_options: DownloadOptions(vec![]),
                kind: if list {
                    Kind::List
                } else if batch {
                    Kind::Batch
                } else {
                    Kind::Download
                },
                playlist,
            },
        )?);
    }
    // Exact same planned vectors below feed SpawnSpec. Dry-run returns before
    // executable availability, mkdir, handlers, worker creation, or spawning.
    if dry {
        for argv in plans {
            write(out, &json!(argv))?;
        }
        return Ok(0);
    }
    if extension.executable.is_none() {
        return Err(Error::Extension {
            file: extension.file.clone(),
            rule: 6,
            message: "backend unavailable".into(),
        });
    }
    let program = search
        .find(&extension.command.args[0])
        .ok_or_else(|| Error::Extension {
            file: extension.file.clone(),
            rule: 6,
            message: "command launcher unavailable".into(),
        })?;
    morid_engine::install_interrupt_handler().map_err(|e| Error::Internal(e.to_string()))?;
    let limit = ConcurrencyLimit::default().updated_from_configure_text(&settings.concurrency);
    let mut service = Service::start(limit).map_err(|e| Error::Internal(e.to_string()))?;
    let mut vectors = BTreeMap::new();
    for mut argv in plans {
        // Resolution changes only argv[0], which is what std::process executes.
        argv[0] = program.to_string_lossy().into_owned();
        let job = service
            .client
            .enqueue(JobRequest {
                spawn: SpawnSpec {
                    program: program.clone(),
                    args: argv[1..].iter().map(Into::into).collect(),
                    cwd: settings.destination.clone(),
                    env: vec![],
                },
                rules: Rules::from_extension(&extension),
            })
            .map_err(|_| Error::Internal("service disconnected".into()))?
            .recv()
            .map_err(|_| Error::Internal("enqueue reply lost".into()))?
            .map_err(|e| Error::Internal(format!("enqueue rejected: {e:?}")))?;
        vectors.insert(job, argv);
    }
    let mut remaining = vectors.len();
    let mut status = 0;
    let mut cancelled = false;
    let mut durations = BTreeMap::new();
    let mut files: BTreeMap<u64, Vec<PathBuf>> = BTreeMap::new();
    while remaining > 0 {
        if morid_engine::interrupted() && !cancelled {
            let _ = service.client.send(Command::CancelAll);
            cancelled = true;
        }
        let event = match service.events.recv_timeout(Duration::from_millis(25)) {
            Ok(ServiceEvent::Job(event)) => event,
            Ok(ServiceEvent::CommandRejected { reason, .. }) => {
                return Err(Error::Internal(format!(
                    "service rejected command: {reason:?}"
                )))
            }
            Ok(ServiceEvent::Stopped { .. })
            | Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err(Error::Internal(
                    "service stopped before jobs finished".into(),
                ))
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
        };
        let record = match event {
            Event::JobQueued { .. } | Event::Notice { .. } => continue,
            Event::JobStarted { job, pid } => {
                json!({"type":"started","v":1,"job":job,"pid":pid,"engine":settings.engine,"engine_version":null,"argv":vectors[&job]})
            }
            Event::JobProgress { job, progress } => {
                if let Some(name) = &progress.filename {
                    let path = PathBuf::from(name);
                    let path = if path.is_absolute() {
                        path
                    } else {
                        settings.destination.join(path)
                    };
                    let entries = files.entry(job).or_default();
                    if !entries.contains(&path) {
                        entries.push(path);
                    }
                }
                json!({"type":"progress","v":1,"job":job,"percent":progress.percent,"downloaded":progress.downloaded_bytes,"total":progress.total_bytes,"speed":progress.speed,"eta":progress.eta})
            }
            Event::JobOutput { job, stream, line } => {
                if list && stream == Stream::Stdout {
                    let meta = parse_metadata(&line)?;
                    json!({"type":"metadata","v":1,"job":job,"title":meta["title"],"duration":meta["duration"],"thumbnail":meta["thumbnail"],"formats":meta["formats"]})
                } else {
                    if let Some(name) = line.strip_prefix("[download] Destination: ") {
                        let path = PathBuf::from(name);
                        files.entry(job).or_default().push(if path.is_absolute() {
                            path
                        } else {
                            settings.destination.join(path)
                        });
                    }
                    json!({"type":"output","v":1,"job":job,"stream":if stream==Stream::Stdout {"stdout"} else {"stderr"},"line":line})
                }
            }
            Event::JobExited { job, exit } => {
                durations.insert(job, exit.duration.as_millis());
                continue;
            }
            Event::JobError { job, failure } => {
                status = failure.exit_code();
                let backend_exit = match failure {
                    JobFailure::BackendExit { code, .. } => code,
                    _ => None,
                };
                json!({"type":"error","v":1,"job":job,"code":status,"message":format!("{failure:?}"),"backend_exit":backend_exit})
            }
            Event::JobFinished { job, state } => {
                remaining -= 1;
                if state == JobState::Cancelled {
                    status = 5;
                } else if state == JobState::Failed && status == 0 {
                    status = 3;
                }
                json!({"type":"finished","v":1,"job":job,"state":match state { JobState::Succeeded=>"success", JobState::Cancelled=>"cancelled", _=>"failed" },"files":files.get(&job).cloned().unwrap_or_default().into_iter().filter(|p| p.is_file()).collect::<Vec<_>>(),"duration_ms":durations.get(&job).copied().unwrap_or(0)})
            }
        };
        if json_output {
            write(out, &record)?;
        } else if !quiet && (verbose || record["type"] != "output") {
            writeln!(
                out,
                "{} {}",
                record["type"].as_str().unwrap_or("event"),
                record
            )
            .map_err(|_| usage("cannot write output"))?;
        }
    }
    let _ = service.shutdown();
    service
        .join()
        .map_err(|_| Error::Internal("service worker panicked".into()))?;
    Ok(status)
}

pub fn parse_metadata(line: &str) -> Result<Value, Error> {
    let mut value: Value = serde_json::from_str(line)
        .map_err(|e| Error::BackendFailure(format!("invalid metadata: {e}")))?;
    let object = value
        .as_object_mut()
        .ok_or_else(|| Error::BackendFailure("metadata must be an object".into()))?;
    if !object.get("formats").is_some_and(Value::is_array) {
        return Err(Error::BackendFailure(
            "metadata formats must be an array".into(),
        ));
    }
    if let Some(formats) = object.get_mut("formats").and_then(Value::as_array_mut) {
        for format in formats {
            if let Some(fields) = format.as_object_mut() {
                if let Some(id) = fields.get("format_id").cloned() {
                    fields.insert("id".into(), id);
                }
            }
        }
    }
    Ok(value)
}
