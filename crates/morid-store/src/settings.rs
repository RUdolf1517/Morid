//! Selective settings import policy. Unknown values are retained, not activated.
use crate::{
    ini::{Document, Entry},
    value::IniValue,
};

#[derive(Debug, Default)]
pub struct Findings {
    pub unknown: Vec<String>,
    pub dropped: Vec<String>,
    pub warnings: Vec<String>,
}
const DROPPED: &[&str] = &[
    "WindowsUpdateChannel",
    "CheckForUpdates",
    "ShowVersionInfoAndAutoDownloadUpdates",
];
// Fixed keys observed in settings.cpp and the captured Qt document. Dynamic
// engine/tab families are matched separately, never by an arbitrary substring.
const KNOWN: &[&str] = &[
    "YtDlpTextEncoding", // settings.cpp::textEncoding legacy fallback
    "AutoDownload",
    "AutoDownloadWhenAddedInBatchDownloader",
    "AutoHideDownloadWhenCompleted",
    "AutoSavePlaylistOnExit",
    "AutoSetDefaultEngineAndOptions",
    "BackgroundClipboardMonitor",
    "BasicDownloaderDefaultEngine",
    "BasicDownloaderMonitorClipboadUrl",
    "BatchDownloaderDefaultEngine",
    "BatchDownloaderMonitorClipboadUrl",
    "CheckForEnginesUpdates",
    "CommandOnSuccessfulDownload",
    "CommandWhenAllFinished",
    "ConcurrentDownloading",
    "CookieSourceSetToBrowerName",
    "DeleteFilesOnCanceledDownload",
    "DenoEnableAutoDownloading",
    "DesktopNotificationTimeOut",
    "DesktopNotifyOnAllDownloadComplete",
    "DesktopNotifyOnDownloadComplete",
    "DownloadFolder",
    "DownloadOptionsAsLast",
    "EnableLibraryTab",
    "Env",
    "FlatpackUseDenoRuntime",
    "HighDpiScalingFactorInterval",
    "HighDpiScalingFactorValue",
    "HistorySize",
    "Language",
    "LibraryArrangeAscending",
    "LibraryArrangeByDate",
    "LibraryDownloadFolder",
    "LibraryShowFolderFirst",
    "MainTableTextAlignment",
    "MainWindowDimensions",
    "Manual",
    "MaxConcurrentDownloads",
    "MaxLoggerProcesses",
    "MediaDownloaderAppDataPath",
    "NetworkTimeOutInSeconds",
    "OpenWith",
    "PlaylistDownloaderDefaultEngine",
    "PlaylistDownloaderMonitorClipboadUrl",
    "PlaylistDownloaderSaveHistory",
    "ProxySettingsCustomSource",
    "ProxySettingsType",
    "SaveDownloadHistory",
    "SaveHistory",
    "ShowLocalAndLatestVersionInformation",
    "ShowLocalVersionInformationOnly",
    "ShowMetaDataInBatchDownloader",
    "ShowTrayIcon",
    "SingleInstance",
    "StringTruncationSize",
    "TabNumber",
    "ThemeName",
    "TimeOutWaitingForClipboardData",
    "UseInternalArchiveFile",
    "UseSystemEngine",
    "UseSystemProvidedVersionIfAvailable",
    "UseSystemProvidingEngineIfAvailable",
    "UseSystemSupportingEngine",
    "denoInFlatpakUpdated",
];
fn known(key: &str) -> bool {
    KNOWN.contains(&key)
        || [
            "CookieBrowser_",
            "CookieTextFilePath_",
            "TextEncoding_",
            "LastUsedOptionBasicTab_",
            "LastUsedOptionBatchTab_",
            "LastUsedOptionPlayListTab_",
            "WindowDimensions_",
            "ThumbnailHeight_",
            "ThumbnailWidth_",
            "BasicDownloaderOptionsHistory-",
            "BatchDownloaderOptionsHistory-",
            "PlaylistDownloaderOptionsHistory-",
            "PlaylistRangeHistory-",
            "PlaylistUrlHistory-",
            "playlistRangeHistoryLastUsed-",
        ]
        .iter()
        .any(|prefix| {
            key.strip_prefix(prefix)
                .is_some_and(|suffix| !suffix.is_empty())
        })
}

pub fn import(document: &Document) -> (Document, Findings) {
    let mut findings = Findings::default();
    let entries = document
        .entries()
        .filter_map(|entry| {
            if entry.section.eq_ignore_ascii_case("General")
                && DROPPED.contains(&entry.key.as_str())
            {
                findings.dropped.push(entry.key.clone());
                return None;
            }
            let mut entry = entry.clone();
            if !entry.section.eq_ignore_ascii_case("General") {
                findings
                    .unknown
                    .push(format!("{}/{}", entry.section, entry.key));
                entry.section = format!("imported/{}", entry.section);
            } else if !known(&entry.key) {
                findings.unknown.push(entry.key.clone());
                entry.section = "imported".into();
            } else if entry.section.eq_ignore_ascii_case("General")
                && entry.key == "MaxConcurrentDownloads"
            {
                if let IniValue::String(raw) = &entry.value {
                    if let Ok(value) = raw.parse::<i64>() {
                        let clamped = value.clamp(1, 64);
                        if value != clamped {
                            findings.warnings.push(format!(
                                "MaxConcurrentDownloads: {value} clamped to {clamped}"
                            ));
                            entry.value = IniValue::String(clamped.to_string());
                        }
                    } else {
                        findings
                            .warnings
                            .push(format!("MaxConcurrentDownloads: invalid number {raw:?}"));
                    }
                } else {
                    findings
                        .warnings
                        .push("MaxConcurrentDownloads: list is not a number".into());
                }
            }
            Some(entry)
        })
        .collect();
    (Document::from_entries(entries), findings)
}

pub fn export(document: &Document) -> Document {
    Document::from_entries(
        document
            .entries()
            .map(|entry| {
                let mut entry: Entry = entry.clone();
                if entry.section == "imported" {
                    entry.section = "General".into();
                } else if let Some(original) = entry.section.strip_prefix("imported/") {
                    entry.section = original.to_owned();
                }
                entry
            })
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unknown_settings_keys_are_preserved_and_reported() {
        let input = Document::parse(b"[General]\nNovelKey=one, two\n").expect("input");
        let (stored, report) = import(&input);
        assert_eq!(report.unknown, ["NovelKey"]);
        let entry = stored.entries().next().expect("preserved");
        assert_eq!(entry.section, "imported");
        assert_eq!(entry.value, input.entries().next().expect("input").value);
        assert_eq!(export(&stored).encode(), input.encode());
    }
    #[test]
    fn unknown_groups_are_quarantined_without_namespace_collisions() {
        let input = Document::parse(b"[General]\nNovelKey=\"one, two\"\n[Plugin]\nDownloadFolder=not-active\n[imported]\nCheckForUpdates=plugin-data\n").expect("input");
        let (stored, report) = import(&input);
        assert_eq!(report.unknown.len(), 3);
        assert!(report.dropped.is_empty());
        assert!(stored
            .entries()
            .all(|entry| entry.section.starts_with("imported")));
        assert_eq!(export(&stored).encode(), input.encode());
    }

    #[test]
    fn update_settings_are_dropped_with_a_notice() {
        let input = Document::parse(b"[General]\nCheckForUpdates=true\nWindowsUpdateChannel=release\nShowVersionInfoAndAutoDownloadUpdates=true\n").expect("input");
        let (stored, report) = import(&input);
        assert_eq!(stored.entries().count(), 0);
        assert_eq!(report.dropped.len(), 3);
    }
    #[test]
    fn concurrency_out_of_range_is_clamped_and_reported() {
        for (value, expected) in [(0, "1"), (-1, "1"), (65, "64")] {
            let input =
                Document::parse(format!("[General]\nMaxConcurrentDownloads={value}\n").as_bytes())
                    .expect("input");
            let (stored, report) = import(&input);
            assert_eq!(
                stored.entries().next().expect("setting").value,
                IniValue::String(expected.into())
            );
            assert_eq!(report.warnings.len(), 1);
        }
    }
}
