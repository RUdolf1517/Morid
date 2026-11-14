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
                format!("{}: symlink in data tree refused", entry.path().display()),
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
                format!(
                    "{}: special file in data tree refused",
                    entry.path().display()
                ),
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
fn clone_tree(source: &Path, destination: &Path) -> io::Result<()> {
    fs::create_dir_all(destination)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let kind = entry.file_type()?;
        let target = destination.join(entry.file_name());
        if kind.is_dir() {
            clone_tree(&entry.path(), &target)?;
        } else if kind.is_file() {
            fs::copy(entry.path(), target)?;
        } else {
            return Err(io::Error::other(
                "special file or symlink in destination refused",
            ));
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CopyPoint {
    StagedFile(usize),
    BeforePublish,
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
    copy_with_checkpoints(
        source,
        destination,
        force,
        |path, bytes, report| Ok(vec![(path.to_owned(), transform(path, bytes, report)?)]),
        |_| Ok(()),
    )
}

// Private deterministic fault seam. Production has no environment variable or
// command-line switch capable of injecting migration failures.
fn copy_with_checkpoints<F, C>(
    source: &Path,
    destination: &Path,
    force: bool,
    mut transform: F,
    mut checkpoint: C,
) -> io::Result<Report>
where
    F: FnMut(&Path, Vec<u8>, &mut Report) -> io::Result<Vec<(PathBuf, Vec<u8>)>>,
    C: FnMut(CopyPoint) -> io::Result<()>,
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
    let mut planned_paths = std::collections::BTreeSet::new();
    for input in files {
        let output = transform(&input, fs::read(source.join(&input))?, &mut report)?;
        for (relative, bytes) in output {
            if relative.as_os_str().is_empty()
                || relative
                    .components()
                    .any(|part| !matches!(part, std::path::Component::Normal(_)))
            {
                return Err(io::Error::other("invalid planned output path"));
            }
            if !planned_paths.insert(relative.clone()) {
                return Err(io::Error::other("duplicate planned output path"));
            }
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
            clone_tree(&destination, &stage)?;
        }
        for (index, (relative, bytes)) in changes.iter().enumerate() {
            let target = stage.join(relative);
            if let Some(dir) = target.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(target, bytes)?;
            checkpoint(CopyPoint::StagedFile(index))?;
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
        if let Err(error) =
            checkpoint(CopyPoint::BeforePublish).and_then(|_| fs::rename(&stage, &destination))
        {
            if let Some(backup) = &backup {
                fs::rename(backup, &destination).map_err(|rollback| {
                    io::Error::other(format!(
                        "publish: {error}; rollback: {rollback}; previous tree at {}",
                        backup.display()
                    ))
                })?;
                if let Some(container) = backup.parent() {
                    fs::remove_dir(container)?;
                }
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

// Morid-only original backing is not part of the directory exported to legacy.
const ORIGINAL_HISTORY: &str = ".morid/original-downloadHistory.json";
const HISTORY: &str = "data/downloadHistory.json";

pub fn import(source: &Path, destination: &Path, force: bool) -> io::Result<Report> {
    copy_with_checkpoints(
        source,
        destination,
        force,
        |path, bytes, report| {
            if path.starts_with(".morid") {
                return Err(io::Error::other(
                    "reserved .morid metadata in import source; no source bytes changed",
                ));
            }
            if path == Path::new("settings/settings.ini") {
                let doc = crate::ini::Document::parse(&bytes).map_err(io::Error::other)?;
                let (doc, findings) = crate::settings::import(&doc);
                report.unknown_keys.extend(findings.unknown);
                report.dropped_keys.extend(findings.dropped);
                report.warnings.extend(findings.warnings);
                return Ok(vec![(path.to_owned(), doc.encode())]);
            }
            use crate::data;
            if path == Path::new(HISTORY) {
                let (rows, fragments) = data::read_history(&bytes);
                for fragment in fragments {
                    report.warnings.push(format!("history corrupt fragment at byte {}; raw bytes preserved in original backing", fragment.offset));
                    report.history_fragments.push(HistoryFinding {
                        offset: fragment.offset,
                        bytes: fragment.bytes,
                    });
                }
                let normalized = data::write_history(&rows).map_err(io::Error::other)?;
                return Ok(vec![
                    (path.to_owned(), normalized),
                    (ORIGINAL_HISTORY.into(), bytes),
                ]);
            }
            let data_name = path.strip_prefix("data").ok().and_then(Path::to_str);
            let archive = path.parent() == Some(Path::new("data"))
                && path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .is_some_and(|name| name.starts_with("archiveFile-"));
            let theme = path.parent() == Some(Path::new("data/themes"))
                && path.extension().is_some_and(|ext| ext == "json");
            if !archive
                && !theme
                && !matches!(
                    data_name,
                    Some(
                        "presetOptions.json"
                            | "downloadDefaultOptions.json"
                            | "downloadEngineDefaultOptions.json"
                            | "subscriptions.json"
                            | "autoSavedList.json"
                    )
                )
            {
                report.unknown_items.push(path.display().to_string());
            }
            if theme {
                if let Err(error) = data::Theme::read(&bytes) {
                    report.warnings.push(format!(
                        "{}: {error}; original bytes preserved",
                        path.display()
                    ));
                }
            }
            let validation = match data_name {
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
            Ok(vec![(path.to_owned(), bytes)])
        },
        |_| Ok(()),
    )
}

pub fn export(source: &Path, destination: &Path, force: bool) -> io::Result<Report> {
    copy_with_checkpoints(
        source,
        destination,
        force,
        |path, bytes, _| {
            if path.starts_with(".morid") {
                return Ok(Vec::new());
            }
            let bytes = if path == Path::new("settings/settings.ini") {
                crate::settings::export(
                    &crate::ini::Document::parse(&bytes).map_err(io::Error::other)?,
                )
                .encode()
            } else if path == Path::new(HISTORY) {
                match fs::read(source.join(ORIGINAL_HISTORY)) {
                    Ok(original) => {
                        let (rows, _) = crate::data::read_history(&original);
                        // Unchanged imported history returns its exact original bytes.
                        // If Morid has edited history, copy the current array as-is;
                        // never discard new entries in favor of an old backing file.
                        if crate::data::write_history(&rows).map_err(io::Error::other)? == bytes {
                            original
                        } else {
                            bytes
                        }
                    }
                    Err(e) if e.kind() == io::ErrorKind::NotFound => bytes,
                    Err(e) => return Err(e),
                }
            } else {
                bytes
            };
            Ok(vec![(path.to_owned(), bytes)])
        },
        |_| Ok(()),
    )
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
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/userdata/media-downloader");
        let before = snapshot(&source);
        let temp = Temp::new();
        import(&source, &temp.0.join("morid"), false).expect("import");
        assert_eq!(snapshot(&source), before);
    }
    #[test]
    fn import_is_idempotent() {
        let source =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/userdata/media-downloader");
        let temp = Temp::new();
        let destination = temp.0.join("morid");
        import(&source, &destination, false).expect("first");
        let before = snapshot(&destination);
        let report = import(&source, &destination, false).expect("second");
        assert!(report.imported.is_empty());
        assert_eq!(snapshot(&destination), before);
    }
    #[test]
    fn failed_copy_after_staging_or_backup_restores_previous_tree() {
        for existing in [false, true] {
            for fail_at in [
                CopyPoint::StagedFile(0),
                CopyPoint::StagedFile(1),
                CopyPoint::BeforePublish,
            ] {
                let temp = Temp::new();
                let source = temp.0.join("source");
                let destination = temp.0.join("dest");
                fs::create_dir(&source).expect("source");
                fs::write(source.join("a"), b"new-a").expect("a");
                fs::write(source.join("b"), b"new-b").expect("b");
                if existing {
                    fs::create_dir_all(destination.join("empty-child")).expect("empty directory");
                    fs::write(destination.join("a"), b"old-a").expect("old");
                }
                let before = snapshot(&temp.0);
                let mut before_children = fs::read_dir(&temp.0)
                    .expect("before")
                    .map(|e| e.expect("entry").file_name())
                    .collect::<Vec<_>>();
                before_children.sort();
                let mut reached = false;
                let result = copy_with_checkpoints(
                    &source,
                    &destination,
                    true,
                    |path, bytes, _| Ok(vec![(path.to_owned(), bytes)]),
                    |point| {
                        if point == fail_at {
                            reached = true;
                            Err(io::Error::other("injected write/publication failure"))
                        } else {
                            Ok(())
                        }
                    },
                );
                assert!(reached, "fault seam not reached: {fail_at:?}");
                assert!(result.is_err());
                assert_eq!(snapshot(&temp.0), before);
                assert_eq!(destination.exists(), existing);
                if existing {
                    assert!(destination.join("empty-child").is_dir());
                }
                let mut after_children = fs::read_dir(&temp.0)
                    .expect("after")
                    .map(|e| e.expect("entry").file_name())
                    .collect::<Vec<_>>();
                after_children.sort();
                assert_eq!(
                    after_children, before_children,
                    "no staging or backup residue"
                );
            }
        }
    }

    #[test]
    fn successful_force_preserves_unrelated_empty_directories() {
        let temp = Temp::new();
        let source = temp.0.join("source");
        let destination = temp.0.join("dest");
        fs::create_dir(&source).expect("source");
        fs::write(source.join("a"), b"new").expect("new");
        fs::create_dir_all(destination.join("empty/nested")).expect("empty");
        fs::write(destination.join("a"), b"old").expect("old");
        let report =
            copy_directory(&source, &destination, true, |_, bytes, _| Ok(bytes)).expect("publish");
        assert!(destination.join("empty/nested").is_dir());
        assert!(report.backup.expect("backup").join("empty/nested").is_dir());
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
