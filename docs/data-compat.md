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
| `presetOptions.json` | bundled download presets plus user additions | JSON array of `{options, uiName, website}` | full |
| `downloadDefaultOptions.json` | URL manager: pattern → engine + options | JSON array of `{downloadOption, engine, url}` | full |
| `downloadEngineDefaultOptions.json` | default options per engine | JSON array of `{default, engineName, options}`, `default` is `"yes"`/`"no"` | full |
| `subscriptions.json` | playlist subscriptions | JSON array of `{getListOptions, uiName, url}` | full |
| `autoSavedList.json` | batch/playlist list saved on exit | JSON array of table-entry objects; the key set varies per entry (`url` always, plus `title`, `id`, `duration`, `intDuration`, `upload_date`, `uploader`, `downloadOptions`, `engineName`, `downloadExtraOptions`, …) | full |
| `downloadHistory.json` | download history | concatenated JSON objects, **not** a JSON array | full, with repair (§5) |
| `archiveFile-<engine>.txt` | yt-dlp-style "already downloaded" archive | one id per line, `youtube <id>` | **not rewritten** |
| `archiveFile-gallery-dl.db` | gallery-dl archive | SQLite database owned by gallery-dl | **not touched at all** |
| `themes/*.json` | theme colour definitions | JSON object of colour values | full |
| `settings.ini` | all settings | INI | selective, §3 |

The field names above are read from the captured directory in `fixtures/userdata/`, not from the original's
headers, because the headers disagree with the files: `downloadDefaultOptions` declares `inUse` and writes
`default`, and the URL rule is stored under `downloadOption`, not `options`. Where this document and a fixture
differ, **the fixture wins** — it is what the original wrote.

**Three of these files have a rule that is not visible in the bytes**, and each is the kind of thing a reader
implemented from the file alone gets silently wrong:

1. **Presets are 15, not 17,** and the shipped file is *not* the constant in the source.
   `configure::presetOptions::defaultData()` (`legacy/src/configure.cpp:1913`) holds 15 entries whose YouTube
   resolutions are written `bestvideo[height=144]`; the constructor then rewrites every one of `144…2160` to
   `bestvideo[format_note*=144p]` and rewrites one long format string for a second time
   (`legacy/src/configure.cpp:1805-1839`). The captured `presetOptions.json` is the **post-rewrite** form, and the
   rewrite is verified: applying those two replacements to the in-source array reproduces the fixture exactly, in
   order, all 15 entries. An import that copies the source constant without the rewrite produces presets the
   original never had, and an export that writes the source constant has the same defect in reverse.
2. **A URL rule is a pattern with three operators.** The field is not a substring: `configure::downloadDefaultOptions::urlType`
   (`legacy/src/configure.cpp:2246`) reads `*x` as *ends-with*, `x*` as *starts-with*, `*x*` as *contains*, and a
   bare `x` as *contains*. Matching walks the array in file order and takes the **first** rule that matches, because
   the callback returns `true` to stop the walk (`legacy/src/configure.h:262`, `forEach` at `:227-245`).
   Direction matters and is easy to invert: first match, not last.
3. **Per-engine defaults: two entries for one engine, one wins.** `engineDefaultDownloadOptions`
   (`legacy/src/configure.cpp:1123`) returns the first entry for that engine whose `default` is `"yes"`. The
   captured file has two `yt-dlp` rows with opposite `default` values, so "read them all and concatenate" is
   wrong in a way that still parses.

Two entries deserve emphasis:

- **The archive file is not ours.** `yt-dlp` and `gallery-dl` write it and read it; Morid only passes
  `--download-archive <path>`. It is never parsed for logic beyond the two flags of F-24 and never rewritten.
  A reimplementation that "normalises" it would silently make users re-download everything.
- **`archiveFile-gallery-dl.db` is SQLite.** Morid does not read it. Copying it and letting gallery-dl use it is
  the correct behaviour.

---

## 3. Settings

### 3.0 Encoding of `settings.ini` — how the original actually writes it

This section replaces an earlier version of itself that was **wrong**. The correction matters, because the
first reading produced a wrong conclusion about what "compatibility" means here.

**The original never calls `QSettings::setIniCodec`** (`grep -r setIniCodec legacy/src` finds nothing), and it
constructs `QSettings(m, QSettings::IniFormat)` on every platform (`legacy/src/settings.cpp:374-380`). Under
**Qt 5.15** that has a specific consequence, taken from the Qt documentation:

> By default, no codec is used, and non-ASCII characters are encoded using standard INI escape sequences.
> [...] QSettings will accept Latin-1 encoded INI files, but generate pure ASCII files, where non-ASCII values
> are encoded using standard INI escape sequences.

So a `DownloadFolder` of `/x/Загрузки` is **written** by the original as `/x/\x417\x430...` — ASCII on disk,
one escape per UTF-16 code unit, **lowercase hexadecimal with no padding** (`\x417`, not `\x0417`; a non-BMP
character becomes two escapes). The file is not UTF-8, not locale-encoded, and not something a byte-oriented
reader can interpret: it is an INI dialect with its own escaping. This is the fact that matters for Morid, and
it is unrelated to the machine's locale.

Three rules of that dialect are easy to get wrong, and all three are taken from
`qtbase/src/corelib/io/qsettings.cpp` (`iniEscapedString` and `iniUnescapedString`) rather than from memory:

1. **The hex scan is greedy, so the writer defends against it.** After emitting `\x<hex>`, Qt sets
   `escapeNextIfDigit`; if the next character is `0-9`, `a-f` or `A-F`, it escapes that character too. `Загрузки5`
   is therefore written `\x417...\x438\x35`, not `...\x4385` — and a reader whose scan is greedy (Qt's is)
   turns the naive form into a different string. A folder called `Отпуск2024` or `фото1` hits this.
2. **A value is quoted when it contains `;`, `,` or `=`, or begins or ends with a space** (`needsQuotes`). Quotes
   are part of the file, not of the value: Qt strips them on read.
3. **`\` is written as `\\`**, and the control characters `\0 \a \b \f \n \r \t \v` have their own
   short escapes.

Both mistakes are silent. The original does not report a bad value; it reads something else, the path does not
exist, and the folder is reset. This is why the round trip has to be tested rather than reasoned about.

The read side of the same documentation matters just as much: Qt 5 "will accept **Latin-1** encoded INI files".
A file written by something else — our capture harness, for instance — as literal UTF-8 is therefore not
rejected; it is decoded as Latin-1 and silently becomes different text. Cyrillic `/x/Загрузки` turns into
`/x/Ð—Ð°Ð³Ñ€ÑƒÐ·ÐºÐ¸`, which does not exist, which is why the original then resets the folder. Silent mojibake,
not an error, is the failure mode to design against.

(The locale theory in the first version of this section was wrong. What actually happened during the Phase 0
capture is recorded in `fixtures/README.md` (appears with PR #2) under s20 and is still an open question
for the capture — see [tasks/phase-0-fixtures.md](tasks/phase-0-fixtures.md) T7. The Qt behaviour above does not
depend on how that turns out: it is a property of the writer, not of the capture host.)

**Under Qt 6 the writer changed.** Qt 6 assumes INI values are UTF-8 and writes them as UTF-8; the Qt 6
documentation says INI files written by Qt 5 remain readable by Qt 6 only "unless an ini codec different from
utf8 had been set". A `settings.ini` produced by our Qt 5 reference build therefore does **not** round-trip
through a Qt 6 build of the original. That matters if a user ever runs a Qt 6 build of Media Downloader: the two
builds of *their own program* disagree about the same file.

**What this means for Morid — decided by D-016 (compatibility first):**

- **Read the Qt dialect, and say what was found.** A value containing `\x` escapes is decoded as Qt's; a value
  that is plain ASCII needs no decision. Two cases must be *reported rather than guessed*: a value whose bytes are
  valid UTF-8 but not ASCII (that is what a Qt 6 build of the original writes — see below), and a value that is
  neither valid UTF-8 nor pure ASCII. Never fall back to Latin-1 the way Qt 5 does; producing silent mojibake is
  the one behaviour not to copy.
- **Write the Qt 5 dialect.** Non-ASCII characters are emitted as `\x<hex>`, with the digit defence and the
  quoting rule above, exactly as Qt 5 does. This is ugly on
  purpose: it is the only form the reference program can read back. (Qt 6 fixed this and, ironically, broke
  itself — so "write what the reference does" has to name *which* reference build.)
- **ASCII files are byte-identical** under either rule, which is why none of this was visible for years.
- **Import decodes strictly and reports.** A file containing bytes that are neither valid UTF-8 nor valid Qt
  escapes is reported as a finding, not silently repaired.
- **Round trip is the acceptance test**, not "it looks right": write with Morid → read with the original →
  value intact ([§6](#6-export-and-going-back)).

The JSON data files are UTF-8 by construction. `archiveFile-<engine>.txt` is written by the backend, not by the
original, and is passed through byte for byte.

---

### 3.0a Keys are escaped too — and it is a different escape

The value escaping above does not apply to keys. `iniEscapedKey` keeps `a-z A-Z 0-9 _ - .`, writes `/` (the
group separator) as `\\`, and escapes everything else as **`%XX`** for code units up to 0xFF or **`%UXXXX`** above
— uppercase `U`, uppercase hex digits, zero-padded to four.

Three details, each of which has already been written wrong once in this project's own notes:

1. **`%XX` is a Latin-1 code point, not a UTF-8 byte.** `%D0%BF` is `Ð¿`, not `п`. A reader that pairs the bytes
   decodes real Windows files into plausible-looking nonsense.
2. **`%U` is uppercase only.** Qt tests `ch == 'U'`; `%u0417` is not an escape there. Lowercase is accepted only for
   the two hex digits of a `%XX` escape (`%e9`). Treating `%u…` as an escape is a divergence dressed as fidelity.
3. **The writer iterates UTF-16 code units, so a non-BMP character is two escapes** — `😀` is `%UD83D%UDE00` — while
   the same character in a *value* is `\xd83d\xde00`. A Rust `escape_key` written over `char` instead of `u16`
   produces `%U1F600` and is wrong.

`iniUnescapedKey` also reports `lowercaseOnly`, and **it lowercases nothing**: the flag is used only by the caller to
pick `Qt::CaseSensitive` versus the platform's `IniCaseSensitivity` when looking a key up. On Linux that sensitivity
is already case-sensitive, so the flag changes nothing there; on Windows it does. The stored key text keeps its case.

Malformed escapes: Qt appends a literal `%` and carries on, so `%zz` reads as the key `%zz`. **Morid refuses it
instead** — deliberately, and this is a documented divergence rather than a port. A file Qt wrote never contains a
malformed escape; one that does should fail loudly rather than resolve to a different key, which is the whole G11
failure mode.

### 3.0b A value is not always a string: `QStringList`

Six of the fixed keys hold a **list**, not a string: `BasicDownloaderOptionsHistory-<engine>`,
`BatchDownloaderOptionsHistory-<engine>`, `PlaylistDownloaderOptionsHistory-<engine>`, `PlaylistRangeHistory-<engine>`,
`PlaylistUrlHistory-<engine>` and `playlistRangeHistoryLastUsed-<engine>`. Qt has no separate syntax for them in an
INI file: a **comma outside quotes** starts the next item, and the writer emits `", "` between items. The captured
fixture has exactly one such key, with 21 items.

Two consequences, both silent when wrong:

- a reader that returns one string per line has already lost the shape, and nothing downstream can recover it;
- a writer that escapes such a value as a single string sees a comma, decides the value `needsQuotes`, and emits
  `"item, item, item"` — which Qt reads back as **one** item, because the quotes suppress splitting. The user's
  options history collapses to a single 400-character option, and the original accepts it without an error.

So the store carries a value as "one string **or** a list of strings", and the pair of functions is used the way
the corpus uses them: decode a value, and if it is a list, escape each item and join with `", "`. Verified: this
reproduces the committed `settings.ini` byte for byte (3 351 bytes). Passing the list to the value escaper is not
a supported call — it raises.

This matters beyond the fixtures: an engine or extension whose name is not ASCII produces such a key in a real
`settings.ini`, and Windows users have those files today. A reader that treats `%U417%U430` as a literal key name
will not find the setting it is looking at, and will report a missing key rather than a decoding problem.

**Morid must implement both escapes** — one helper for values, one for keys, in `morid-store`, with a round-trip
test that re-encodes what it decoded and compares bytes. They may share a UTF-16 code-unit splitter and nothing
else: the two dialects look similar and share no rules, and a single "smart" function is how a `%U` sequence ends
up inside a path.

**Status, 7 November 2026:** both escapes exist and are proved against Qt's own reader and writer in
`tools/capture/qt_ini.py` and `tools/capture/canonicalise.py`, by 30 stdlib tests and a seeded fuzz. That is the
specification in executable form; the Rust implementation in Phase 4 has to reproduce it, not re-derive it.

### 3.1 Keys

`settings.ini` holds 94 keys in the captured fixture: 81 fixed names plus 13 dynamically named ones
(`<engine>` is an extension's `Name`, `<tab>` is `basic`, `batch`, `playlist` or `LogWindow`)
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
BasicDownloaderOptionsHistory-<engine>    BatchDownloaderOptionsHistory-<engine>
PlaylistDownloaderOptionsHistory-<engine> PlaylistRangeHistory-<engine>
PlaylistUrlHistory-<engine>               playlistRangeHistoryLastUsed-<engine>
ThumbnailHeight_<tab>                     ThumbnailWidth_<tab>
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
UseSystemEngine                           UseSystemProvidedVersionIfAvailable
UseSystemProvidingEngineIfAvailable       UseSystemSupportingEngine
denoInFlatpakUpdated
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
