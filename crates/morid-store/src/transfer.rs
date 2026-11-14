//! Transactional directory copies. No archive is parsed or normalized here.
use serde::Serialize;
use std::{
    fs, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Serialize)]
pub struct HistoryFinding {
    pub offset: usize,
    pub bytes: Vec<u8>,
}

#[derive(Debug, Default, Serialize)]
pub struct Report {
    pub imported: Vec<String>,
    pub skipped: Vec<String>,
    pub unchanged: Vec<String>,
    pub warnings: Vec<String>,
    pub unknown_keys: Vec<String>,
    pub unknown_items: Vec<String>,
    pub history_fragments: Vec<HistoryFinding>,
    pub dropped_keys: Vec<String>,
    pub backup: Option<PathBuf>,
}
impl Report {
    pub fn code(&self) -> u8 {
        if self.skipped.is_empty() && self.warnings.is_empty() && self.dropped_keys.is_empty() {
            0
        } else {
            8
        }
    }
}
fn collect(root: &Path, dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
    let mut entries = fs::read_dir(dir)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "symlink in data tree refused",
            ));
        }
        if kind.is_dir() {
            collect(root, &entry.path(), files)?;
        } else if kind.is_file() {
            files.push(
                entry
                    .path()
                    .strip_prefix(root)
                    .map_err(io::Error::other)?
                    .to_owned(),
            );
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "special file in data tree refused",
            ));
        }
    }
    Ok(())
}
fn absolute(path: &Path) -> io::Result<PathBuf> {
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    };
    if path
        .components()
        .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "parent traversal refused",
        ));
    }
    let mut ancestor = path.as_path();
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or_else(|| io::Error::other("no existing ancestor"))?;
    }
    Ok(ancestor
        .canonicalize()?
        .join(path.strip_prefix(ancestor).map_err(io::Error::other)?))
}
/// Stage a complete tree beside destination; publish only after every write
/// succeeds. Existing destination moves to a backup before the staged rename.
/// The caller's transform is pure; all reads/validation precede publication.
pub fn copy_directory<F>(
    source: &Path,
    destination: &Path,
    force: bool,
    mut transform: F,
) -> io::Result<Report>
where
    F: FnMut(&Path, Vec<u8>, &mut Report) -> io::Result<Vec<u8>>,
{
    let source = source.canonicalize()?;
    let destination = absolute(destination)?;
    if source.starts_with(&destination) || destination.starts_with(&source) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "source and destination overlap",
        ));
    }
    let mut files = Vec::new();
    collect(&source, &source, &mut files)?;
    let mut report = Report::default();
    let mut changes = Vec::new();
    for relative in files {
        let bytes = transform(&relative, fs::read(source.join(&relative))?, &mut report)?;
        let target = destination.join(&relative);
        let old = match fs::read(&target) {
            Ok(data) => Some(data),
            Err(e) if e.kind() == io::ErrorKind::NotFound => None,
            Err(e) => return Err(e),
        };
        if old.as_ref() == Some(&bytes) {
            report.unchanged.push(relative.display().to_string());
            continue;
        }
        if old.is_some() && !force {
            report.skipped.push(format!(
                "{}: different existing content; use --force",
                relative.display()
            ));
        } else {
            changes.push((relative, bytes));
        }
    }
    if changes.is_empty() {
        return Ok(report);
    }
    let parent = destination
        .parent()
        .ok_or_else(|| io::Error::other("destination has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut serial = 0u64;
    let stage = loop {
        let name = parent.join(format!(
            ".morid-transfer-{}-{serial}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(io::Error::other)?
                .as_nanos()
        ));
        match fs::create_dir(&name) {
            Ok(()) => break name,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => serial += 1,
            Err(e) => return Err(e),
        }
    };
    let result = (|| {
        if destination.exists() {
            let mut old_files = Vec::new();
            collect(&destination, &destination, &mut old_files)?;
            for relative in old_files {
                let target = stage.join(&relative);
                if let Some(dir) = target.parent() {
                    fs::create_dir_all(dir)?;
                }
                fs::copy(destination.join(relative), target)?;
            }
        }
        for (relative, bytes) in &changes {
            let target = stage.join(relative);
            if let Some(dir) = target.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(target, bytes)?;
        }
        let backup = if destination.exists() {
            let mut number = 0u64;
            let backup = loop {
                let name = destination.with_file_name(format!(
                    "{}.morid-backup-{number}",
                    destination
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                ));
                match fs::create_dir(&name) {
                    Ok(()) => break name,
                    Err(error) if error.kind() == io::ErrorKind::AlreadyExists => number += 1,
                    Err(error) => return Err(error),
                }
            };
            let previous = backup.join("previous");
            if let Err(error) = fs::rename(&destination, &previous) {
                fs::remove_dir(&backup)?;
                return Err(error);
            }
            let backup = previous;
            Some(backup)
        } else {
            None
        };
        if let Err(error) = fs::rename(&stage, &destination) {
            if let Some(backup) = &backup {
                fs::rename(backup, &destination).map_err(|rollback| {
                    io::Error::other(format!(
                        "publish: {error}; rollback: {rollback}; previous tree at {}",
                        backup.display()
                    ))
                })?;
            }
            return Err(error);
        }
        report.backup = backup;
        report.imported.extend(
            changes
                .iter()
                .map(|(relative, _)| relative.display().to_string()),
        );
        Ok(())
    })();
    if stage.exists() {
        fs::remove_dir_all(&stage)?;
    }
    result?;
    Ok(report)
}

pub fn recognized(path: &Path) -> bool {
    path.join("settings/settings.ini").is_file()
        || [
            "presetOptions.json",
            "downloadDefaultOptions.json",
            "downloadEngineDefaultOptions.json",
            "subscriptions.json",
            "autoSavedList.json",
            "downloadHistory.json",
        ]
        .iter()
        .any(|file| path.join("data").join(file).is_file())
}

pub fn import(source: &Path, destination: &Path, force: bool) -> io::Result<Report> {
    copy_directory(source, destination, force, |path, bytes, report| {
        if path == Path::new("settings/settings.ini") {
            let doc = crate::ini::Document::parse(&bytes).map_err(io::Error::other)?;
            let (doc, findings) = crate::settings::import(&doc);
            report.unknown_keys.extend(findings.unknown);
            report.dropped_keys.extend(findings.dropped);
            report.warnings.extend(findings.warnings);
            Ok(doc.encode())
        } else {
            use crate::data;
            let validation = match path.strip_prefix("data").ok().and_then(Path::to_str) {
                Some("presetOptions.json") => data::read_presets(&bytes).map(|_| ()),
                Some("downloadDefaultOptions.json") => data::read_url_rules(&bytes).map(|_| ()),
                Some("downloadEngineDefaultOptions.json") => {
                    data::read_engine_defaults(&bytes).map(|_| ())
                }
                Some("subscriptions.json") => data::read_subscriptions(&bytes).map(|_| ()),
                Some("autoSavedList.json") => data::read_saved_list(&bytes).map(|_| ()),
                _ => Ok(()),
            };
            if let Err(error) = validation {
                report.warnings.push(format!(
                    "{}: {error}; original bytes preserved",
                    path.display()
                ));
            }
            if path == Path::new("data/downloadHistory.json") {
                let (_, fragments) = data::read_history(&bytes);
                for fragment in fragments {
                    report.warnings.push(format!(
                        "history corrupt fragment at byte {}; original bytes preserved",
                        fragment.offset
                    ));
                    report.history_fragments.push(HistoryFinding {
                        offset: fragment.offset,
                        bytes: fragment.bytes,
                    });
                }
            }
            Ok(bytes)
        }
    })
}
pub fn export(source: &Path, destination: &Path, force: bool) -> io::Result<Report> {
    copy_directory(source, destination, force, |path, bytes, _| {
        if path == Path::new("settings/settings.ini") {
            Ok(crate::settings::export(
                &crate::ini::Document::parse(&bytes).map_err(io::Error::other)?,
            )
            .encode())
        } else {
            Ok(bytes)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temp(PathBuf);
    impl Temp {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "morid-transfer-test-{}-{}",
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .expect("clock")
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).expect("temp");
            Self(path)
        }
    }
    impl Drop for Temp {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn snapshot(root: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut files = Vec::new();
        collect(root, root, &mut files).expect("walk");
        files
            .into_iter()
            .map(|file| {
                let bytes = fs::read(root.join(&file)).expect("read");
                (file, bytes)
            })
            .collect()
    }
    #[test]
    fn import_leaves_the_source_byte_identical() {
        let source = Path::new(file!())
            .parent()
            .expect("source")
            .join("../../../fixtures/userdata/media-downloader");
        let before = snapshot(&source);
        let temp = Temp::new();
        import(&source, &temp.0.join("morid"), false).expect("import");
        assert_eq!(snapshot(&source), before);
    }
    #[test]
    fn import_is_idempotent() {
        let source = Path::new(file!())
            .parent()
            .expect("source")
            .join("../../../fixtures/userdata/media-downloader");
        let temp = Temp::new();
        let destination = temp.0.join("morid");
        import(&source, &destination, false).expect("first");
        let before = snapshot(&destination);
        let report = import(&source, &destination, false).expect("second");
        assert!(report.imported.is_empty());
        assert_eq!(snapshot(&destination), before);
    }
    #[test]
    fn force_backs_up_and_failure_does_not_publish() {
        let temp = Temp::new();
        let source = temp.0.join("source");
        let destination = temp.0.join("dest");
        fs::create_dir(&source).expect("source");
        fs::create_dir(&destination).expect("dest");
        fs::write(source.join("a"), b"new").expect("new");
        fs::write(destination.join("a"), b"old").expect("old");
        let report =
            copy_directory(&source, &destination, false, |_, b, _| Ok(b)).expect("conflict");
        assert_eq!(report.code(), 8);
        assert_eq!(fs::read(destination.join("a")).expect("old"), b"old");
        assert!(copy_directory(&source, &destination, true, |_, _, _| Err(
            io::Error::other("injected")
        ))
        .is_err());
        assert_eq!(fs::read(destination.join("a")).expect("old"), b"old");
        let report = copy_directory(&source, &destination, true, |_, b, _| Ok(b)).expect("force");
        assert_eq!(
            fs::read(report.backup.expect("backup").join("a")).expect("backup bytes"),
            b"old"
        );
        assert_eq!(fs::read(destination.join("a")).expect("new"), b"new");
    }
}
