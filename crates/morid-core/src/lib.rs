//! Domain types and rules: `Url`, `Format`, `DownloadOptions`, `EngineId`, `JobState`, `Error`,
//! option-token parsing, playlist range parsing and version comparison.
//!
//! Must not contain: file I/O, processes, network, time or randomness.

#![forbid(unsafe_code)]
