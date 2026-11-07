# User data: formats and import

Related: [extensions](extensions.md) · [decisions](decisions.md) D-003 · [audit](audit-original.md) §2.10 ·
[architecture](architecture.md) §7

Morid replaces Media Downloader, which means an existing user must not lose anything: download folder, presets,
URL rules, per-engine defaults, subscriptions, the "already downloaded" archive, history, cookies, themes and
window geometry. This document describes where the original keeps that data, in which format, and what Morid
does with it.

The format descriptions are derived from `legacy/src/settings.cpp`, `configure.cpp`, `playlistdownloader.cpp`,
`utility.cpp` and `engines.h`. Where the original's behaviour is ambiguous, the ambiguity is stated rather than
guessed.

---

## 1. Where the original keeps its data

| Item | Path |
| --- | --- |
| Settings | `<appdata>/media-downloader/settings/settings.ini` (INI via `QSettings`) |
| Data directory | `<appdata>/media-downloader/` |
| Binaries of backends | `<data>/bin` |
| Temporary files | `<data>/tmp` |
| Themes | `<data>/themes` |
| IPC socket | `<data>/tmp/ipc` (Unix), `\\.\pipe\MediaDownloaderIPC` (Windows) |

`<appdata>` is `%APPDATA%`-equivalent from `QStandardPaths::AppDataLocation` with `/media-downloader/` appended,
and can be overridden by the environment variable **`MediaDownloaderAppDataPath`**
(`legacy/src/settings.cpp:423`). That variable is part of the compatibility surface.

Portable mode on Windows keeps everything beside the executable (`--portable`).

---

## 2. Data files

| File | Purpose | Format | Import |
| --- | --- | --- | --- |
| `presetOptions.json` | 17 bundled download presets plus user additions | JSON array of `{uiName, options, website}` | full |
| `downloadDefaultOptions.json` | URL manager: substring → engine + options | JSON array of `{url, engineName, options, inUse}` | full |
| `downloadEngineDefaultOptions.json` | default options per engine | JSON array of `{engineName, options, inUse}` | full |
| `subscriptions.json` | playlist subscriptions | JSON array of `{uiName, url, options}` | full |
| `autoSavedList.json` | batch/playlist list saved on exit | JSON array of table entries (`url`, `title`, `options`, …) | full |
| `downloadHistory.json` | download history | concatenated JSON objects, **not** a JSON array | full, with repair (§5) |
| `archiveFile-<engine>.txt` | yt-dlp-style "already downloaded" archive | one id per line, `youtube <id>` | **not rewritten** |
| `archiveFile-gallery-dl.db` | gallery-dl archive | SQLite database owned by gallery-dl | **not touched at all** |
| `themes/*.json` | theme colour definitions | JSON object of colour values | full |
| `settings.ini` | all settings | INI | selective, §3 |

Two entries deserve emphasis:

- **The archive file is not ours.** `yt-dlp` and `gallery-dl` write it and read it; Morid only passes
  `--download-archive <path>`. It is never parsed for logic beyond the two flags of F-24 and never rewritten.
  A reimplementation that "normalises" it would silently make users re-download everything.
- **`archiveFile-gallery-dl.db` is SQLite.** Morid does not read it. Copying it and letting gallery-dl use it is
  the correct behaviour.

---

## 3. Settings

### 3.0 Encoding — established empirically in Phase 0

The original reads `settings.ini` through `QSettings` with **no `setIniCodec` call anywhere in the source**, so
Qt uses the locale codec, not UTF-8. Observed during Phase 0: a `DownloadFolder` containing Cyrillic did not
survive the round trip under a `POSIX` locale, the original treated the path as non-existent and reset the
folder to `QStandardPaths::DownloadLocation` (`legacy/src/settings.cpp:688-728`), logging "Resetting download
folder to default".

What Morid does, and why it is a decision rather than a detail:

- **Read as UTF-8, unconditionally.** A user whose settings file contains a non-ASCII path should not lose it.
  This is a deliberate difference from the original ([migration](migration.md) §5, M11).
- **Write as UTF-8.** Writing in the locale codec would make the output unreadable on another machine and to
  the original on a third.
- **The difference is invisible for pure-ASCII settings**, which is why it went unnoticed for the original's
  whole life.
- **Import is affected.** [§4](#4-the-import-procedure) must not assume the file was written by a UTF-8
  program. A file written by the original under a non-UTF-8 locale may contain bytes that are not valid UTF-8;
  the importer decodes strictly and reports rather than substituting replacement characters.

The JSON data files are UTF-8 by construction. `archiveFile-<engine>.txt` is written by the backend, not by the
original, and is passed through byte for byte.

---

### 3.1 Keys

`settings.ini` holds ~51 distinct keys plus dynamically named families
(`CookieBrowser_<engine>`, `CookieTextFilePath_<engine>`, `LastUsedOption<tab>_<engine>`,
`TextEncoding_<engine>`, `WindowDimensions_<tab>`). The full list extracted from `legacy/src/settings.cpp` is:

```
AutoDownload                              AutoDownloadWhenAddedInBatchDownloader
AutoHideDownloadWhenCompleted             AutoSavePlaylistOnExit
AutoSetDefaultEngineAndOptions            BackgroundClipboardMonitor
BasicDownloaderDefaultEngine              BasicDownloaderMonitorClipboadUrl
BatchDownloaderDefaultEngine              BatchDownloaderMonitorClipboadUrl
PlaylistDownloaderDefaultEngine           PlaylistDownloaderMonitorClipboadUrl
BasicDownloaderOptionsHistory             BatchDownloaderOptionsHistory
PlaylistDownloaderOptionsHistory          PlaylistDownloaderSaveHistory
CheckForEnginesUpdates                    CheckForUpdates
CommandOnSuccessfulDownload               CommandWhenAllFinished
ConcurrentDownloading                     MaxConcurrentDownloads
CookieBrowser_<engine>                    CookieTextFilePath_<engine>
CookieSourceSetToBrowerName               DeleteFilesOnCanceledDownload
DenoEnableAutoDownloading                 DesktopNotificationTimeOut
DesktopNotifyOnAllDownloadComplete        DesktopNotifyOnDownloadComplete
DownloadFolder                            DownloadOptionsAsLast
EnableLibraryTab                          Env
FlatpackUseDenoRuntime                    HighDpiScalingFactorInterval
HighDpiScalingFactorValue                 HistorySize
Language                                  LastUsedOption<tab>_<engine>
LibraryArrangeAscending                   LibraryArrangeByDate
LibraryDownloadFolder                     LibraryShowFolderFirst
MainTableTextAlignment                    MainWindowDimensions
Manual                                    MaxLoggerProcesses
MediaDownloaderAppDataPath                NetworkTimeOutInSeconds
OpenWith                                  PlaylistDownloaderSaveHistory
PlaylistRangeHistory                      PlaylistUrlHistory
ProxySettingsCustomSource                 ProxySettingsType
SaveDownloadHistory                       SaveHistory
ShowLocalAndLatestVersionInformation      ShowLocalVersionInformationOnly
ShowMetaDataInBatchDownloader             ShowTrayIcon
ShowVersionInfoAndAutoDownloadUpdates     SingleInstance
StringTruncationSize                      TabNumber
TextEncoding_<engine>                     ThemeName
ThumbnailHeight                           ThumbnailWidth
TimeOutWaitingForClipboardData            UseInternalArchiveFile
UseSystemEngine                           UseSystemProvidingEngineIfAvailable
WindowsUpdateChannel                      WindowDimensions_<tab>
```

Mapping rules for the import:

| Original key | Morid | Rule |
| --- | --- | --- |
| `DownloadFolder`, `LibraryDownloadFolder` | same meaning | copied |
| `MaxConcurrentDownloads`, `ConcurrentDownloading` | scheduler | copied; values outside 1–64 clamped with a report |
| `ProxySettingsType` (`None`/`System`/`Env`/`Manual`), `ProxySettingsCustomSource` | proxy mode | copied, string values preserved verbatim |
| `Language` | interface language | copied if the locale still exists, otherwise falls back with a notice |
| `ThemeName` | theme | copied |
| `CookieBrowser_<engine>`, `CookieTextFilePath_<engine>` | same meaning | copied, key shape preserved |
| `TextEncoding_<engine>`, `LastUsedOption<tab>_<engine>` | same meaning | copied, key shape preserved |
| `WindowDimensions_<tab>`, `MainWindowDimensions` | window geometry | copied if parseable, otherwise ignored with a notice |
| `WindowsUpdateChannel`, `CheckForUpdates`, `ShowVersionInfoAndAutoDownloadUpdates` | **not imported** | self-update is not carried over ([D-013](decisions.md)); the user is told the setting was dropped |
| `SingleInstance` | same meaning | copied |
| `DenoEnableAutoDownloading`, `FlatpackUseDenoRuntime` | supporting-engine policy | copied where it still applies |
| unknown key | preserved | stored untouched in an `[imported]`-prefixed section, never used, never deleted |

Preservation of unknown keys is not sentimentality: a user who returns to the original program after trying
Morid must find their settings intact, and a future Morid version may learn to use a key that an older one
ignored.

---

## 4. The import procedure

```
morid import <path-to-media-downloader-data-dir>          (also: GUI — Configure → Import)
  1. verify the directory looks like a Media Downloader data directory (settings/settings.ini or json files)
  2. copy, never move; the source directory is left byte-identical
  3. write into <appdata>/morid/ with the same file names and structures
  4. produce a report: imported / skipped / unknown keys / warnings
  5. never overwrite an existing Morid file unless --force is given; with --force, back up first
```

Rules:

- **Idempotent.** Running it twice produces the same result and does not duplicate presets or subscriptions.
- **Non-destructive.** The source is opened read-only. If the copy fails halfway, the half-written Morid files
  are removed and the previous state is kept.
- **Reported.** The user sees what was imported and what was not, including every dropped key.
- **Tested against a real directory.** The fixture in the test suite is a captured, anonymised data directory
  from the original program; the acceptance criterion (F-68) is that nothing is lost.

---

## 5. Files whose format is questionable

### 5.1 `downloadHistory.json`

The original appends a pretty-printed JSON object per download to one file, so the result is **not** valid JSON
(several concatenated objects). Reading it back is done by substring search for a URL
(`legacy/src/utility.cpp:2470-2520`).

Morid's approach:

- **read** the file tolerantly: parse a stream of concatenated JSON values, skipping junk between them;
- if a fragment cannot be parsed, keep the raw text in the report and do not lose the rest;
- **write** a proper JSON array under the Morid directory, and keep the original file untouched so the user can
  go back;
- document the difference, because a user comparing the two files will notice.

### 5.2 `autoSavedList.json`

Written on exit when `AutoSavePlaylistOnExit` is set. Its entries carry the whole table row, including fields
that only make sense in the original's UI (`banner`, `showFirst`, `runningRecursively`). Morid reads the
meaningful fields and ignores the rest; it does not attempt to restore "running" state from a saved file — a
download that was running when the program closed is restored as *not started*.

### 5.3 Presets and URL rules

Plain JSON arrays, but with an ambiguity worth stating: the original records a preset as a combination of
`uiName`, `options` and `website`, and the "website" is only used for grouping in the UI. A user can have
duplicate `uiName` values. Morid preserves duplicates rather than de-duplicating, because removing an entry a
user created is worse than showing a duplicate.

---

## 6. Export and going back

Two-way compatibility is a promise, and it is testable:

- files Morid writes that overlap with the original (`presetOptions.json`, `downloadDefaultOptions.json`,
  `downloadEngineDefaultOptions.json`, `subscriptions.json`, `autoSavedList.json`, `themes/*.json`) keep the
  original's structure, so the original program can read them;
- `morid export <dir>` produces a directory the original can be pointed at (a documented copy, not a
  conversion);
- the *original* directory is never modified by Morid, in any operation. Import copies, export copies, and
  nothing writes outside `<appdata>/morid/` except the user's chosen download folder.

The acceptance test for this section is a round trip: original data → import → export → run the original on the
exported directory → settings and lists intact.
