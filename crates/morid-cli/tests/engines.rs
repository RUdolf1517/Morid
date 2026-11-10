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
