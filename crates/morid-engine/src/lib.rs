//! Spawning a backend, reading its output, applying an engine's parse rules, emitting
//! progress/metadata/destination events and cancelling a whole process tree.
//!
//! Must not contain: knowledge of widgets, or of specific engines beyond their data.

#![forbid(unsafe_code)]
