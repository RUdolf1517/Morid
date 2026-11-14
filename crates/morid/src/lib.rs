//! Application service: assembling an engine invocation, the job queue and concurrency limit,
//! lifecycle events and single-instance IPC.
//!
//! Must not contain: rendering. Nothing below this crate knows that a user interface exists.

#![forbid(unsafe_code)]

pub mod jobs;

pub mod limit;
pub mod queue;

pub mod service;

pub mod planner;
