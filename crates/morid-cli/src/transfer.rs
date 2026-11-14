//! Import/export commands do not require extensions or a backend executable.
use morid::{store::transfer, store::transfer::Report};
use morid_core::Error;
use std::{io::Write, path::PathBuf};

pub fn run(args: &[String], out: &mut dyn Write) -> Result<u8, Error> {
    let mut directory = None;
    let mut config = None;
    let mut report_path = None;
    let mut force = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--force" => force = true,
            flag @ ("--config" | "--report") => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or_else(|| Error::Usage(format!("missing value for {flag}")))?;
                if flag == "--config" {
                    config = Some(PathBuf::from(value));
                } else {
                    report_path = Some(PathBuf::from(value));
                }
            }
            arg if arg.starts_with('-') => {
                return Err(Error::Usage(format!("unknown option {arg}")))
            }
            arg => {
                if directory.replace(PathBuf::from(arg)).is_some() {
                    return Err(Error::Usage("expected one directory".into()));
                }
            }
        }
        index += 1;
    }
    let directory = directory.ok_or_else(|| Error::Usage("expected one directory".into()))?;
    let config = config.unwrap_or_else(super::data_directory);
    let (source, destination) = if args[0] == "import" {
        (&directory, &config)
    } else {
        (&config, &directory)
    };
    if !transfer::recognized(source) {
        return Err(Error::ImportSource(format!(
            "{}: data directory not recognized",
            source.display()
        )));
    }
    // Reporting must not mutate the input tree (including aliases through symlinks).
    if let Some(path) = &report_path {
        let absolute = if path.is_absolute() {
            path.clone()
        } else {
            std::env::current_dir()
                .map_err(|e| Error::Destination(e.to_string()))?
                .join(path)
        };
        let parent = absolute
            .parent()
            .ok_or_else(|| Error::Usage("invalid report path".into()))?;
        let parent = parent
            .canonicalize()
            .map_err(|e| Error::Destination(e.to_string()))?;
        let source = source
            .canonicalize()
            .map_err(|e| Error::ImportSource(e.to_string()))?;
        let dest = destination.canonicalize().ok();
        if parent.starts_with(&source)
            || dest.as_ref().is_some_and(|dest| parent.starts_with(dest))
            || absolute.exists()
        {
            return Err(Error::Usage(
                "report must be a new file outside source and destination".into(),
            ));
        }
    }
    let report = if args[0] == "import" {
        transfer::import(source, destination, force)
    } else {
        transfer::export(source, destination, force)
    };
    let report = match report {
        Ok(report) => report,
        Err(error) => Report {
            warnings: vec![error.to_string()],
            ..Report::default()
        },
    };
    let write_error = |e: std::io::Error| Error::Destination(e.to_string());
    for file in &report.imported {
        writeln!(out, "imported: {file}").map_err(write_error)?;
    }
    for file in &report.unchanged {
        writeln!(out, "unchanged: {file}").map_err(write_error)?;
    }
    for file in &report.skipped {
        writeln!(out, "skipped: {file}").map_err(write_error)?;
    }
    for item in &report.unknown_items {
        writeln!(out, "unknown item preserved: {item}").map_err(write_error)?;
    }
    for key in &report.unknown_keys {
        writeln!(out, "unknown key preserved: {key}").map_err(write_error)?;
    }
    for key in &report.dropped_keys {
        writeln!(out, "dropped self-update key: {key}").map_err(write_error)?;
    }
    for warning in &report.warnings {
        writeln!(out, "warning: {warning}").map_err(write_error)?;
    }
    if let Some(backup) = &report.backup {
        writeln!(out, "backup: {}", backup.display()).map_err(write_error)?;
    }
    if let Some(path) = report_path {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(write_error)?;
        let bytes =
            serde_json::to_vec_pretty(&report).map_err(|e| Error::Internal(e.to_string()))?;
        file.write_all(&bytes).map_err(write_error)?;
    }
    writeln!(out, "result: {}", report.code()).map_err(write_error)?;
    Ok(report.code())
}
