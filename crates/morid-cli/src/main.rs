//! Console adapter. Domain and CLI logic are exercised without starting a process.
#![forbid(unsafe_code)]
use morid_engine::ExitCode;
use morid_ext::{SearchPath, Target};
use std::path::PathBuf;

fn main() -> ExitCode {
    let mut args = match std::env::args_os()
        .skip(1)
        .map(|arg| arg.into_string())
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(args) => args,
        Err(_) => {
            eprintln!("morid: arguments must be UTF-8");
            return ExitCode::from(1);
        }
    };
    morid_cli::normalize_arguments(&mut args);
    let data = args
        .iter()
        .position(|v| v == "--config")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .unwrap_or_else(morid_cli::data_directory);
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            eprintln!("morid: {error}");
            return ExitCode::from(4);
        }
    };
    let searched = morid_cli::extension_paths(&executable, &data);
    let root = searched
        .iter()
        .find(|path| path.is_dir())
        .cloned()
        .unwrap_or_else(|| searched[0].clone());
    if !matches!(args.first().map(String::as_str), Some("import" | "export"))
        && !root.is_dir()
        && !args
            .iter()
            .any(|v| ["--version", "-V", "--help", "-h"].contains(&v.as_str()))
    {
        eprintln!(
            "morid: extensions unavailable; searched {}",
            searched
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        return ExitCode::from(4);
    }
    if args.first().map(String::as_str) == Some("engines") {
        eprintln!(
            "extension paths searched: {}",
            searched
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        if let Some(index) = args.iter().position(|v| v == "--config") {
            args.drain(index..(index + 2).min(args.len()));
        }
    }
    let bin = data.join("bin");
    match morid_cli::run(
        &args,
        &root,
        &Target::current(),
        &SearchPath::from_environment(bin),
        &mut std::io::stdout().lock(),
        &mut std::io::stderr().lock(),
    ) {
        Ok(code) => ExitCode::from(code),
        Err(error) => {
            eprintln!("morid: {error}");
            ExitCode::from(error.exit_code())
        }
    }
}
