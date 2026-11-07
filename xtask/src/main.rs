//! Build helper: measures the two release binaries and prints their sizes.
//!
//! Run from the repository root after `cargo build --release`:
//!
//! ```sh
//! cargo run --manifest-path xtask/Cargo.toml -- size
//! ```
//!
//! It prints and appends nothing: recording the numbers is a manual step
//! (`docs/dependencies.md` §6). A missing binary is an error, because a
//! measurement that cannot be taken is not a measurement.

#![forbid(unsafe_code)]

use std::path::PathBuf;
use std::process::ExitCode;

const BINARIES: [&str; 2] = ["morid", "morid-gui"];

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("size") => size(),
        Some(other) => {
            eprintln!("xtask: unknown subcommand {other:?}, try `size`");
            ExitCode::from(2)
        }
        None => {
            eprintln!("xtask: usage: xtask size");
            ExitCode::from(2)
        }
    }
}

fn size() -> ExitCode {
    let mut target = PathBuf::from("target");
    if let Some(directory) = std::env::var_os("CARGO_TARGET_DIR") {
        target = PathBuf::from(directory);
    }
    let mut failed = false;

    for name in BINARIES {
        let file_name = format!("{name}{}", std::env::consts::EXE_SUFFIX);
        let path = target.join("release").join(file_name);
        match std::fs::metadata(&path) {
            Ok(metadata) => {
                let bytes = metadata.len();
                let mib = bytes as f64 / (1024.0 * 1024.0);
                println!("{name}: {bytes} bytes ({mib:.2} MiB) at {}", path.display());
            }
            Err(error) => {
                eprintln!("xtask: cannot measure {}: {error}", path.display());
                failed = true;
            }
        }
    }

    if failed {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
