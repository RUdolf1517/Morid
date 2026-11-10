use morid_ext::{SearchPath, Target};
use std::path::PathBuf;

#[test]
fn cli_engines_integration_without_spawning() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let search = SearchPath {
        bin: root.join("nonexistent-bin"),
        paths: vec![],
    };
    for json_output in [false, true] {
        let mut args = vec!["engines".to_owned()];
        if json_output {
            args.push("--json".to_owned());
        }
        let (mut out, mut err) = (Vec::new(), Vec::new());
        assert_eq!(
            morid_cli::run(
                &args,
                &root,
                &Target::current(),
                &search,
                &mut out,
                &mut err
            )
            .expect("CLI"),
            4
        );
        let text = String::from_utf8(out).expect("UTF-8");
        assert_eq!(text.lines().count(), 17, "{text}");
        for line in text.lines() {
            if json_output {
                let value: serde_json::Value = serde_json::from_str(line).expect("one JSON object");
                assert!(value.is_object());
                assert_eq!(value["resolved"], true, "{line}");
                assert_eq!(value["verdict"], "needs installation");
            } else {
                assert!(!line.contains("invalid"), "{line}");
            }
        }
        let diagnostics = String::from_utf8(err).expect("UTF-8");
        assert!(diagnostics.contains("ytdl-patched.json: warning: ExtraArguments"));
    }
}

#[test]
fn warning_demo_copy_of_ytdl_patched() {
    let directory =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/phase2-warning-demo");
    std::fs::create_dir_all(&directory).expect("demo directory");
    let mut document: serde_json::Value =
        serde_json::from_str(include_str!("../../../extensions/ytdl-patched.json"))
            .expect("original");
    document["FutureFeature"] = serde_json::json!({"preserve":true});
    std::fs::write(directory.join("ytdl-patched.json"), document.to_string()).expect("copy");
    let search = SearchPath {
        bin: directory.join("bin"),
        paths: vec![],
    };
    let (mut out, mut err) = (Vec::new(), Vec::new());
    assert_eq!(
        morid_cli::run(
            &["engines".to_owned()],
            &directory,
            &Target::current(),
            &search,
            &mut out,
            &mut err
        )
        .expect("CLI"),
        4
    );
    let diagnostics = String::from_utf8(err).expect("UTF-8");
    assert!(
        diagnostics.contains("ytdl-patched.json: warning: FutureFeature: unknown key preserved")
    );
    for key in [
        "ExtraArguments",
        "ExtraArgumentsFlatpak",
        "ExtraArgumentsWin7",
    ] {
        assert!(diagnostics.contains(&format!(
            "ytdl-patched.json: warning: {key}: ignored spelling"
        )));
    }
    println!("warning-demo (rule 7: unknown keys; rule 8: ExtraArguments*):\n{diagnostics}");
    std::fs::remove_dir_all(directory).expect("cleanup copy");
}
