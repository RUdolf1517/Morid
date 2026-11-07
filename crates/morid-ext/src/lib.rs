//! Reading and validating the extension JSON (34 keys, schema 1-4), selecting the command for the
//! current OS and architecture, and evaluating `ControlJsonStructure` rules.
//!
//! Must not contain: networking or spawning.

#![forbid(unsafe_code)]
