use morid::planner::{self, Kind, Request, Settings};
use morid_core::{DownloadOptions, Url};
use morid_ext::{read_path, SearchPath, Target};
use std::path::PathBuf;

// Per-case settings/inputs recovered from tools/capture/capture.py, not a
// scenario lookup in the implementation and not production defaults.
fn check(case: &str, options: &str, kind: Kind) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let search = SearchPath {
        bin: root.join("absent"),
        paths: vec![],
    };
    let ext = read_path(
        &root.join("extensions/yt-dlp.json"),
        &Target {
            platform: "Generic",
            arch: "amd64",
        },
        &search,
    )
    .expect("extension");
    let mut settings = Settings {
        data_directory: "<case-dir>/appdata/data".into(),
        runtime_arguments: ["--no-js-runtimes", "--js-runtimes", "node:<node>"]
            .map(str::to_owned)
            .to_vec(),
        ..Settings::default()
    };
    if case.starts_with("s09") {
        settings.internal_archive = false;
    }
    if case.starts_with("s11") {
        settings.cookie_browser = Some("fixture-browser".into());
    }
    if case.starts_with("s12") {
        settings.cookie_file = Some("<case-dir>/cookie files/fixture-cookies.txt".into());
    }
    if case.starts_with("s20") {
        settings.destination = "<case-dir>/output dir/Загрузки".into();
    }
    let url = if case.starts_with("s22") {
        "https://example.com/watch?a=1&b=two#frag%20x"
    } else {
        "https://example.com/v/fixture-01"
    };
    let request = Request {
        url: Url::parse(url).expect("url"),
        options: DownloadOptions::parse(options),
        ui_options: DownloadOptions(vec![]),
        kind,
        playlist: false,
    };
    let actual = planner::plan(&ext, &settings, &request).expect("plan");
    let expected: Vec<String> = serde_json::from_str(
        &std::fs::read_to_string(root.join(format!("fixtures/scenarios/{case}/argv.json")))
            .expect("golden"),
    )
    .expect("argv");
    assert_eq!(&actual[1..], expected, "{case}");
    if kind == Kind::Batch {
        let second = Request {
            url: Url::parse("https://example.com/v/fixture-02").expect("url"),
            ..request
        };
        let next = planner::plan(&ext, &settings, &second).expect("second URL");
        assert_eq!(&actual[..actual.len() - 1], &next[..next.len() - 1]);
        assert_eq!(
            next.last().map(String::as_str),
            Some("https://example.com/v/fixture-02")
        );
    }
}
macro_rules! golden {
    ($name:ident,$case:literal,$options:literal,$kind:ident) => {
        #[test]
        fn $name() {
            check($case, $options, Kind::$kind);
        }
    };
}
golden!(
    dry_run_argv_matches_golden_s01,
    "s01-plain-yt-dlp",
    "",
    Download
);
golden!(dry_run_argv_matches_golden_s02,"s02-bundled-preset","-f bestvideo[format_note*=144p][ext=mp4][vcodec^=avc]+bestaudio[ext=m4a]/bestvideo[format_note*=144p]+bestaudio",Download);
golden!(
    dry_run_argv_matches_golden_s03,
    "s03-user-format-22",
    "-f 22",
    Download
);
golden!(
    dry_run_argv_matches_golden_s04,
    "s04-conflicting-user-options",
    "-f 22 -f 18 --no-playlist --yes-playlist",
    Download
);
golden!(dry_run_argv_matches_golden_s05, "s05-format-list", "", List);
golden!(
    dry_run_argv_matches_golden_s09,
    "s09-download-archive",
    "--download-archive \"<case-dir>/archive folder/archive.txt\"",
    Download
);
golden!(
    dry_run_argv_matches_golden_s10,
    "s10-batch-two-urls",
    "",
    Batch
);
golden!(
    dry_run_argv_matches_golden_s11,
    "s11-cookies-browser",
    "",
    Download
);
golden!(
    dry_run_argv_matches_golden_s12,
    "s12-cookies-file",
    "",
    Download
);
golden!(
    dry_run_argv_matches_golden_s17,
    "s17-subtitle-languages",
    "--write-subs --sub-langs \"en,pl\"",
    Download
);
golden!(dry_run_argv_matches_golden_s19,"s19-split-chapters","-o \"chapter:%(title)s-%(section_number)02d.%(ext)s\" --download-sections 00:00:00-00:00:01 --download-sections 00:00:01-00:00:02 --split-chapters",Batch);
golden!(
    dry_run_argv_matches_golden_s20,
    "s20-output-dir-space-unicode",
    "-P \"<case-dir>/output dir/Загрузки\"",
    Download
);
golden!(
    dry_run_argv_matches_golden_s21,
    "s21-user-output-template",
    "-o \"%(title)s from user.%(ext)s\"",
    Download
);
golden!(
    dry_run_argv_matches_golden_s22,
    "s22-url-metacharacters",
    "",
    Download
);
golden!(
    dry_run_argv_matches_golden_s37,
    "s37-audio-extraction",
    "-x --audio-format mp3 -f bestaudio",
    Download
);

#[test]
fn dry_run_never_spawns() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let search = SearchPath {
        bin: root.join("no-binaries"),
        paths: vec![],
    };
    let args = [
        "download",
        "https://example.test/opaque?x=1&y=2",
        "--dry-run",
        "--no-archive",
        "--to",
        "does-not-exist",
    ]
    .map(str::to_owned);
    let mut out = Vec::new();
    assert_eq!(
        morid_cli::run(
            &args,
            &root,
            &Target::current(),
            &search,
            &mut out,
            &mut Vec::new()
        )
        .expect("dry run"),
        0
    );
    let argv: Vec<String> = serde_json::from_slice(&out).expect("vector");
    assert_eq!(
        argv.last().map(String::as_str),
        Some("https://example.test/opaque?x=1&y=2")
    );
    assert!(search.paths.is_empty());
}
#[test]
fn list_parses_fixture_metadata() {
    let meta = morid_cli::download::parse_metadata(include_str!(
        "../../../fixtures/streams/yt-dlp/list.json"
    ))
    .expect("metadata");
    assert_eq!(meta["title"], "video");
    assert_eq!(meta["formats"][0]["id"], "mp4");
    assert!(morid_cli::download::parse_metadata("[]").is_err());
}
