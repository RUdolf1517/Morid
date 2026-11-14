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
    let canonical = actual[1..]
        .iter()
        .map(|arg| {
            if arg.starts_with("<case-dir>") {
                arg.replace('\\', "/")
            } else {
                arg.clone()
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(canonical, expected, "{case}");
    // Exercise the real CLI override/flag adapter too, not merely planner output.
    let settings_path = std::env::temp_dir().join(format!("morid-{case}-settings.json"));
    std::fs::write(
        &settings_path,
        serde_json::to_vec(&settings).expect("settings JSON"),
    )
    .expect("settings file");
    let batch_path = settings_path.with_extension("urls");
    let mut cli = vec![
        if kind == Kind::List {
            "list"
        } else {
            "download"
        }
        .into(),
        "--settings".into(),
        settings_path.to_string_lossy().into_owned(),
        "--options".into(),
        options.into(),
        "--dry-run".into(),
    ];
    if kind == Kind::Batch {
        std::fs::write(
            &batch_path,
            format!("{url}\nhttps://example.com/v/fixture-02\n"),
        )
        .expect("batch");
        cli.extend(["--batch".into(), batch_path.to_string_lossy().into_owned()]);
    } else {
        cli.push(url.into());
    }
    let mut stdout = Vec::new();
    assert_eq!(
        morid_cli::run(
            &cli,
            &root.join("extensions"),
            &Target {
                platform: "Generic",
                arch: "amd64"
            },
            &search,
            &mut stdout,
            &mut Vec::new()
        )
        .expect("CLI dry run"),
        0
    );
    let lines = String::from_utf8(stdout).expect("UTF8");
    let rows = lines
        .lines()
        .map(|line| serde_json::from_str::<Vec<String>>(line).expect("argv JSON"))
        .collect::<Vec<_>>();
    assert_eq!(rows[0], actual);
    assert_eq!(rows.len(), if kind == Kind::Batch { 2 } else { 1 });
    println!(
        "{case} DRY-RUN {} GOLDEN {}",
        serde_json::to_string(&rows[0][1..]).expect("argv"),
        serde_json::to_string(&expected).expect("golden")
    );
    let _ = std::fs::remove_file(settings_path);
    let _ = std::fs::remove_file(batch_path);

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
    let meta = morid::metadata::MediaMeta::from_yt_dlp(include_str!(
        "../../../fixtures/streams/yt-dlp/list.json"
    ))
    .expect("metadata");
    let meta = serde_json::to_value(meta).expect("projection JSON");
    assert_eq!(meta["title"], "video");
    assert_eq!(meta["formats"][0]["id"], "mp4");
    assert!(morid::metadata::MediaMeta::from_yt_dlp("[]").is_err());
}

#[test]
fn url_round_trips_byte_for_byte_from_phase0_fixtures_and_s07_range_is_opaque() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/scenarios");
    let mut urls = 0;
    for directory in std::fs::read_dir(&root).expect("scenarios") {
        let argv = directory.expect("directory").path().join("argv.json");
        if !argv.is_file() {
            continue;
        }
        let values: Vec<String> =
            serde_json::from_str(&std::fs::read_to_string(argv).expect("fixture")).expect("argv");
        for value in values {
            if value.starts_with("http://") || value.starts_with("https://") {
                assert_eq!(
                    Url::parse(&value).expect("fixture URL").as_str().as_bytes(),
                    value.as_bytes()
                );
                urls += 1;
            }
        }
    }
    assert!(
        urls >= 30,
        "must cover real scenario URLs, not invented samples"
    );
    let range = morid_core::PlaylistRange::parse("--playlist-items 2-3");
    let golden: Vec<String> = serde_json::from_str(include_str!(
        "../../../fixtures/scenarios/s07-playlist-item-range/argv.json"
    ))
    .expect("s07");
    assert!(golden.windows(2).any(|pair| pair == range.tokens()));
    assert_eq!(
        morid_core::PlaylistRange::parse("garbage --x").tokens(),
        ["garbage", "--x"]
    );
}

#[test]
fn global_options_do_not_mistake_values_for_commands() {
    let mut args = [
        "--settings",
        "list",
        "--json",
        "download",
        "https://example.test/x",
    ]
    .map(str::to_owned)
    .to_vec();
    morid_cli::normalize_arguments(&mut args);
    assert_eq!(
        args,
        [
            "download",
            "--settings",
            "list",
            "--json",
            "https://example.test/x"
        ]
    );
}

#[test]
fn list_playlist_removes_single_url_guard_and_checks_engine_capability() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let search = SearchPath {
        bin: root.join("absent"),
        paths: vec![],
    };
    let mut extension =
        read_path(&root.join("yt-dlp.json"), &Target::current(), &search).expect("extension");
    let request = Request {
        url: Url::parse("https://example.test/playlist").expect("URL"),
        options: DownloadOptions(vec![]),
        ui_options: DownloadOptions(vec![]),
        kind: Kind::List,
        playlist: true,
    };
    let argv = planner::plan(&extension, &Settings::default(), &request).expect("playlist plan");
    assert!(!argv
        .iter()
        .any(|arg| arg == "--no-playlist" || arg == "!playlist"));
    assert!(argv.iter().any(|arg| arg == "--print"));
    extension.metadata.can_download_playlist = false;
    assert_eq!(
        planner::plan(&extension, &Settings::default(), &request)
            .expect_err("unsupported playlist")
            .exit_code(),
        4
    );
}
