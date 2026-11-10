//! `morid`: the console binary.
//!
//! Phase 1 has no features: it prints its version and nothing else.

#![forbid(unsafe_code)]

use std::process::ExitCode;

fn main() -> ExitCode {
    match std::env::args().nth(1).as_deref() {
        Some("--version" | "-V") => {
            println!("morid {}", env!("CARGO_PKG_VERSION"));
            ExitCode::SUCCESS
        }
        Some(other) => {
            eprintln!("morid: unknown argument {other:?}");
            ExitCode::from(2)
        }
        None => {
            eprintln!("morid {}: no features yet", env!("CARGO_PKG_VERSION"));
            ExitCode::from(2)
        }
    }
}
