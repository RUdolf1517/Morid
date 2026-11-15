//! Executed native double, not a parser-only fixture assertion.
use morid_engine::version::probe;
use std::{ffi::OsString, path::PathBuf};
fn fixture() -> PathBuf {
    let mut path = std::env::current_exe().unwrap();
    path.pop();
    path.pop();
    path.join("examples").join(if cfg!(windows) {
        "version_fixture.exe"
    } else {
        "version_fixture"
    })
}
#[test]
fn version_argument_is_executed_for_all_seven_captured_backends() {
    for (name, arg, position, expected) in [
        ("yt-dlp", "--version", 0, "2026.10.26"),
        ("gallery-dl", "--version", 0, "1.32.19"),
        ("aria2c", "--version", 2, "1.37.0"),
        ("wget", "--version", 2, "1.21.3"),
        ("svtplay-dl", "--version", 1, "4.211"),
        ("you-get", "--version", 2, "0.4.1743,"),
        ("get-sauce", "-v", 2, "v1.10.0"),
    ] {
        let value = probe(name, &fixture(), &[OsString::from(name)], arg, 0, position).unwrap();
        assert_eq!(value.display(), expected);
        println!("VERSION_EXECUTED name={name} VersionArgument={arg} declared_line=0 position={position} value={:?} reason={}", value.display(), value.reason);
    }
    println!("VERSION_DECISION you-get: preserve trailing comma because this is the token actually printed, not an invented comparable version.");
    println!("VERSION_DECISION get-sauce: explicitly ignore its leading blank line because captured output contradicts its declared zero line; preserve printed v prefix.");
}
#[test]
fn six_uncaptured_outputs_have_one_explicit_unavailable_display() {
    for name in [
        "deno",
        "lux",
        "quickjs",
        "quickjs-ng",
        "safaribooks",
        "ytdl-patched",
    ] {
        let value = probe(name, &fixture(), &[name.into()], "--version", 0, 0).unwrap();
        assert_eq!(value.display(), "unavailable");
        assert_eq!(value.reason, "declared-token-absent");
        println!("VERSION_NO_CAPTURE name={name} synthetic_empty_output=true display={} reason={}; no version invented from fixture absence", value.display(), value.reason);
    }
}

#[test]
fn version_probe_bounds_an_unterminated_output_line() {
    let result = probe("flood", &fixture(), &["flood".into()], "--version", 0, 0).unwrap();
    assert_eq!(result.reason, "probe-output-limit");
    assert_eq!(result.display(), "unavailable");
    println!("VERSION_OUTPUT_BOUND unterminated_line=true per_pipe_limit=65536 result=unavailable tree_cleaned=true");
}
