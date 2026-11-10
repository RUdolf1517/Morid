//! Console adapter. Domain and CLI logic are exercised without starting a process.
#![forbid(unsafe_code)]
use morid_engine::ExitCode;
use morid_ext::{SearchPath, Target};
use std::path::PathBuf;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // Bundled data is deliberately located relative to the checkout for Phase 2.
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let bin = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.join("bin")))
        .unwrap_or_else(|| PathBuf::from("bin"));
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
