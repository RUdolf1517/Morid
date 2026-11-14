//! Spawning a backend, reading its output, applying an engine's parse rules, emitting
//! progress/metadata/destination events and cancelling a whole process tree.
//!
//! Must not contain: knowledge of widgets, or of specific engines beyond their data.

#![deny(unsafe_code)]

// Keep the Phase 2 CLI exit-status re-export.
pub use std::process::ExitCode;

#[cfg(windows)]
pub mod job_object;
pub mod managed;
pub mod output;
#[cfg(unix)]
pub mod process_group;
pub mod spawn;

pub mod stream;

#[cfg(windows)]
pub use job_object::{install_interrupt_handler, interrupted};
#[cfg(unix)]
pub use process_group::{install_interrupt_handler, interrupted};
