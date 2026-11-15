//! Native test double only. Replays captured version bytes on the exact declared
//! VersionArgument; a wrong argument exits 41. No backend or network is used.
use std::io::Write;
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args == ["--version"] {
        return;
    } // uncaptured helper double
    if args.len() != 2 {
        std::process::exit(40);
    }
    let (argument, bytes): (&str, &[u8]) = match args[0].as_str() {
        "yt-dlp" => (
            "--version",
            include_bytes!("../../../fixtures/streams/yt-dlp/version.txt"),
        ),
        "gallery-dl" => (
            "--version",
            include_bytes!("../../../fixtures/streams/gallery-dl/version.txt"),
        ),
        "aria2c" => (
            "--version",
            include_bytes!("../../../fixtures/streams/aria2c/version.txt"),
        ),
        "wget" => (
            "--version",
            include_bytes!("../../../fixtures/streams/wget/version.txt"),
        ),
        "svtplay-dl" => (
            "--version",
            include_bytes!("../../../fixtures/streams/svtplay-dl/version.txt"),
        ),
        "you-get" => (
            "--version",
            include_bytes!("../../../fixtures/streams/you-get/version.txt"),
        ),
        "get-sauce" => (
            "-v",
            include_bytes!("../../../fixtures/streams/get-sauce/version.txt"),
        ),
        _ => ("--version", b""),
    };
    if args[1] != argument {
        std::process::exit(41);
    }
    std::io::stderr().write_all(bytes).unwrap();
}
