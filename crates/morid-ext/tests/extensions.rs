use morid_core::Error;
use morid_ext::{read, read_path, Executables, Target};
use serde_json::json;
use std::cell::Cell;
use std::path::{Path, PathBuf};

struct EmptySearch {
    lookups: Cell<usize>,
}
impl EmptySearch {
    fn new() -> Self {
        Self {
            lookups: Cell::new(0),
        }
    }
}
impl Executables for EmptySearch {
    fn find(&self, _: &str) -> Option<PathBuf> {
        self.lookups.set(self.lookups.get() + 1);
        None
    }
    fn bin_dir(&self) -> &Path {
        Path::new("/private/bin")
    }
}
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extensions")
}
fn valid() -> serde_json::Value {
    json!({"Name":"probe","Cmd":{"Generic":{"amd64":{"Name":"probe","Args":["probe"]}}}})
}

#[test]
fn all_shipped_extensions_load() {
    let mut files: Vec<_> = std::fs::read_dir(root())
        .expect("extensions")
        .map(|entry| entry.expect("file").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    assert_eq!(files.len(), 17);
    for target in [
        Target::current(),
        Target {
            platform: "Windows",
            arch: "amd64",
        },
        Target {
            platform: "MacOS",
            arch: "aarch64",
        },
        Target {
            platform: "Generic",
            arch: "amd64",
        },
    ] {
        for path in &files {
            let engine = read_path(path, &target, &EmptySearch::new()).unwrap_or_else(|error| {
                panic!(
                    "{}: {}/{}: {error}",
                    path.display(),
                    target.platform,
                    target.arch
                )
            });
            assert!(!engine.command.args.is_empty());
            assert!(engine.executable.is_none());
        }
    }
}

#[test]
fn invalid_extension_never_spawns() {
    for (file, rule) in [
        ("not-json", 1),
        ("no-name", 2),
        ("future-schema", 3),
        ("minimum-9x", 4),
        ("win7-only", 5),
        ("malformed-cmd", 5),
        ("invalid-connector", 5),
        ("yt-dlp-mine", 5),
    ] {
        let file = format!("{file}.json");
        let text = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data/invalid")
                .join(&file),
        )
        .expect("negative fixture");
        let search = EmptySearch::new();
        let error = match read(
            &file,
            &text,
            None,
            &Target {
                platform: "Windows",
                arch: "amd64",
            },
            &search,
        ) {
            Ok(_) => panic!("{file} accepted"),
            Err(error) => error,
        };
        assert!(
            matches!(&error,Error::Extension { file:failed,rule:failed_rule,.. } if *failed_rule==rule && failed==&file),
            "{error}"
        );
        assert_eq!(
            search.lookups.get(),
            0,
            "{file}: invalid input must not reach executable lookup"
        );
        println!("{error}");
    }
}

#[test]
fn arbitrary_thin_file_is_rejected_even_with_known_name() {
    let error = read(
        "yt-dlp-mine.json",
        r#"{"Name":"yt-dlp-nightly","Version":"4"}"#,
        Some(include_str!("../../../extensions/yt-dlp.json")),
        &Target::current(),
        &EmptySearch::new(),
    );
    assert!(matches!(error, Err(Error::Extension { rule: 5, .. })));
}
#[test]
fn unknown_keys_are_warned_not_fatal() {
    let mut document = valid();
    document["FutureFeature"] = json!({"nested":[1,2]});
    let engine = read(
        "future.json",
        &document.to_string(),
        None,
        &Target::current(),
        &EmptySearch::new(),
    )
    .expect("warning not error");
    assert_eq!(engine.raw["FutureFeature"], json!({"nested":[1,2]}));
    assert!(engine
        .warnings
        .iter()
        .any(|warning| warning.file == "future.json" && warning.key == "FutureFeature"));
}
#[test]
fn extra_arguments_keys_are_warned() {
    let text = include_str!("data/invalid/extra-arguments.json");
    let engine = read(
        "extra-arguments.json",
        text,
        None,
        &Target::current(),
        &EmptySearch::new(),
    )
    .expect("warning-only fixture");
    assert_eq!(engine.warnings[0].key, "ExtraArguments");
    assert!(engine.policy.extra_options.is_empty());
}
#[test]
fn win7_keys_are_not_used() {
    let text = include_str!("data/invalid/win7-only.json");
    for arch in ["amd64", "x86", "aarch64", "win7amd64"] {
        assert!(matches!(
            read(
                "win7.json",
                text,
                None,
                &Target {
                    platform: "Windows",
                    arch
                },
                &EmptySearch::new()
            ),
            Err(Error::Extension { rule: 5, .. })
        ));
    }
}
#[test]
fn executable_missing_means_needs_installation_not_invalid() {
    let search = EmptySearch::new();
    let engine = read(
        "probe.json",
        &valid().to_string(),
        None,
        &Target::current(),
        &search,
    )
    .expect("kept");
    assert!(engine.executable.is_none());
    assert_eq!(search.lookups.get(), 1);
}
#[test]
fn argument_policy_preserves_playlist_ids_and_placement() {
    let mut document = valid();
    document["PlayListIdArguments"] = json!(["--flat-playlist", "--get-id"]);
    document["PlaylistItemsArgument"] = json!("--playlist-items");
    document["DownloadOptionsAsLast"] = json!(true);
    document["DefaultDownLoadCmdOptions"] = json!(["--no-playlist"]);
    let engine = read(
        "probe.json",
        &document.to_string(),
        None,
        &Target::current(),
        &EmptySearch::new(),
    )
    .expect("policy");
    assert_eq!(engine.policy.playlist_ids, ["--flat-playlist", "--get-id"]);
    assert_eq!(engine.policy.playlist_items_argument, "--playlist-items");
    assert_eq!(engine.policy.download, ["--no-playlist"]);
    assert!(engine.policy.download_options_as_last);
}
#[test]
fn placeholders_keep_fixed_arguments_separate() {
    let document = json!({"Name":"safari","Cmd":{"Generic":{"amd64":{"Name":"/somewhere/safari.py","Args":["python3","-u","${BackendPath}/${CommandName}","${default}"]}}}});
    let engine = read(
        "safari.json",
        &document.to_string(),
        None,
        &Target::current(),
        &EmptySearch::new(),
    )
    .expect("placeholders");
    assert_eq!(
        engine.command.args,
        ["python3", "-u", "/private/bin/safari.py", "/private/bin"]
    );
    assert_eq!(engine.command.name, "/somewhere/safari.py");
}
#[test]
fn control_error_names_file_and_rule() {
    let mut document = valid();
    document["ControlJsonStructure"] = json!({"Connector":"xor","lhs":{},"rhs":{}});
    let error = match read(
        "bad-control.json",
        &document.to_string(),
        None,
        &Target::current(),
        &EmptySearch::new(),
    ) {
        Ok(_) => panic!("accepted"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("bad-control.json: rule 5:"));
}

#[test]
fn shipped_argument_policy_keeps_ids_distinct_from_range_flag() {
    let engine = read_path(
        &root().join("aria2c.json"),
        &Target::current(),
        &EmptySearch::new(),
    )
    .expect("aria2c");
    assert_eq!(
        engine.policy.playlist_ids,
        engine.raw["PlayListIdArguments"]
            .as_array()
            .expect("ids")
            .iter()
            .map(|value| value.as_str().expect("argument").to_owned())
            .collect::<Vec<_>>()
    );
    for platform in ["Generic", "MacOS", "Windows"] {
        let engine = read_path(
            &root().join("yt-dlp-ffmpeg.json"),
            &Target {
                platform,
                arch: "amd64",
            },
            &EmptySearch::new(),
        )
        .expect("variant");
        assert!(engine
            .policy
            .download
            .ends_with(&["--downloader".to_owned(), "ffmpeg".to_owned()]));
        assert_eq!(engine.policy.options_argument, "-f");
        assert_eq!(engine.policy.playlist_items_argument, "--playlist-items");
    }
}

#[test]
fn engine_with_interpreter_still_needs_the_backend_script() {
    struct InterpreterOnly;
    impl Executables for InterpreterOnly {
        fn bin_dir(&self) -> &Path {
            Path::new("/bin")
        }
        fn find(&self, name: &str) -> Option<PathBuf> {
            if name == "python3" {
                Some(PathBuf::from("/bin/python3"))
            } else {
                None
            }
        }
    }
    let engine = read_path(
        &root().join("safaribooks.json"),
        &Target {
            platform: "Generic",
            arch: "amd64",
        },
        &InterpreterOnly,
    )
    .expect("valid definition");
    assert!(
        engine.executable.is_none(),
        "interpreter alone is not an installed backend"
    );
}
