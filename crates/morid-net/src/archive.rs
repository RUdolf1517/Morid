//! Bounded preflight before extraction; no archive-provided filesystem operations.
use morid_core::Error;
use std::collections::BTreeSet;
use std::fs::{self, OpenOptions};
use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};

const MAX_BYTES: u64 = 512 * 1024 * 1024;
const MAX_ENTRIES: usize = 10_000;

#[derive(Clone, Copy, Debug)]
pub enum Format {
    TarGz,
    Zip,
}
struct Entry {
    path: PathBuf,
    bytes: Option<Vec<u8>>,
}
fn refused(name: &str, reason: &str) -> Error {
    Error::BackendFailure(format!("archive entry {name:?} refused: {reason}"))
}
fn malformed(error: impl std::fmt::Display) -> Error {
    Error::BackendFailure(format!("archive: {error}"))
}

fn path(name: &str) -> Result<PathBuf, Error> {
    if name.is_empty()
        || name.len() > 4096
        || name.starts_with('/')
        || name.contains(['\\', ':', '\0'])
        || name.split('/').any(|p| p == "..")
    {
        return Err(refused(name, "non-relative or non-portable path"));
    }
    // A conventional leading ./ is harmless; never strip a parent component.
    let name = name.trim_end_matches('/');
    let name = name.trim_start_matches("./");
    if name.is_empty() {
        return Err(refused(name, "empty path"));
    }
    for part in name.split('/') {
        let stem = part.split('.').next().unwrap_or("").to_ascii_uppercase();
        if part.is_empty()
            || part.ends_with(['.', ' '])
            || part.chars().any(|c| c.is_control())
            || matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            return Err(refused(name, "ambiguous portable filename"));
        }
    }
    Ok(PathBuf::from(name))
}
fn read_bounded(reader: &mut impl Read, name: &str, remaining: &mut u64) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    reader
        .take(*remaining + 1)
        .read_to_end(&mut bytes)
        .map_err(malformed)?;
    if bytes.len() as u64 > *remaining {
        return Err(refused(name, "expanded size limit"));
    }
    *remaining -= bytes.len() as u64;
    Ok(bytes)
}

/// Caller supplies an empty, private staging directory. Preflight all entries
/// before writing any file. Reject all links, devices and duplicate names.
pub fn extract(body: &[u8], format: Format, destination: &Path) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(destination).map_err(malformed)?;
    if !metadata.file_type().is_dir()
        || fs::read_dir(destination)
            .map_err(malformed)?
            .next()
            .is_some()
    {
        return Err(malformed(
            "destination must be an empty, non-symlink staging directory",
        ));
    }
    let mut plan = Vec::new();
    let mut remaining = MAX_BYTES;
    let mut names = BTreeSet::new();
    match format {
        Format::TarGz => {
            let gzip = flate2::read::GzDecoder::new(body);
            let mut archive = tar::Archive::new(gzip);
            for item in archive.entries().map_err(malformed)? {
                let mut item = item.map_err(malformed)?;
                let raw = item.path_bytes().into_owned();
                let name = std::str::from_utf8(&raw)
                    .map_err(|_| refused(&String::from_utf8_lossy(&raw), "invalid UTF-8"))?
                    .to_owned();
                let kind = item.header().entry_type();
                if kind.is_dir() && matches!(name.as_str(), "." | "./") {
                    continue;
                }
                let path = path(&name)?;
                if !(kind.is_file() || kind.is_dir()) {
                    return Err(refused(&name, "link or special entry"));
                }
                let bytes = if kind.is_dir() {
                    None
                } else {
                    Some(read_bounded(&mut item, &name, &mut remaining)?)
                };
                push(&mut plan, &mut names, Entry { path, bytes }, &name)?;
            }
        }
        Format::Zip => {
            let mut archive = zip::ZipArchive::new(Cursor::new(body)).map_err(malformed)?;
            if archive.len() > MAX_ENTRIES {
                return Err(malformed("entry count limit"));
            }
            for i in 0..archive.len() {
                let mut item = archive.by_index(i).map_err(malformed)?;
                let name = item.name().to_owned();
                let path = path(&name)?;
                let kind = item.unix_mode().unwrap_or(0) & 0o170000;
                if kind != 0 && kind != 0o100000 && kind != 0o040000 {
                    return Err(refused(&name, "link or special entry"));
                }
                let bytes = if item.is_dir() {
                    None
                } else {
                    Some(read_bounded(&mut item, &name, &mut remaining)?)
                };
                push(&mut plan, &mut names, Entry { path, bytes }, &name)?;
            }
        }
    }
    // A file used as another entry's parent is also rejected before writing.
    for entry in &plan {
        if entry.bytes.is_some()
            && plan
                .iter()
                .any(|other| other.path != entry.path && other.path.starts_with(&entry.path))
        {
            return Err(refused(
                &entry.path.to_string_lossy(),
                "file is a parent directory",
            ));
        }
    }
    for entry in plan {
        let output = destination.join(entry.path);
        if let Some(bytes) = entry.bytes {
            if let Some(parent) = output.parent() {
                fs::create_dir_all(parent).map_err(malformed)?;
            }
            let mut file = OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output)
                .map_err(malformed)?;
            file.write_all(&bytes).map_err(malformed)?;
        } else {
            fs::create_dir_all(output).map_err(malformed)?;
        }
    }
    Ok(())
}
fn push(
    plan: &mut Vec<Entry>,
    names: &mut BTreeSet<PathBuf>,
    entry: Entry,
    name: &str,
) -> Result<(), Error> {
    // Case folding avoids cross-platform collisions; refusing more is safer than
    // having different binaries installed on Windows and Unix.
    let folded = PathBuf::from(entry.path.to_string_lossy().to_lowercase());
    if !names.insert(folded) {
        return Err(refused(name, "duplicate path"));
    }
    if plan.len() >= MAX_ENTRIES {
        return Err(refused(name, "entry count limit"));
    }
    plan.push(entry);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);
    struct Sandbox(PathBuf);
    impl Sandbox {
        fn new() -> Self {
            let t = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "morid-archive-{t}-{}",
                SERIAL.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            fs::create_dir(path.join("target")).unwrap();
            fs::write(path.join("outside"), b"untouched").unwrap();
            Self(path)
        }
    }
    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn synthetic_tar(name: &str, link: bool) -> Vec<u8> {
        let mut builder = tar::Builder::new(Vec::new());
        // Raw header deliberately bypasses Builder's own safe-path checks.
        let mut header = tar::Header::new_gnu();
        header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
        header.set_mode(0o755);
        header.set_size(if link { 0 } else { 3 });
        if link {
            header.set_entry_type(tar::EntryType::Symlink);
            header.set_link_name("../outside").unwrap();
        }
        header.set_cksum();
        builder
            .append(
                &header,
                if link {
                    b"".as_slice()
                } else {
                    b"bin".as_slice()
                },
            )
            .unwrap();
        let tar = builder.into_inner().unwrap();
        let mut gzip = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gzip.write_all(&tar).unwrap();
        gzip.finish().unwrap()
    }
    #[test]
    fn synthetic_tar_four_attacks_refuse_named_entry_without_outside_writes() {
        for (name, link) in [
            ("../outside", false),
            ("/morid-absolute-attack", false),
            ("escape-link", true),
            ("C:\\outside", false),
        ] {
            let root = Sandbox::new();
            let absolute_existed = Path::new(name)
                .is_absolute()
                .then(|| Path::new(name).exists());
            let error = extract(
                &synthetic_tar(name, link),
                Format::TarGz,
                &root.0.join("target"),
            )
            .expect_err("attack");
            assert!(error.to_string().contains(&format!("{name:?}")), "{error}");
            assert_eq!(error.exit_code(), 3);
            if let Some(existed) = absolute_existed {
                assert_eq!(Path::new(name).exists(), existed);
            }
            assert_eq!(fs::read(root.0.join("outside")).unwrap(), b"untouched");
            assert_eq!(fs::read_dir(&root.0).unwrap().count(), 2);
            assert_eq!(fs::read_dir(root.0.join("target")).unwrap().count(), 0);
            println!("SYNTHETIC_TAR_ATTACK exit=3 error={error} outside=unchanged target_files=0");
        }
    }
    #[test]
    fn synthetic_tar_valid_file_extracts() {
        let root = Sandbox::new();
        extract(
            &synthetic_tar("folder/backend", false),
            Format::TarGz,
            &root.0.join("target"),
        )
        .unwrap();
        assert_eq!(
            fs::read(root.0.join("target/folder/backend")).unwrap(),
            b"bin"
        );
        println!("SYNTHETIC_TAR_VALID nested_file=bin outside=unchanged");
    }
    #[test]
    fn synthetic_zip_traversal_and_symlink_are_refused() {
        for name in ["../outside", "/morid-absolute-attack", "C:\\outside"] {
            let root = Sandbox::new();
            let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
            zip.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            zip.write_all(b"bin").unwrap();
            let body = zip.finish().unwrap().into_inner();
            let error = extract(&body, Format::Zip, &root.0.join("target")).expect_err("attack");
            assert!(error.to_string().contains(&format!("{name:?}")));
            assert_eq!(fs::read(root.0.join("outside")).unwrap(), b"untouched");
            assert_eq!(fs::read_dir(root.0.join("target")).unwrap().count(), 0);
            println!("SYNTHETIC_ZIP_ATTACK {error} outside=unchanged target_files=0");
        }
        let root = Sandbox::new();
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        zip.add_symlink(
            "escape-link",
            "../outside",
            zip::write::SimpleFileOptions::default(),
        )
        .unwrap();
        let body = zip.finish().unwrap().into_inner();
        let error = extract(&body, Format::Zip, &root.0.join("target")).expect_err("symlink");
        assert!(error.to_string().contains("escape-link"));
        assert_eq!(fs::read(root.0.join("outside")).unwrap(), b"untouched");
        assert_eq!(fs::read_dir(root.0.join("target")).unwrap().count(), 0);
        println!("SYNTHETIC_ZIP_ATTACK {error} outside=unchanged target_files=0");
    }
}
