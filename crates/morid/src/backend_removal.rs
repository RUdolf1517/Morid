//! Removal only touches a private executable, never an extension description.
use morid_core::Error;
use morid_ext::{Executables, Target};
use std::{fs, path::Path};

/// Built-in identities include helpers, independent of whether their executable
/// happens to be installed. Matches the bundled protection required by Task 6.
pub fn bundled(name: &str) -> bool {
    matches!(
        name,
        "aria2c"
            | "deno"
            | "gallery-dl"
            | "getsauce"
            | "lux"
            | "quickjs"
            | "quickjs-ng"
            | "safaribooks"
            | "svtplay-dl"
            | "wget"
            | "you-get"
            | "yt-dlp"
            | "yt-dlp-aria2c"
            | "yt-dlp-aria2c-native"
            | "yt-dlp-ffmpeg"
            | "yt-dlp-nightly"
            | "ytdl-patched"
            | "ffmpeg"
            | "python"
            | "tar"
            | "bsdtar"
    )
}

pub fn remove(
    name: &str,
    extensions: &Path,
    target: &Target<'_>,
    search: &dyn Executables,
) -> Result<&'static str, Error> {
    if bundled(name) {
        return Err(Error::Usage(format!("cannot remove bundled backend {name}: required bundled engine; extension JSON is retained")));
    }
    let files = fs::read_dir(extensions).map_err(|e| Error::Internal(e.to_string()))?;
    for file in files {
        let path = file.map_err(|e| Error::Internal(e.to_string()))?.path();
        if path.extension().is_none_or(|suffix| suffix != "json") {
            continue;
        }
        let Ok(extension) = morid_ext::read_path(&path, target, search) else {
            continue;
        };
        if extension.id.as_str() != name {
            continue;
        }
        let executable = &extension.command.name;
        if executable.is_empty()
            || executable.contains(['/', '\\', ':'])
            || matches!(executable.as_str(), "." | "..")
        {
            return Err(Error::Usage(
                "removal requires a private executable basename".into(),
            ));
        }
        let protected = executable.strip_suffix(".exe").unwrap_or(executable);
        if bundled(protected)
            || matches!(
                protected,
                "get-sauce"
                    | "gallery-dl.bin"
                    | "qjs-linux-x86"
                    | "qjs-linux-x86_64"
                    | "qjs-linux-aarch64"
                    | "qjs-darwin"
                    | "python3"
            )
        {
            return Err(Error::Usage(format!("cannot remove {name}: it shares bundled backend executable {executable}; extension JSON is retained")));
        }
        let destination = extension
            .executable
            .as_ref()
            .filter(|path| path.parent() == Some(search.bin_dir()))
            .cloned()
            .unwrap_or_else(|| search.bin_dir().join(executable));
        match fs::symlink_metadata(&destination) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok("already absent"),
            Err(e) => return Err(Error::Destination(e.to_string())),
            Ok(metadata) if metadata.file_type().is_file() => {}
            Ok(_) => {
                return Err(Error::Usage(format!(
                    "refuse removal of non-regular executable {}",
                    destination.display()
                )))
            }
        }
        fs::remove_file(destination).map_err(|e| Error::Destination(e.to_string()))?;
        return Ok("removed private executable; extension JSON retained");
    }
    Err(Error::Extension {
        file: name.into(),
        rule: 1,
        message: "unknown engine name".into(),
    })
}
