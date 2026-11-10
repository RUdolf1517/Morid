//! Extension validation and command resolution. Never executes a command.
#![forbid(unsafe_code)]

pub mod control;
pub mod thin;

use morid_core::{EngineId, Error, Version};
use serde::Deserialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize)]
pub struct Command {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Args")]
    pub args: Vec<String>,
}

/// Known argument keys are typed; the complete document remains alongside them.
#[derive(Default, Debug, Clone, Deserialize)]
#[serde(default, rename_all = "PascalCase")]
pub struct ArgPolicy {
    #[serde(rename = "DefaultDownLoadCmdOptions")]
    pub download: Vec<String>,
    #[serde(rename = "DefaultListCmdOptions")]
    pub list: Vec<String>,
    #[serde(rename = "DumptJsonArguments")]
    pub metadata: Vec<String>,
    #[serde(rename = "DefaultCommentsCmdOptions")]
    pub comments: Vec<String>,
    #[serde(rename = "DefaultSubstitlesCmdOptions")]
    pub subtitles: Vec<String>,
    #[serde(rename = "DefaultSubtitleDownloadOptions")]
    pub subtitle_download: Vec<String>,
    #[serde(rename = "PlayListIdArguments")]
    pub playlist_ids: Vec<String>,
    pub options_argument: String,
    pub playlist_items_argument: String,
    pub batch_file_argument: String,
    pub cookie_argument: String,
    pub cookie_argument_text_file: String,
    pub encoding_argument: String,
    pub download_options_as_last: bool,
    pub extra_options: Vec<String>,
    pub extra_options_flatpak_app_image: Vec<String>,
    pub extra_options_win7: Vec<String>,
}

#[derive(Deserialize)]
struct Known {
    #[serde(rename = "Cmd")]
    commands: BTreeMap<String, BTreeMap<String, Command>>,
    #[serde(flatten)]
    policy: ArgPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    pub file: String,
    pub key: String,
    pub message: String,
}

pub struct Extension {
    pub file: String,
    pub id: EngineId,
    pub schema_version: u32,
    /// Includes unknown keys. Credentials in this value must never be logged.
    pub raw: Value,
    pub policy: ArgPolicy,
    pub control: control::Control,
    pub command: Command,
    pub selected_arch: String,
    pub executable: Option<PathBuf>,
    pub warnings: Vec<Warning>,
}

pub struct Target<'a> {
    pub platform: &'a str,
    pub arch: &'a str,
}
impl Target<'static> {
    pub fn current() -> Self {
        Self {
            platform: if cfg!(windows) {
                "Windows"
            } else if cfg!(target_os = "macos") {
                "MacOS"
            } else {
                "Generic"
            },
            arch: match std::env::consts::ARCH {
                "x86_64" => "amd64",
                "x86" => "x86",
                "aarch64" => "aarch64",
                other => other,
            },
        }
    }
}

/// Existence checks are injectable; tests do not need a process or an ambient PATH.
pub trait Executables {
    fn find(&self, executable: &str) -> Option<PathBuf>;
    fn bin_dir(&self) -> &Path;
}

pub struct SearchPath {
    pub bin: PathBuf,
    pub paths: Vec<PathBuf>,
}
impl SearchPath {
    pub fn from_environment(bin: PathBuf) -> Self {
        Self {
            bin,
            paths: std::env::var_os("PATH")
                .map(|value| std::env::split_paths(&value).collect())
                .unwrap_or_default(),
        }
    }
}
impl Executables for SearchPath {
    fn bin_dir(&self) -> &Path {
        &self.bin
    }
    fn find(&self, executable: &str) -> Option<PathBuf> {
        fn usable(path: &Path) -> bool {
            let Ok(metadata) = path.metadata() else {
                return false;
            };
            if !metadata.is_file() {
                return false;
            }
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                metadata.permissions().mode() & 0o111 != 0
            }
            #[cfg(not(unix))]
            {
                true
            }
        }
        let name = Path::new(executable);
        let candidates = if name.is_absolute() {
            vec![name.to_path_buf()]
        } else {
            std::iter::once(self.bin.join(name))
                .chain(self.paths.iter().map(|dir| dir.join(name)))
                .collect()
        };
        for candidate in candidates {
            if usable(&candidate) {
                return Some(candidate);
            }
            #[cfg(windows)]
            if candidate.extension().is_none() {
                for ext in ["exe", "cmd", "bat", "com"] {
                    let path = candidate.with_extension(ext);
                    if usable(&path) {
                        return Some(path);
                    }
                }
            }
        }
        None
    }
}

fn failure(file: &str, rule: u8, message: impl Into<String>) -> Error {
    Error::Extension {
        file: file.to_owned(),
        rule,
        message: message.into(),
    }
}

const KEYS: &[&str] = &[
    "Name",
    "Version",
    "RequiredMinimumVersionOfMediaDownloader",
    "LikeYoutubeDl",
    "CanDownloadPlaylist",
    "AutoUpdate",
    "UsePrivateExecutable",
    "SupportingEngine",
    "UpdatableSupportingEngine",
    "ArchiveContainsFolder",
    "BackendPath",
    "UserName",
    "Password",
    "Cmd",
    "ControlJsonStructure",
    "DefaultDownLoadCmdOptions",
    "DefaultListCmdOptions",
    "DumptJsonArguments",
    "DefaultCommentsCmdOptions",
    "DefaultSubstitlesCmdOptions",
    "DefaultSubtitleDownloadOptions",
    "OptionsArgument",
    "PlaylistItemsArgument",
    "PlayListIdArguments",
    "BatchFileArgument",
    "VersionArgument",
    "CookieArgument",
    "CookieArgumentTextFile",
    "EncodingArgument",
    "DownloadOptionsAsLast",
    "RemoveText",
    "SkipLineWithText",
    "SplitLinesBy",
    "ReplaceOutputWithProgressReport",
    "ShowListTableBoundary",
    "ExtraOptions",
    "ExtraOptionsFlatpakAppImage",
    "ExtraOptionsWin7",
    "DownloadUrl",
    "DownloadUrlWin7",
    "DownloadUrlMAC",
    "VersionStringLine",
    "VersionStringPosition",
];

/// Input values stay opaque in errors: an invalid value might contain a credential.
pub fn read(
    file: &str,
    text: &str,
    base_text: Option<&str>,
    target: &Target<'_>,
    search: &dyn Executables,
) -> Result<Extension, Error> {
    let input: Value = serde_json::from_str(text).map_err(|_| failure(file, 1, "invalid JSON"))?;
    if !input.is_object() {
        return Err(failure(file, 1, "extension must be an object"));
    }
    validate_identity(file, &input)?;
    let raw = match thin::ThinFile::for_file_name(file) {
        None => input.clone(),
        Some(transform) => {
            let base: Value = serde_json::from_str(
                base_text.ok_or_else(|| failure(file, 5, "missing yt-dlp.json base"))?,
            )
            .map_err(|_| failure(file, 5, "invalid yt-dlp.json base JSON"))?;
            validate_identity("yt-dlp.json", &base)
                .map_err(|error| failure(file, 5, error.to_string()))?;
            transform
                .apply(&base, &input)
                .map_err(|message| failure(file, 5, message))?
        }
    };
    let (id, schema_version) = validate_identity(file, &raw)?;
    let known: Known = serde_json::from_value(raw.clone())
        .map_err(|_| failure(file, 5, "malformed Cmd or argument policy"))?;
    // Reject malformed commands, including unselected blocks, rather than hiding bad data.
    for block in known.commands.values() {
        for command in block.values() {
            if command.name.is_empty() || command.args.is_empty() || command.args[0].is_empty() {
                return Err(failure(file, 5, "Cmd requires nonempty Name and Args[0]"));
            }
        }
    }
    let arches = if target.arch == "amd64" {
        vec!["amd64"]
    } else {
        vec![target.arch, "amd64"]
    };
    let mut selected = None;
    for platform in [target.platform, "Generic"] {
        if let Some(block) = known.commands.get(platform) {
            for arch in &arches {
                if !arch.starts_with("win7") {
                    if let Some(command) = block.get(*arch) {
                        selected = Some((command.clone(), (*arch).to_owned()));
                        break;
                    }
                }
            }
        }
        if selected.is_some() {
            break;
        }
    }
    let (mut command, selected_arch) = selected
        .ok_or_else(|| failure(file, 5, "no command for supported platform/architecture"))?;
    let bin = search.bin_dir().to_string_lossy();
    let name = command
        .name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(&command.name)
        .to_owned();
    for arg in &mut command.args {
        *arg = arg
            .replace("${default}", &bin)
            .replace("${BackendPath}", &bin)
            .replace("${CommandName}", &name);
    }
    command.name = command
        .name
        .replace("${default}", &bin)
        .replace("${BackendPath}", &bin)
        .replace("${CommandName}", &name);
    let control = control::Control::parse(
        raw.get("ControlJsonStructure")
            .unwrap_or(&serde_json::json!({})),
    )
    .map_err(|message| failure(file, 5, message))?;
    let executable = search.find(&command.args[0]);
    let mut warnings = Vec::new();
    for object in [&raw, &input] {
        if let Some(object) = object.as_object() {
            for key in object.keys() {
                if !KEYS.contains(&key.as_str())
                    && !warnings.iter().any(|warning: &Warning| warning.key == *key)
                {
                    warnings.push(Warning {
                        file: file.to_owned(),
                        key: key.clone(),
                        message: if key.starts_with("ExtraArguments") {
                            "ignored spelling; use ExtraOptions keys".to_owned()
                        } else {
                            "unknown key preserved".to_owned()
                        },
                    });
                }
            }
        }
    }
    Ok(Extension {
        file: file.to_owned(),
        id,
        schema_version,
        raw,
        policy: known.policy,
        control,
        command,
        selected_arch,
        executable,
        warnings,
    })
}

fn validate_identity(file: &str, raw: &Value) -> Result<(EngineId, u32), Error> {
    let id = raw
        .get("Name")
        .and_then(Value::as_str)
        .ok_or_else(|| failure(file, 2, "missing string Name"))?;
    let id = EngineId::parse(id).map_err(|_| failure(file, 2, "invalid Name"))?;
    let version = match raw.get("Version") {
        None => 1,
        Some(Value::String(version)) => version
            .parse::<u32>()
            .map_err(|_| failure(file, 3, "Version must be an integer string"))?,
        _ => return Err(failure(file, 3, "Version must be an integer string")),
    };
    if !(1..=4).contains(&version) {
        return Err(failure(
            file,
            3,
            format!("schema version {version} unsupported; supported 1..4"),
        ));
    }
    let required = match raw.get("RequiredMinimumVersionOfMediaDownloader") {
        None => None,
        Some(Value::String(s)) => Some(s.as_str()),
        _ => {
            return Err(failure(
                file,
                4,
                "minimum application version must be a string",
            ))
        }
    };
    if !Version::satisfies(required, "0.1.0")
        .map_err(|_| failure(file, 4, "invalid minimum application version"))?
    {
        return Err(failure(
            file,
            4,
            "requires a newer application; running 0.1.0",
        ));
    }
    Ok((id, version))
}

pub fn read_path(
    path: &Path,
    target: &Target<'_>,
    search: &dyn Executables,
) -> Result<Extension, Error> {
    let file = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| failure("<path>", 1, "non-Unicode file name"))?;
    let text =
        std::fs::read_to_string(path).map_err(|_| failure(file, 1, "cannot read extension"))?;
    let base = if thin::ThinFile::for_file_name(file).is_some() {
        Some(
            std::fs::read_to_string(path.with_file_name("yt-dlp.json"))
                .map_err(|_| failure(file, 5, "cannot read yt-dlp.json base"))?,
        )
    } else {
        None
    };
    read(file, &text, base.as_deref(), target, search)
}
