# Audit of the original code base (C++ / Qt)

An audit of the fork of **Media Downloader** (upstream `mhogomchungu/media-downloader`) at version **5.6.8**,
whose last `changelog` entry is dated 4 November 2026.

This document describes **what is there now**, not what is planned. Claims are backed by file references in the
form `file:line`. Where a conclusion comes from reading code rather than running it — no Qt toolchain was
available in the audit environment — that is said explicitly.

Decisions taken on the basis of this audit are in [decisions.md](decisions.md).

---

## 1. Current state

| Property | Value |
| --- | --- |
| Language | C++14, Qt 5.15 or Qt 6 (`CMakeLists.txt:11,32-52`) |
| Build system | CMake, plus a qmake project (`legacy/src/media-downloader.pro`) as a second path |
| Code size | 42,910 lines: 37 `.cpp`, 35 `.h`, 8 `.hpp` |
| Files / tree size | 202 files / ~12 MB |
| UI | 2 `.ui` files, 2,958 lines (`mainwindow.ui` 2,872, `logwindow.ui` 86) |
| Localisation | 18 `.ts`/`.qm` catalogues, 354 messages each |
| Extensions | 17 JSON engine descriptions in `extensions/` (the only part of the original tree that stays at the repository root) |
| Licence | GPL-2.0-or-later (`LICENSE` = GPLv2 text, `LICENSE.txt` = copyright notice) |
| CI | 10 workflows, all building and publishing artefacts, **none running a test**. Retired in Phase 1; recorded in [../legacy/removed-ci-workflows.md](../legacy/removed-ci-workflows.md) |
| Version control | one commit of history in this fork (`46e881e code improvement`) |
| Release history | 71 releases, 1.0.0 (January 2021) to 5.6.8 |

Because the fork carries a single commit, the reasoning behind any particular line is not recoverable from
history — only the current state is. Copyright notices inside the files are intact.

Three findings matter more for planning than the size of the code:

1. **There are no automated tests.** The only verification is a hidden CLI mode that spawns real backends against
   the network (`legacy/src/engines/tests.cpp`, `--media-downloader-test-engine-*`). The "reference behaviour" therefore
   has to be recorded by us, or there will be nothing to compare a Rust implementation against.
2. **The core is not the GUI; it is parsing other programs' output.** Most of the difficulty — 3,024 lines of
   `engines.cpp`, 2,083 of `engines.h`, 1,853 of `engines/yt-dlp.cpp`, 986 of `logger.h` — is reading progress and
   metadata out of the stdout of yt-dlp, gallery-dl, wget, aria2c, svtplay-dl and get-sauce. That output is
   controlled by other projects.
3. **The UI layer is entangled with the logic.** Handlers in `tabManager`, `batchdownloader` and
   `playlistdownloader` call engines directly; there is no service layer. This, not the widgets, is the bulk of
   the architectural work in a Rust rewrite.

---

## 2. Architecture

### 2.1 Entry point and lifecycle

`legacy/src/main.cpp`:

- `argv` parsed into `utility::cliArguments` (`legacy/src/utility.cpp:1705`);
- platform detection, including "pretend" platforms for debugging (`PretendPlatform`, `--pretend-win`,
  `--pretend-winLegacy`, `--pretend-winModern`);
- `settings` and `engines::enginePaths`;
- the self-update branch `utility::startedUpdatedVersion` — **Windows only** (`legacy/src/utility.cpp:1358`): renames
  the `update`/`update_new` directories at startup;
- the hidden engine test mode (`tests::test_engine`);
- single- or multi-instance mode selected by `-s` or the `SingleInstance` setting (`legacy/src/main.cpp:93`),
  implemented with `QLocalServer`/`QLocalSocket` (`legacy/src/utils/single_instance.hpp`).

The IPC protocol is a JSON event (`utility::event::toJson`, `legacy/src/utility.h:699`) carrying `Url`, `AutoDownload`,
`ShowMetaData` and `Proxy`. The socket path is `<basePath>/tmp/ipc` on Unix and `\\.\pipe\MediaDownloaderIPC` on
Windows (`legacy/src/engines.cpp:1350-1360`).

### 2.2 Composition: `Context` as a service locator

`legacy/src/context.hpp` — `Context` holds references to `settings`, `translator`, `Ui::MainWindow`, `QWidget`,
`MainWindow`, `Logger`, `engines`, `tabManager`, `networkAccess`, `versionInfo` and `printOutPut`.

Good: dependencies are passed explicitly; there are almost no global singletons. Bad: fifteen dependencies in one
object means every module depends on everything, nothing can be constructed without a `QApplication`, a UI and a
logger, and there are cycles (`Context` → `tabManager` → `basicdownloader` → `Context`).

This is the direct explanation for the absence of unit tests: the architecture does not permit one.

### 2.3 The engine layer is a god object

`legacy/src/engines.h` (2,083 lines) contains in one file:

- `engines::enginePaths` — every data path (`legacy/src/engines.h:202-330`);
- `engines::metadata` — release metadata from GitHub;
- `engines::engine` — a facade wrapper;
- `engines::engine::baseEngine` — the engine base class with **57 virtual methods**;
- `filter`, `filterOutPut`, `DataFilter`, `FilterOutPut`, `timer`, `preProcessing`, `postProcessing`,
  `optionsEnvironment`, `mediaInfo`, `finishedState`;
- `jsRuntimeInstalled`, `EnginesList`, `Iterator`, `proxySettings`.

Type nesting reaches `engines::engine::baseEngine::filterOutPut::result::callables` (`legacy/src/engines.h:660`).

The 57 virtual methods are not an "engine interface"; they are "everything an engine might want to override":
version parsing, JSON parsing, argument rewriting, argument-to-environment conversion, directory deletion, archive
renaming, completion text, skip conditions, update policy. Inheritance was used where composition would have
been smaller.

### 2.4 `baseEngine` subclasses (13 files in `legacy/src/engines/`)

| File | What is specific |
| --- | --- |
| `yt-dlp.cpp` (1,853 lines) | metadata JSON, `-o` template rewriting with playlist fields (`updateDownLoadCmdOptions`, ~line 1493), subtitles, comments, chapters |
| `gallery-dl.cpp` (736) | line-oriented JSON, own `parseJsonData`/`parseJson` |
| `svtplay-dl.cpp` (1,041) | own output filter and `updateVersionString` |
| `getsauce.cpp` (1,046) | banners, own progress, arguments converted to environment |
| `wget.cpp` (765), `aria2c.cpp` (325) | progress lines, `trimProgressLine`, `bundledEngine` |
| `you-get.cpp` (279), `python.cpp` | wrappers around the Python interpreter |
| `ffmpeg.cpp` (226), `deno.cpp` (183), `quickjs_ng.cpp` (193) | supporting binaries |
| `generic.cpp` | empty — everything comes from JSON |
| `tests.cpp` (185) | hidden engine self-test mode |

Conclusion: **the extension JSON does not cover everything.** Some engines need code in the application. This is
the key limit on "the Rust version reads the same JSON and works".

### 2.5 The extension schema (the de facto contract)

Collected from all 17 files in `extensions/`: **40** distinct keys. (An earlier draft of this line said 34 and
omitted six that the files do contain: `ControlJsonStructure`, `DownloadUrlWin7`, `ExtraArguments`,
`ExtraArgumentsFlatpak`, `ExtraArgumentsWin7`, `PlayListIdArguments`, `ShowListTableBoundary`. Corrected 8 November
2026 by counting the files; [extensions](extensions.md) §1 has the authoritative list.)

```
ArchiveContainsFolder, AutoUpdate, BackendPath, BatchFileArgument, CanDownloadPlaylist,
Cmd, ControlJsonStructure, CookieArgument, CookieArgumentTextFile, DefaultCommentsCmdOptions,
DefaultDownLoadCmdOptions, DefaultListCmdOptions, DefaultSubstitlesCmdOptions,
DefaultSubtitleDownloadOptions, DownloadUrl, DownloadUrlMAC, DownloadUrlWin7,
DumptJsonArguments, EncodingArgument, ExtraOptions, ExtraOptionsFlatpakAppImage,
ExtraOptionsWin7, LikeYoutubeDl, Name, OptionsArgument, Password, PlaylistItemsArgument,
RemoveText, ReplaceOutputWithProgressReport, RequiredMinimumVersionOfMediaDownloader,
SkipLineWithText, SplitLinesBy, SupportingEngine, UpdatableSupportingEngine,
UserName, Version, VersionArgument, VersionStringLine, VersionStringPosition
```

*(This list is the audit's original, therefore incomplete — the missing six are named in extensions.md §1 and the
files are the source of truth.)*

`Cmd` is nested by OS (`Generic`, `Windows`, `MacOS`) and architecture (`amd64`, `x86`, `aarch64`, `win7amd64`,
`win7x86`) — 34 combinations occur.

`ControlJsonStructure` is a small condition language over output lines: `Connector` (`&&`), `lhs`/`rhs` with
`startsWith`, `endsWith`, `contains`, `containsAny`, `containsAll` (`meetExtraCondition`,
`legacy/src/engines.cpp:1494-1580`). It exists to separate progress lines from noise without regular expressions.

Schema defects found during the audit:

1. **`ExtraOptions*` versus `ExtraArguments*`.** The code reads `ExtraOptions`, `ExtraOptionsWin7`,
   `ExtraOptionsFlatpakAppImage` (`legacy/src/engines.cpp:1052-1058`), while `extensions/ytdl-patched.json` contains
   `ExtraArguments`, `ExtraArgumentsFlatpak`, `ExtraArgumentsWin7` — **the code never reads them**. A shipped
   extension therefore silently loses three of its keys. Handled in [extensions.md](extensions.md) §4.1.
2. `DefaultSubstitlesCmdOptions` — a misspelling fixed in five files; must be preserved for compatibility.
3. `RequiredMinimumVersionOfMediaDownloader` is enforced at load time (`legacy/src/engines.cpp:264-272`): an extension
   with a higher minimum is rejected. With a new product version this rejects every existing extension — see
   [decisions.md](decisions.md) D-004.
4. `Version` is a schema version (1–4), but the difference between versions is documented nowhere and the reader
   almost never branches on it; the field is effectively decorative.
5. Source of extensions: a **pinned upstream commit**
   (`https://raw.githubusercontent.com/mhogomchungu/media-downloader/d20c57211494276b206b19c021d358746de5ba29/extensions/`,
   `legacy/src/configure.cpp:876`) and a menu of 10 names plus `custom` (`legacy/src/configure.cpp:1231-1252`), while the
   repository ships **17** files (`lux`, `safaribooks`, `yt-dlp-aria2c-native`, `quickjs*`, `deno`,
   `ytdl-patched` are unreachable from the menu).

### 2.6 The download pipeline

`utility::context<AppCtx, Logger, Events, Connection>` (`legacy/src/utility.h:1873-2135`) is a template wrapper over
`QProcess`:

- `whenCreated` — environment, working directory, channel mode, echo of the command into the log;
- `whenStarted` — a periodic elapsed-time timer for engines with `ReplaceOutputWithProgressReport`, plus
  credentials written to stdin;
- `withData` — stdout/stderr routing (`ProcessOutputChannels`) into `Logger`;
- `whenDone` — `ProcessExitState`, final text, `registerDone`;
- `cmd(args)` — proxy injection (as an argument **or** as environment variables) and argument-to-environment
  conversion by the engine.

Process control: `utils::qprocess::run` (`legacy/src/utils/qprocess.hpp`); cancellation via `utility::Terminator`
(`legacy/src/utility.cpp:621-680`) — `taskkill` on Windows, `SIGTERM` by PID on Unix.

Orchestration lives in the tabs: `basicdownloader` (694 lines), `batchdownloader` (3,208),
`playlistdownloader` (1,863). Each tab manages its own processes, queue, row state and completion reports. There
is no single scheduler: a "queue" is a walk over table rows with flags such as `running`, `finished`,
`splitByChapters` (`legacy/src/tableWidget.h:94-180`).

Concurrency is bounded by `MaxConcurrentDownloads` (default 4, `legacy/src/settings.cpp:623`) and `ConcurrentDownloading`.

### 2.7 `Logger` is a progress model, not a logger

`legacy/src/logger.h` (986 lines): per-process ring buffers, "progress lines" that replace rather than append,
specialisations `ProcessData`/`YtDlpData`/`SvtData`, limits (`MaxLoggerProcesses`, `HistorySize`,
`StringTruncationSize`), the log window, and the download history file (`downloadHistory.json`, appended objects
and substring search — `legacy/src/utility.cpp:2470-2520`).

It is simultaneously the log, the progress parser and the result store. In Rust this becomes three things.

### 2.8 Network and updates

`legacy/src/networkAccess.cpp` (1,168 lines):

- downloading backend binaries from GitHub (`api.github.com/.../releases/latest`) with size and SHA-256
  (`digest`) verification — `hashDoNotMatch` (`legacy/src/networkAccess.cpp:918`);
- archive extraction via external `bsdtar`/`unzip` (`legacy/src/networkAccess.cpp:929-990`);
- updating extensions and the application itself (Windows; `settings::gitHubDownloadUrl`,
  `legacy/src/settings.cpp:353-372`);
- proxy: `None` / `System` / `Env` / `Manual` (`legacy/src/settings.cpp:1416-1470`), with system proxy discovery from
  environment variables (`legacy/src/proxy.cpp`).

For a fork, every one of these addresses points at `mhogomchungu` and at a pinned commit — see
[decisions.md](decisions.md) D-010 and D-014.

### 2.9 GUI

`legacy/src/mainwindow.ui` (2,872 lines), tabs:

| Tab | Contents |
| --- | --- |
| Basic Downloader | URL, format table (Format Code / Extension / Resolution / File Size / Note), options, option history, "Play Downloaded Media" |
| Batch Downloader | URL table with thumbnails, time intervals, chapters, "Split By Chapters", add from file/clipboard, auto-download |
| Playlist Downloader | playlist URL, range, subscriptions, archive, comments, subtitles |
| Library | browse the download folder, rename, delete, sorting |
| Configure | 6 sub-tabs: General Options, Engine's Options, Preset Options, Extensions, Url Manager, Proxy Settings, UI Scale |
| About | links, engine versions |

Plus `logwindow.ui`, a tray icon with notifications (`legacy/src/mainwindow.cpp:88-218`), generated dark themes
(`<data>/themes`, `legacy/src/themes.cpp:201-280`), and RTL text alignment for Arabic
(`translator::setLanguage` → `setLayoutDirection` + `textAlignmentChanged`).

### 2.10 Settings and data files

`QSettings`, INI format, `<AppData>/media-downloader/settings/settings.ini` (`legacy/src/settings.cpp:374-410`). The
directory can be overridden with the `MediaDownloaderAppDataPath` environment variable
(`legacy/src/settings.cpp:423`); portable mode on Windows. 51 distinct keys via `getOption`, 61 including the dynamic
families (`CookieBrowser_<engine>`, `LastUsedOption<tab>_<engine>`, `TextEncoding_<engine>`,
`WindowDimensions_<tab>`). There is a migration path from the older registry-based
`QSettings("media-downloader","media-downloader")`.

Data files under `<data>/`:

| File | Purpose |
| --- | --- |
| `presetOptions.json` | 17 bundled presets (YouTube resolutions and similar) |
| `downloadDefaultOptions.json` | URL manager: substring → engine + options |
| `downloadEngineDefaultOptions.json` | default options per engine |
| `subscriptions.json` | playlist subscriptions |
| `autoSavedList.json` | batch/playlist list saved on exit |
| `downloadHistory.json` | download history |
| `archiveFile-<engine>.txt` / `.db` | already-downloaded archive (`--download-archive`) |
| `themes/*.json` | themes |

Details and import rules: [data-compat.md](data-compat.md).

### 2.11 Localisation

18 catalogues: `ar, bg_BG, de_DE, el_GR, en_US, es_ES, fr_FR, it_IT, ja_JP, ko_KR, nl_NL, pl_PL, pt_BR, ru_RU,
sv_SE, tr_TR, uk_UA, zh_CN`. 354 messages each. Strings in code use `tr()`, strings in the UI live in `.ui`.
Language names are themselves translated (`legacy/src/translator.cpp:32-52`). The format is Qt `.ts`/`.qm`. There is no
documentation for translators; there is a script, `legacy/src/update_translations`.

### 2.12 Build and distribution

- CMake: Qt5/Qt6, `AUTOMOC`/`AUTORCC`/`AUTOUIC`, C++14; generates `version.h`, `.desktop`, AppStream metainfo,
  the macOS bundle and Inno Setup scripts for Qt5 and Qt6 inside `CMakeLists.txt`.
- Scripts: `build_linux-qt5.sh`, `build_arch-qt5.sh`, `build_fedora-qt5.sh`, `build_windows.ps1`.
- Flatpak manifest (`legacy/src/flatpak/*.json`), AUR, Fedora and AppImage packaging live elsewhere.
- CI (10 workflows): macOS arm/Intel, Windows (Qt5), publishing through `repository-dispatch` into
  `mhogomchungu/media-downloader-git` using secrets. **In the fork these cannot work** (no secrets, no target
  repositories) and, more importantly, none of them runs a test.

### 2.13 Technical limits and defects recorded by the audit

| # | Finding | Where |
| --- | --- | --- |
| D1 | `metadata::valid()` checks `m_url` twice and does not check hash or file name — a release without a digest is "valid" | `legacy/src/engines.h:344-350` |
| D2 | Errors are strings recognised by substring: `m.contains("Download Failed(ErrorCode=1)")` | `legacy/src/playlistdownloader.cpp:1791-1795` |
| D3 | `baseEngine` has 57 virtual methods and deeply nested types | `legacy/src/engines.h` |
| D4 | No unit tests; engine verification is a hidden CLI mode that needs the network | `legacy/src/engines/tests.cpp` |
| D5 | Extensions are fetched from a pinned upstream commit; the menu lists 10 while 17 ship | `legacy/src/configure.cpp:876,1231` |
| D6 | `ExtraArguments*` keys in `ytdl-patched.json` are never read | §2.5 |
| D7 | Four licence files, none identified as normative | repository root |
| D8 | `QSettings` is read from worker threads (15 call sites) — potential races | `legacy/src/*.cpp` |
| D9 | `//?????` placeholders in the extension-download completion handlers | `legacy/src/configure.cpp:983-987` |
| D10 | Application self-update renames directories at startup — a fragile path | `legacy/src/utility.cpp:1358-1400` |

---

## 3. What the program does

The full functional map, with a priority and an acceptance criterion per feature, is in
[features.md](features.md). In outline:

- **Downloading:** format listing, format/option selection, single download, progress with speed and ETA,
  cancellation, 17 presets, option history, subtitles, comments.
- **Batch:** URL table, load from file, auto-download on add, concurrency limit, thumbnails, time intervals,
  chapters, save/restore, playing the result, renaming, conflict handling.
- **Playlists:** entry listing, ranges, selective and recursive download, download archive,
  break-on-existing / skip-on-existing, subscriptions, histories.
- **Engines:** extension loading, command selection by OS/architecture, version reading and comparison, binary
  update with hash verification, install/remove, yt-dlp family behaviour, supporting engines (ffmpeg, deno,
  quickjs-ng, python, bsdtar), custom extension.
- **Configuration:** folders, per-engine defaults, URL manager, proxy (four modes), cookies, encoding, themes,
  UI scale, per-tab default engine, history sizes.
- **Integration:** 18 languages, tray and notifications, clipboard monitoring, single instance and IPC,
  portable mode, download history, library, window geometry, "run a command after download".
- **Platforms:** Linux (X11/Wayland, Flatpak), macOS arm64/x86_64, Windows (x64 and, historically, x86 and 7/8).

---

## 4. Shape of the Rust design

Full proposal: [architecture.md](architecture.md). The short form:

```
morid-core    domain types, no I/O
morid-ext     extension schema and command resolution
morid-engine  process spawn, stream parsing, cancellation
morid-store   settings, data files, import
morid-net     HTTP, hashes, unpacking
morid         service layer: queue, job lifecycle, argv construction
morid-cli     console binary
morid-gui     Slint window
```

Five structural choices, each with a cost:

1. **An engine is data plus a small closed set of named behaviours**, not a trait with 57 methods. Cost: a truly
   unusual future engine needs code and a decision, rather than just a subclass.
2. **One scheduler.** Queue and job state live in the service layer; the tabs are views. Cost: the logic of three
   tab classes is rebuilt as one state machine.
3. **Events, not UI callbacks.** The core emits `Event`s; the GUI and the CLI subscribe. This is what makes the
   console interface and the tests possible.
4. **Typed errors** instead of `ErrorCode=N` string matching. Cost: comparison with the original is by exit code,
   not by message text.
5. **Compatibility by import, not by co-ownership.** Morid reads the original's data once and writes its own.
   Cost: a one-time migration that must be tested against a real directory.

---

## 5. Technology stack

Decided: Slint for the interface ([decisions.md](decisions.md) D-001), GPL-2.0-or-later for the project, pure-Rust
TLS and compression so that no C toolchain is needed.

Candidate set, with the argument for each: [dependencies.md](dependencies.md). In outline — `serde`/`serde_json`,
`sha2`, a small synchronous HTTP client, `tar`/`flate2`/`zip`, `libc` (Unix) and `windows-sys` (Windows), gettext
for translations. Explicitly rejected: an async runtime, a time-zone library, a globbing/naming crate collection,
`thiserror`, a logging framework.

Constraints that shaped the choice and are not negotiable: no C compiler at build time, no network at build time,
build works offline, no `unsafe` in our code.

---

## 6. Migration risks

| # | Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- | --- |
| R1 | No behavioural reference exists — the original has no tests | high | high | Phase 0: capture argv and output as fixtures before writing code |
| R2 | Backend output format changes break parsing | high | high | rules are data (`ControlJsonStructure`); fixtures per rule; a nightly manual diff job |
| R3 | GUI parity: tables, thumbnails, context menus, tray | medium | medium | functional parity first, pixel parity never promised; gaps published |
| R4 | RTL and localisation | medium | medium | decision D-002: text correct, layout not mirrored; stated publicly |
| R5 | Compatibility with third-party extensions (40 keys, 5 schema versions, dead keys) | high | high | schema frozen as a versioned public contract; all 17 files in the test set |
| R6 | `RequiredMinimumVersionOfMediaDownloader` ties extensions to a product version | high | medium | D-004 soft check with clear diagnostics |
| R7 | All endpoints point at upstream | certain | high | D-010 and D-014: extensions local, backends from their own projects, application endpoint ours |
| R8 | Windows 7/8 support | removed | — | D-005: not supported, recorded in [platforms.md](platforms.md) |
| R9 | Scope: 42.9k lines of C++ plus UI plus 18 languages | certain | high | vertical slices by feature with acceptance criteria; no "rewrite everything" task |
| R10 | GPL obligations when code is derived | certain | high | keep GPL-2.0-or-later and the author's notices; THIRD-PARTY; licence review before release |
| R11 | Two programs to maintain during the transition | medium | medium | fix the point at which the C++ version stops being a source of new features |
| R12 | No CI in the fork | certain | medium | Phase 1: fmt/clippy/test/build on three platforms before any functional code |

---

## 7. Repository structure

Proposed and in force:

```
Morid/
├── Cargo.toml                 workspace
├── Cargo.lock
├── rust-toolchain.toml
├── README.md  SECURITY.md  CONTRIBUTING.md  CHANGELOG.md
├── CHANGELOG-legacy.md        the original's history, unedited
├── LICENSE  LICENSE.txt  GPLv2  GPLv3  LICENSES.md
├── THIRD-PARTY.md
├── legacy/                    the C++ program as a behavioural reference (D-006, D-015)
├── docs/                      this documentation
├── crates/                    see §4
├── ui/                        .slint files
├── extensions/                the 17 JSON files, schema version preserved (D-003)
├── i18n/                      message catalogues
├── assets/                    icons, .desktop, AppStream metadata
├── packaging/                 flatpak, windows, macos, linux
├── archive/ru/                Russian-language copies of early working documents (D-012)
└── xtask/                     build, size measurement, packaging
```

`extensions/`, the licence files and the icons stay in the root: the extensions are an active contract, and the
copyright notices are not moved or edited.

---

## 8. Phased plan

Details and estimates: [roadmap.md](roadmap.md). Sequence:

0. **Reference behaviour** — capture argv and backend output as fixtures. No Rust code.
1. **Skeleton** — workspace, CI on three platforms, size reporting, `legacy/` move, documentation.
2. **Domain and extension schema** — all 17 files load, validate and resolve to a command; no process is spawned.
3. **One backend end to end** — yt-dlp via `morid list` / `morid download`, argv equal to the golden fixtures,
   cancellation kills the process tree.
4. **User data** — settings and JSON files, import and export, round trip readable by the original.
5. **Network** — backend installation and updates with integrity checking.
6. **Remaining backends** — gallery-dl, you-get, svtplay-dl, get-sauce, wget, aria2c.
7. **Interface** — one screen at a time, translations with each screen.
8. **Parity and release** — packaging, gates, release notes for a user coming from Media Downloader.

Headless first, because it is verifiable at every step and produces something useful before the window exists.

---

## 9. Questions for the owner

All fifteen were answered on 4 November 2026 and are recorded in [decisions.md](decisions.md).

| Question | Decision | Question | Decision |
| --- | --- | --- | --- |
| Q1 UI toolkit | D-001 Slint | Q9 download profiles | D-011 not introduced |
| Q2 RTL | D-002 deferred | Q10 documentation language | D-012 English |
| Q3 Slint licence | D-007 royalty-free | Q11 repository | D-006 this fork |
| Q4 `legacy/` lifetime | D-015 permanent | Q12 binary size | D-008 measured, not gated |
| Q5 compatibility | D-003 full | Q13 console interface | D-009 product feature |
| Q6 extension versions | D-004 soft check | Q14 extension source | D-010 repository files |
| Q7 Windows | D-005 10/11 only | Q15 network endpoints | D-014 split |
| Q8 self-update | D-013 notice only | | |

---

## 10. Next step

The audit is complete and the architecture is agreed. The first task for the coder is Phase 0 (reference
behaviour and fixtures) followed by Phase 1 (workspace and CI), as two separate tasks. No task to reimplement
functionality is issued before the behaviour it must reproduce has been recorded.
