//! Pure argv planning from extension policy + typed request + settings. No spawn.
use morid_core::{DownloadOptions, Error, Url};
use morid_ext::Extension;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const DEFAULT_FORMAT: &str = "bestvideo[ext=mp4][vcodec^=av]+bestaudio[ext=m4a]/bestvideo[ext=mp4]+bestaudio[ext=m4a]/bestvideo+bestaudio/best[ext=mp4]/best";
pub const DEFAULT_PROGRESS: &str = "download:[download] downloaded_bytes:%(progress.downloaded_bytes)s ETA:%(progress.eta)s total_bytes_estimate:%(progress.total_bytes_estimate)s total_bytes:%(progress.total_bytes)s progress.speed:%(progress.speed)s filename:%(progress.filename)s";

/// Phase 3 JSON override surface, not an INI/product storage format. Paths are
/// supplied by the adapter; defaults are relative until resolved against appdata.
#[derive(Clone, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    pub engine: String,
    pub text_encoding: Option<String>,
    pub destination: PathBuf,
    pub data_directory: PathBuf,
    pub internal_archive: bool,
    pub archive: Option<String>,
    pub cookie_browser: Option<String>,
    pub cookie_file: Option<String>,
    pub runtime_arguments: Vec<String>,
    pub progress_template: Option<String>,
    pub default_format: String,
    pub concurrency: String,
    pub proxy: Option<String>,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            engine: "yt-dlp".into(),
            text_encoding: None,
            destination: "Downloads".into(),
            data_directory: ".".into(),
            internal_archive: true,
            archive: None,
            cookie_browser: None,
            cookie_file: None,
            runtime_arguments: vec![],
            progress_template: Some(DEFAULT_PROGRESS.into()),
            default_format: DEFAULT_FORMAT.into(),
            concurrency: "4".into(),
            proxy: None,
        }
    }
}
impl Settings {
    pub fn from_data_directory(directory: &std::path::Path) -> Result<Self, Error> {
        let mut settings = Self {
            data_directory: directory.join("data"),
            ..Self::default()
        };
        let path = directory.join("settings/settings.ini");
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(settings),
            Err(e) => return Err(Error::Usage(format!("{}: {e}", path.display()))),
        };
        let doc = morid_store::ini::Document::parse(&bytes).map_err(Error::Usage)?;
        let scalar = |key: &str| {
            doc.entries()
                .find(|entry| entry.section.eq_ignore_ascii_case("General") && entry.key == key)
                .and_then(|entry| match &entry.value {
                    morid_store::value::IniValue::String(value) => Some(value.clone()),
                    _ => None,
                })
        };
        if let Some(value) = scalar("BasicDownloaderDefaultEngine") {
            settings.engine = value;
        }
        if let Some(value) = scalar("DownloadFolder") {
            settings.destination = value.into();
        }
        if let Some(value) = scalar("UseInternalArchiveFile") {
            settings.internal_archive = value == "true";
        }
        if let Some(value) = scalar("MaxConcurrentDownloads") {
            settings.concurrency = value;
        }
        settings.text_encoding =
            scalar(&format!("TextEncoding_{}", settings.engine)).filter(|value| !value.is_empty());
        if scalar("CookieSourceSetToBrowerName").as_deref() == Some("true") {
            settings.cookie_browser = scalar(&format!("CookieBrowser_{}", settings.engine))
                .filter(|value| !value.is_empty());
        } else {
            settings.cookie_file = scalar(&format!("CookieTextFilePath_{}", settings.engine))
                .filter(|value| !value.is_empty());
        }
        if scalar("ProxySettingsType").as_deref() == Some("Manual") {
            settings.proxy = scalar("ProxySettingsCustomSource").filter(|value| !value.is_empty());
        }
        Ok(settings)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Download,
    Batch,
    List,
}

pub struct Request {
    pub url: Url,
    pub options: DownloadOptions,
    pub ui_options: DownloadOptions,
    pub kind: Kind,
    pub playlist: bool,
}

pub fn plan(
    extension: &Extension,
    settings: &Settings,
    request: &Request,
) -> Result<Vec<String>, Error> {
    let p = &extension.policy;
    let yt = extension.metadata.like_youtube_dl;
    let list = request.kind == Kind::List;
    let mut args = extension.command.args.clone();
    if request.kind == Kind::Batch && yt {
        args.extend(["--match-filter", "!playlist", "--break-on-reject"].map(str::to_owned));
    }
    let mut base = if list {
        p.list.clone()
    } else {
        p.download.clone()
    };
    if list && base.is_empty() {
        return Err(Error::Extension {
            file: extension.file.clone(),
            rule: 1,
            message: "engine does not support listing".into(),
        });
    }
    if request.playlist {
        if !extension.metadata.can_download_playlist {
            return Err(Error::Extension {
                file: extension.file.clone(),
                rule: 1,
                message: "engine does not support playlists".into(),
            });
        }
        base.retain(|arg| arg != "--no-playlist");
        // The list policy's !playlist guard is for single-URL inspection; it
        // would reject every playlist entry if retained under --playlist.
        if list {
            let mut index = 0;
            while index + 1 < base.len() {
                if base[index] == "--match-filter" && base[index + 1] == "!playlist" {
                    base.drain(index..index + 2);
                } else {
                    index += 1;
                }
            }
        }
    }
    args.extend(base);
    if let Some(encoding) = settings.text_encoding.as_ref().filter(|_| list) {
        if !p.encoding_argument.is_empty() {
            args.extend([p.encoding_argument.clone(), encoding.clone()]);
        }
    }
    if yt {
        args.extend([
            "--output-na-placeholder".into(),
            if list { "\"NA\"".into() } else { "NA".into() },
        ]);
    }
    // Preserve duplicate/conflicting user options. Cookie/archive settings use
    // their own post-template slot; raw --download-archive stays in user slot.
    let selected = if p.download_options_as_last {
        if request.ui_options.0.is_empty() {
            &request.options.0
        } else {
            &request.ui_options.0
        }
    } else if request.options.0.is_empty() {
        &request.ui_options.0
    } else {
        &request.options.0
    };
    args.extend(selected.clone());
    args.extend(p.extra_options.clone());
    if yt {
        args.extend(settings.runtime_arguments.clone());
    }
    if !list && yt {
        if !selected
            .iter()
            .any(|v| ["-f", "--format", "-S", "--format-sort"].contains(&v.as_str()))
        {
            args.extend([p.options_argument.clone(), settings.default_format.clone()]);
        }
        if let Some(template) = &settings.progress_template {
            args.extend(["--progress-template".into(), template.clone()]);
        }
    }
    if let Some(browser) = &settings.cookie_browser {
        if !p.cookie_argument.is_empty() {
            args.extend([p.cookie_argument.clone(), browser.clone()]);
        }
    } else if let Some(file) = &settings.cookie_file {
        if !p.cookie_argument_text_file.is_empty() {
            args.extend([p.cookie_argument_text_file.clone(), file.clone()]);
        }
    }
    if !list && yt && !selected.iter().any(|v| v == "--download-archive") {
        if let Some(archive) = &settings.archive {
            args.extend(["--download-archive".into(), archive.clone()]);
        } else if settings.internal_archive {
            args.extend([
                "--download-archive".into(),
                settings
                    .data_directory
                    .join(extension.id.archive_file())
                    .to_string_lossy()
                    .into_owned(),
            ]);
        }
    }
    if let Some(proxy) = &settings.proxy {
        if yt {
            args.extend(["--proxy".into(), proxy.clone()]);
        }
    }
    if let Some(encoding) = settings.text_encoding.as_ref().filter(|_| !list) {
        if !p.encoding_argument.is_empty() {
            args.extend([p.encoding_argument.clone(), encoding.clone()]);
        }
    }
    args.push(request.url.as_str().into());
    Ok(args)
}

#[cfg(test)]
mod store_tests {
    use super::*;
    #[test]
    fn download_reads_the_fixture_settings_ini() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../fixtures/userdata/media-downloader");
        let settings = Settings::from_data_directory(&path).expect("fixture INI");
        assert_eq!(settings.engine, "yt-dlp");
        assert_eq!(settings.text_encoding.as_deref(), Some("ISO-8859-1"));
        assert_eq!(settings.cookie_browser.as_deref(), Some("fixture-browser"));
        assert!(settings.internal_archive);
        assert_eq!(settings.data_directory, path.join("data"));
    }
}
