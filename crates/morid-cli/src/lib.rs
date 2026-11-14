//! Testable console adapter; all backend execution belongs to morid-engine.
#![forbid(unsafe_code)]
use morid_core::Error;
use morid_ext::{read_path, Executables, Target};
use serde_json::json;
use std::io::Write;
use std::path::Path;

pub fn run(
    args: &[String],
    directory: &Path,
    target: &Target<'_>,
    search: &dyn Executables,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<u8, Error> {
    let output_error = |_| Error::Internal("cannot write CLI output".to_owned());
    if matches!(args.first().map(String::as_str), Some("download" | "list")) {
        return download::run(args, directory, target, search, out, err);
    }
    if args == ["--help"] || args == ["-h"] {
        writeln!(out, "morid --version | engines [--json] | list URL | download URL [--dry-run] [--settings FILE] [--options TEXT] [--to DIR] [--json] | download --batch FILE").map_err(output_error)?;
        return Ok(0);
    }

    if args == ["--version"] || args == ["-V"] {
        writeln!(out, "morid {}", env!("CARGO_PKG_VERSION")).map_err(output_error)?;
        return Ok(0);
    }
    if args.first().map(String::as_str) != Some("engines")
        || args.len() > 2
        || args.get(1).is_some_and(|arg| arg != "--json")
    {
        return Err(Error::Usage(
            "usage: morid --version | engines [--json]".to_owned(),
        ));
    }
    let json_output = args.len() == 2;
    let mut files = std::fs::read_dir(directory)
        .map_err(|_| Error::Extension {
            file: directory.display().to_string(),
            rule: 1,
            message: "cannot enumerate extension directory".to_owned(),
        })?
        .map(|entry| entry.map(|entry| entry.path()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| Error::Internal("cannot enumerate extensions".to_owned()))?;
    files.retain(|file| file.extension().is_some_and(|ext| ext == "json"));
    files.sort();
    let mut status = 0;
    for file in files {
        let record = match read_path(&file, target, search) {
            Ok(extension) => {
                let verdict = if extension.executable.is_some() {
                    "ready"
                } else {
                    status = 4;
                    "needs installation"
                };
                for warning in &extension.warnings {
                    writeln!(
                        err,
                        "{}: warning: {}: {}",
                        warning.file, warning.key, warning.message
                    )
                    .map_err(output_error)?;
                }
                json!({"file":extension.file,"name":extension.id.as_str(),"schema_version":extension.schema_version,"platform":target.platform,"arch":target.arch,"selected_arch":extension.selected_arch,"command":extension.command.args,"command_name":extension.command.name,"executable":extension.executable,"verdict":verdict,"resolved":true})
            }
            Err(error) => {
                status = error.exit_code();
                writeln!(err, "{error}").map_err(output_error)?;
                let rule = match &error {
                    Error::Extension { rule, .. } => Some(*rule),
                    _ => None,
                };
                json!({"file":file.file_name().and_then(|s|s.to_str()),"platform":target.platform,"arch":target.arch,"resolved":false,"verdict":"invalid","rule":rule,"error":error.to_string()})
            }
        };
        if json_output {
            writeln!(out, "{record}").map_err(output_error)?;
        } else {
            writeln!(
                out,
                "{} | {} | schema {} | {}/{} -> {} | {} | {}",
                record["file"].as_str().unwrap_or("?"),
                record["name"].as_str().unwrap_or("?"),
                record["schema_version"],
                target.platform,
                target.arch,
                record["selected_arch"].as_str().unwrap_or("?"),
                record["command"],
                record["error"]
                    .as_str()
                    .unwrap_or_else(|| record["verdict"].as_str().unwrap_or("invalid"))
            )
            .map_err(output_error)?;
        }
    }
    Ok(status)
}

pub mod download;

/// Runtime paths only: executable-adjacent resources, then per-user appdata.
pub fn extension_paths(executable: &Path, data: &Path) -> Vec<std::path::PathBuf> {
    vec![
        executable
            .parent()
            .unwrap_or(Path::new("."))
            .join("extensions"),
        data.join("extensions"),
    ]
}
pub fn data_directory() -> std::path::PathBuf {
    use std::path::PathBuf;
    if let Some(path) = std::env::var_os("MediaDownloaderAppDataPath") {
        return PathBuf::from(path);
    }
    if cfg!(windows) {
        return std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("morid");
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    if cfg!(target_os = "macos") {
        home.join("Library/Application Support/morid")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join("morid")
    }
}

/// Move only the actual verb, never a global option's value named "list".
pub fn normalize_arguments(args: &mut Vec<String>) {
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "list" | "download" | "engines" => {
                let verb = args.remove(index);
                args.insert(0, verb);
                return;
            }
            "--settings" | "--config" | "--lang" => index += 2,
            "--json" | "--quiet" | "--verbose" | "--no-color" => index += 1,
            _ => return,
        }
    }
}
