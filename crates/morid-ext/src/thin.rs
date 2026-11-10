//! Three explicit transformations, not inheritance or name-prefix matching.
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinFile {
    Nightly,
    Ffmpeg,
    Aria2c,
}

impl ThinFile {
    pub fn for_file_name(file_name: &str) -> Option<Self> {
        match file_name {
            "yt-dlp-nightly.json" => Some(Self::Nightly),
            "yt-dlp-ffmpeg.json" => Some(Self::Ffmpeg),
            "yt-dlp-aria2c.json" => Some(Self::Aria2c),
            _ => None,
        }
    }

    /// Apply to a validated object base. The reader validates the resulting document again.
    pub fn apply(self, base: &Value, thin: &Value) -> Result<Value, String> {
        let mut object = base
            .as_object()
            .ok_or("yt-dlp.json base must be an object")?
            .clone();
        let own = thin.as_object().ok_or("thin file must be an object")?;
        let (name, version) = match self {
            Self::Nightly => ("yt-dlp-nightly", "4"),
            Self::Ffmpeg => ("yt-dlp-ffmpeg", "3"),
            Self::Aria2c => ("yt-dlp-aria2c", "3"),
        };
        object.insert("Name".to_owned(), json!(name));
        // All three legacy transformations read Version from the thin file.
        object.insert(
            "Version".to_owned(),
            match own.get("Version") {
                Some(Value::String(value)) if value.is_empty() => json!(version),
                Some(value) => value.clone(),
                None => json!(version),
            },
        );
        match self {
            Self::Nightly => {
                for key in ["AutoUpdate", "BackendPath"] {
                    if let Some(value) = own.get(key) {
                        object.insert(key.to_owned(), value.clone());
                    }
                }
                object.insert(
                    "DownloadUrl".to_owned(),
                    json!(
                        "https://api.github.com/repos/yt-dlp/yt-dlp-nightly-builds/releases/latest"
                    ),
                );
                let mut commands = Map::new();
                for (platform, entries) in [
                    (
                        "Generic",
                        vec![("amd64", "yt-dlp-nightly"), ("x86", "yt-dlp-nightly")],
                    ),
                    (
                        "MacOS",
                        vec![
                            ("amd64", "yt-dlp-nightly_macos"),
                            ("x86", "yt-dlp-nightly_macos"),
                        ],
                    ),
                    (
                        "Windows",
                        vec![
                            ("amd64", "yt-dlp-nightly.exe"),
                            ("x86", "yt-dlp_x86-nightly.exe"),
                            ("win7amd64", "yt-dlp_win7.exe"),
                            ("win7x86", "yt-dlp_x86_win7.exe"),
                        ],
                    ),
                ] {
                    let block: Map<String, Value> = entries
                        .into_iter()
                        .map(|(arch, executable)| {
                            (
                                arch.to_owned(),
                                json!({"Name":executable,"Args":[executable]}),
                            )
                        })
                        .collect();
                    commands.insert(platform.to_owned(), Value::Object(block));
                }
                object.insert("Cmd".to_owned(), Value::Object(commands));
            }
            Self::Ffmpeg | Self::Aria2c => {
                let defaults = object
                    .entry("DefaultDownLoadCmdOptions")
                    .or_insert_with(|| json!([]));
                let defaults = defaults
                    .as_array_mut()
                    .ok_or("DefaultDownLoadCmdOptions must be an array")?;
                defaults.push(json!("--downloader"));
                defaults.push(json!(if self == Self::Ffmpeg {
                    "ffmpeg"
                } else {
                    "aria2c"
                }));
                // addJsonCmd::add only changes the helper's m_obj. cmdAria2C
                // never calls done(), so the returned object's Cmd stays intact.
                // legacy/src/engines/yt-dlp.cpp:402–436; utility.h:1187–1189.
            }
        }
        // Preserve extra thin-file data for warnings; never interpret it as an override.
        for (key, value) in own {
            if !object.contains_key(key) {
                object.insert(key.clone(), value.clone());
            }
        }
        Ok(Value::Object(object))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Value {
        serde_json::from_str(include_str!("../../../extensions/yt-dlp.json")).expect("shipped base")
    }

    #[test]
    fn thin_files_resolve_by_file_name() {
        let base = base();
        let original_base = base.clone();
        for (file, name, tail) in [
            ("yt-dlp-ffmpeg.json", "yt-dlp-ffmpeg", "ffmpeg"),
            ("yt-dlp-aria2c.json", "yt-dlp-aria2c", "aria2c"),
        ] {
            let result = ThinFile::for_file_name(file)
                .expect("exact file")
                .apply(&base, &json!({"Name":"irrelevant","Version":"3"}))
                .expect("transform");
            assert_eq!(result["Name"], name);
            assert_eq!(result["Version"], "3");
            let mut expected = base["DefaultDownLoadCmdOptions"]
                .as_array()
                .expect("defaults")
                .clone();
            expected.extend([json!("--downloader"), json!(tail)]);
            assert_eq!(result["DefaultDownLoadCmdOptions"], json!(expected));
            assert_eq!(result["Cmd"], base["Cmd"]);
            assert_eq!(base, original_base, "transformation mutated the base");
            println!(
                "variant-evidence: {}",
                json!({"name":name,"Cmd":result["Cmd"],"DefaultDownLoadCmdOptions":result["DefaultDownLoadCmdOptions"]})
            );
        }
        let result = ThinFile::Nightly
            .apply(
                &base,
                &json!({"Version":"3","AutoUpdate":false,"BackendPath":"custom"}),
            )
            .expect("nightly");
        assert_eq!(result["Name"], "yt-dlp-nightly");
        assert_eq!(result["Version"], "3");
        assert_eq!(result["AutoUpdate"], false);
        assert_eq!(result["BackendPath"], "custom");
        assert_eq!(
            result["DownloadUrl"],
            "https://api.github.com/repos/yt-dlp/yt-dlp-nightly-builds/releases/latest"
        );
        assert_eq!(
            result["DefaultDownLoadCmdOptions"],
            base["DefaultDownLoadCmdOptions"]
        );
        for (platform, executable) in [
            ("Generic", "yt-dlp-nightly"),
            ("MacOS", "yt-dlp-nightly_macos"),
            ("Windows", "yt-dlp-nightly.exe"),
        ] {
            assert_eq!(
                result["Cmd"][platform]["amd64"],
                json!({"Name":executable,"Args":[executable]})
            );
        }
        assert_eq!(
            result["Cmd"]["Windows"]["x86"]["Name"],
            "yt-dlp_x86-nightly.exe"
        );
        assert_eq!(
            result["Cmd"]["Windows"]["win7amd64"]["Name"],
            "yt-dlp_win7.exe"
        );
        assert_eq!(
            result["Cmd"]["Windows"]["win7x86"]["Name"],
            "yt-dlp_x86_win7.exe"
        );
        assert_eq!(
            base, original_base,
            "nightly transformation mutated the base"
        );
        println!(
            "variant-evidence: {}",
            json!({"name":result["Name"],"Cmd":result["Cmd"],"DefaultDownLoadCmdOptions":result["DefaultDownLoadCmdOptions"]})
        );
    }

    #[test]
    fn aria2c_preserves_complete_base_cmd() {
        let mut base = base();
        // Protect unselected architectures/blocks too, not just today's platforms.
        base["Cmd"]["FuturePlatform"] =
            json!({"future-arch":{"Name":"custom","Args":["custom","--keep-me"]}});
        base["Cmd"]["Generic"]["amd64"]["Args"] = json!(["custom-wrapper", "yt-dlp"]);
        let original = base.clone();
        let result = ThinFile::Aria2c
            .apply(&base, &json!({"Name":"yt-dlp-aria2c","Version":"3"}))
            .expect("aria2c");
        assert_eq!(result["Cmd"], base["Cmd"]);
        assert_eq!(base, original);
    }

    #[test]
    fn arbitrary_thin_file_never_resolves_through_a_family() {
        for name in ["yt-dlp-mine.json", "YT-DLP-NIGHTLY.json", "custom.json"] {
            assert_eq!(ThinFile::for_file_name(name), None);
        }
        // Dispatch must not inspect Name even when it names a known variant.
        let custom = json!({"Name":"yt-dlp-nightly","Version":"4"});
        assert_eq!(ThinFile::for_file_name("yt-dlp-mine.json"), None);
        assert!(custom.get("Cmd").is_none());
    }
}
