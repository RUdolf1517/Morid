# Functional specification and migration map

Related: [audit](audit-original.md) · [architecture](architecture.md) · [roadmap](roadmap.md) ·
[extensions](extensions.md) · [cli](cli.md) · [testing](testing.md)

Every feature of the original program, with an ID, where it lives in `legacy/`, what it depends on, how
important it is for the replacement, its migration status, and how we will know it works.

**Read the status column honestly.** As of 10 November 2026 Phases 0–2 are accepted: there is a workspace, a
domain layer, an extension reader and a read-only `engines` command. Four rows are therefore `done` or `partly`
— the extension reader, the command selection, the version check and the range field — and everything that
downloads, stores or draws is still `not started`. "Done" means "a test in CI proves the acceptance criterion",
never "someone wrote something".

## Priority scale

| Priority | Meaning |
| --- | --- |
| **P0** | The replacement is not usable without it. Blocks the first usable release. |
| **P1** | Needed for parity; the first release may ship without it only with a documented gap. |
| **P2** | Parity, but deferrable to a later release without breaking the promise. |
| **P3** | Not carried over. Requires a decision record; keep the gap documented. |

## Status scale

`not started` · `in progress` · `done` (test exists and passes) · `blocked` (says what on) ·
`not carried` (P3, with a decision reference)

---

## 1. Downloading and formats

| ID | Feature | In the original | Depends on | Priority | Status | Acceptance criterion |
| --- | --- | --- | --- | --- | --- | --- |
| F-01 | Fetch the list of formats a URL offers | `basicdownloader::list`, `engine::dumpJsonArguments` | backend `--print` JSON | P0 | done (Phase 3, one of three URLs) | For three fixture URLs the parsed format table equals the one captured from the original field by field — **met for one of three URLs** in Phase 3: the captured `fixtures/streams/yt-dlp/list.json` is replayed through the backend and the real CLI, and every field of every format plus the printed table is asserted. **Not met** for the two other URLs, and the capture holds one format with `resolution: null`, so a wider table (several formats, differing `ext`/`resolution`/`filesize`) is still untested |
| F-02 | Choose format / options | `utility::args::parseOptions` (`legacy/src/utility.cpp:1548`) | backend, `OptionsArgument` | P0 | done (Phase 3) | `mp4`, `-f 22`, `default` and an empty string each produce the same argv as the original |
| F-03 | Download a single URL | `basicdownloader::download` | backend | P0 | done (Phase 3) | File appears in the download folder; progress events observed; exit state recorded |
| F-04 | Progress, size, speed, ETA | `Logger`, `ControlJsonStructure`, per-engine filters | backend output | P0 | done (Phase 3) | Replaying captured output yields the same sequence of progress values as the original |
| F-05 | Cancel a download | `Terminator::terminate` (`legacy/src/utility.cpp:621`) | OS process control | P0 | done (Phase 3) | No descendant process survives cancellation, on Linux and Windows; partial files are kept unless the setting says otherwise |
| F-06 | 15 bundled presets | `configure::presetOptions::defaultData` | — | P0 | not started | The preset list loads from the shipped file and matches the original's 15 entries ([data-compat](data-compat.md) §5.3) |
| F-07 | Option history per field | `settings::addToHistory`, `HistorySize` | settings | P1 | not started | History round-trips through settings; size limit enforced |
| F-08 | Subtitle download and embedding | `DefaultSubstitlesCmdOptions`, `--embed-subs` | backend, ffmpeg | P1 | not started | Subtitle arguments match; requested language files are produced |
| F-09 | Comment retrieval and saving | `--get-comments`, `showComments` | backend | P2 | not started | Comments JSON is fetched and saved in the original's format |

## 2. Batch downloading

| ID | Feature | In the original | Depends on | Priority | Status | Acceptance criterion |
| --- | --- | --- | --- | --- | --- | --- |
| F-10 | URL table with add/remove | `batchdownloader::addToList` | — | P0 | not started | Rows add, remove and persist like the original |
| F-11 | Load URLs from a file (txt/json) | `batchdownloader::getListFromFile` | — | P0 | not started | A list saved by the original loads without loss, including per-entry options |
| F-12 | Auto-download newly added URLs | `AutoDownloadWhenAddedInBatchDownloader`, `-a` | IPC | P0 | not started | `-u <url> -a` in a second instance starts a download in the first |
| F-13 | Concurrent downloads with a limit | `MaxConcurrentDownloads` (default 4) | scheduler | P0 | done (Phase 3) | With limit 2 and five queued jobs, at most two run at any instant and all five finish |
| F-14 | Thumbnails and metadata in the table | `getMetaData`, `ShowMetaDataInBatchDownloader` | network, image decoding | P1 | not started | Thumbnail and title populate; a failed fetch leaves the row usable |
| F-15 | Time interval, chapters, split by chapters | `--download-sections`, `cbSplitByChapters`, `lineEditChapters` | backend | P1 | not started | Generated arguments identical to the original for the same input |
| F-16 | Save and restore the list | `autoSavedList.json`, `AutoSavePlaylistOnExit` | store | P1 | not started | The list survives a restart; file readable by the original |
| F-17 | Play a downloaded file | `settings::mediaPlayer` (VLC or the system default) | external player | P2 | not started | Configured player opens the file; a missing player gives a clear message |
| F-18 | Rename an entry / file | `renameFile`, `normalizeFilePath` | filesystem | P2 | not started | Rename is reflected on disk and in the list; invalid names rejected |
| F-19 | Name conflicts and destination normalisation | `normalizeFilePath` | filesystem | P1 | not started | A file that already exists is not silently overwritten; path separators normalised per platform |

## 3. Playlists and subscriptions

| ID | Feature | In the original | Depends on | Priority | Status | Acceptance criterion |
| --- | --- | --- | --- | --- | --- | --- |
| F-20 | Fetch a playlist's entries | `playlistdownloader::parseJson`, `CanDownloadPlaylist` | backend | P0 | not started | Parsed entry list equals the captured original result for the fixture playlist |
| F-21 | Item range selection | playlist tab range field, `splitPreserveQuotes` | backend | P0 | partly in Phase 2 | The range field produces identical argv tokens; **no validation** — the original has none, a bad value reaches the backend and its message is shown ([extensions](extensions.md) §5.4) |
| F-22 | Download selected / recursive | `download`, `downloadRecursively` | scheduler | P0 | not started | Per-row state transitions match; a cancelled parent leaves no children running |
| F-23 | Download archive (`--download-archive`) | `enginePaths::archiveFileArgument` | archive file | P0 | partly (Phase 3): the argument and the setting are built and tested; the archive file itself is the backend's and the store owns it in Phase 4 | A second run skips already-downloaded entries; archive format identical to the original's |
| F-24 | `--break-on-existing` / `--skip-on-existing` | `archiveData` (`legacy/src/utility.h:654`) | archive | P1 | not started | Behaviour and stop condition match the original on the fixture playlist |
| F-25 | Subscriptions to playlists | `subscriptions.json`, `subscription::save` | store, archive | P1 | not started | Add, remove and persistence match; file readable by the original |
| F-26 | Automatic subscription checking | not present — the original requires a manual "Get List" | scheduler | P2 | not started | A scheduled check runs at the configured interval and reports new entries; **this is a new feature, not a migration** |
| F-27 | Playlist URL history | `PlaylistUrlHistory`, `PlaylistRangeHistory` | settings | P2 | not started | History behaves as in the original |

## 4. Engines and extensions

| ID | Feature | In the original | Depends on | Priority | Status | Acceptance criterion |
| --- | --- | --- | --- | --- | --- | --- |
| F-30 | Read an extension JSON | `engines::engine` ctor (`legacy/src/engines.cpp:~1000-1120`) | — | P0 | done (Phase 2) | All 17 files in `extensions/` parse; a file with an unknown key parses and warns, a file with a missing `Name` fails with a precise message |
| F-31 | Select the command by OS and architecture | `Cmd`, `addJsonCmd` | — | P0 | done (Phase 2) | On each supported platform the selected executable and arguments match a table-driven expectation |
| F-32 | Read a backend's version | `VersionArgument`, `VersionStringLine/Position` | process | P0 | not started | Correct versions extracted for yt-dlp, gallery-dl, wget, aria2c fixture outputs |
| F-33 | Enforce a minimum compatible version | `RequiredMinimumVersionOfMediaDownloader` (`legacy/src/engines.cpp:264`) | — | P0 | done (Phase 2) | `2.x` satisfies the check ([D-004](decisions.md)); a `9.x` requirement refuses with a message naming the required version |
| F-34 | Update a backend binary | `networkAccess::download`, `AutoUpdate` | `morid-net` | P0 | not started | Download verified by size and SHA-256; a hash mismatch aborts and leaves the previous binary intact |
| F-35 | Install / remove an extension | `configure` (Extensions sub-tab) | `morid-ext`, filesystem | P0 | not started | Install and remove work; removal of a bundled engine is refused where the original refuses |
| F-36 | Engine variants (yt-dlp-aria2c, -ffmpeg, -nightly) | `extensions/*.json` + `configure.cpp` | — | P1 | not started | All variants appear in the menu and produce their own argument sets |
| F-37 | yt-dlp family behaviour (`LikeYoutubeDl`) | `engines::engine` | — | P0 | not started | A family member shares the archive file and parser with yt-dlp, exactly as the original shares them |
| F-38 | Supporting engines (ffmpeg, deno, quickjs-ng, python, bsdtar) | `getSupportingEngineByName` (`legacy/src/engines.cpp:311`) | network | P0 | not started | Each is found, version-checked and installed per platform rules |
| F-39 | Custom extension (user-supplied binary) | `configure::addAction` with `custom` | — | P1 | not started | A user-chosen executable with user-chosen arguments downloads successfully |
| F-40 | `--pretend-*` platform overrides | `utility::PretendPlatform` | — | P2 | not started | Debug overrides select the same commands as the real platform would |
| F-41 | Built-in engine self-test mode | `legacy/src/engines/tests.cpp` | network | P3 | not carried | Replaced by the fixture-based suite ([testing](testing.md)); a network self-test is not carried over |

## 5. Settings, profiles, routing

| ID | Feature | In the original | Depends on | Priority | Status | Acceptance criterion |
| --- | --- | --- | --- | --- | --- | --- |
| F-50 | Download folder and library folder | `DownloadFolder`, `LibraryDownloadFolder` | — | P0 | partly (Phase 3): the destination is created before the job and a non-writable folder gives exit 6; the library folder and the settings UI are later phases | Paths applied; a missing folder is created; a non-writable folder gives a clear error |
| F-51 | Per-engine default options | `downloadEngineDefaultOptions.json` | store | P0 | not started | Options applied to every download of that engine; file compatible with the original |
| F-52 | URL manager (substring → engine + options) | `downloadDefaultOptions.json`, `getEngineNameFromUrlManager` | store | P0 | not started | A rule matches as the original matches it, including precedence between rules |
| F-53 | Download "profiles" | **not a separate feature** — presets + URL manager + per-engine defaults | — | P3 | not carried | Recorded as covered by D-011; a named-profile feature is a roadmap item, not a migration |
| F-54 | Proxy: none / system / env / manual | `settings::proxySettings`, `legacy/src/proxy.cpp` | environment | P0 | not started | Each mode produces the same arguments or environment variables as the original, including the manual string format |
| F-55 | Cookies from a browser or a file | `CookieArgument`, `CookieArgumentTextFile` | backend | P0 | partly (Phase 3): cookie arguments match the goldens, including a path with a space; reading the cookie file and per-platform quoting tests are not done | Arguments identical; the cookie file path is quoted correctly on all platforms |
| F-56 | Text encoding of backend output | `EncodingArgument` | backend | P1 | not started | Non-UTF-8 output is decoded as configured; the setting is ignored where the backend does not support it |
| F-57 | Themes (dark, custom JSON) | `themes.cpp`, `<data>/themes` | — | P1 | not started | Bundled themes match the original's colour values; a custom theme file loads |
| F-58 | UI scale | `HighDpiScalingFactor*`, `UI Scale` sub-tab | — | P2 | not started | Scale changes take effect and persist |
| F-59 | Tray icon and notifications | `mainwindow.cpp:88-218` | platform integration | P1 | not started | Notification appears on download completion and on all-complete, honouring the two settings |
| F-60 | Command after a download / after all | `CommandOnSuccessfulDownload`, `CommandWhenAllFinished` | shell | P2 | not started | Command runs with documented substitution; a failing command does not fail the download |
| F-61 | 18 interface languages | `translator.cpp`, `.ts`/`.qm` | i18n | P0 | not started | All 18 selectable; for `ar` text renders correctly with LTR layout ([D-002](decisions.md)) |
| F-62 | Single instance + IPC | `single_instance.hpp`, `-u/-a/-e/-p` | local socket | P0 | not started | Second instance forwards the URL and exits; a stale socket does not block startup |
| F-63 | Portable mode (Windows) | `--portable`, `settings::init` | — | P1 | not started | Data stored beside the binary; switching modes does not lose settings |
| F-64 | Download history | `downloadHistory.json`, log window | store | P2 | not started | History written, readable and searchable; format compatible with the original |
| F-65 | Library (built-in file manager) | `library.cpp`, `directoryEntries.cpp` | filesystem | P1 | not started | Navigate, rename, delete, sort; deletion is confirmed and irreversible-action safe |
| F-66 | Self-update of the application | `versionInfo`, `startedUpdatedVersion` | network | P3 | not carried | Not carried ([D-013](decisions.md)); only a "new version available" notice remains |
| F-67 | Window geometry persistence | `MainWindowDimensions`, `WindowDimensions_*` | settings | P2 | not started | Window reopens at the stored size and position |
| F-68 | User data import from Media Downloader | not present (no need) | store | P0 | not started | An import of a real data directory reproduces settings, presets, URL rules, subscriptions, archive and history without loss; unknown keys preserved or reported |

## 6. Console interface (our addition, decided in D-009)

| ID | Feature | Depends on | Priority | Status | Acceptance criterion |
| --- | --- | --- | --- | --- | --- |
| C-01 | `morid list <url>` prints formats | `morid-core`, `morid-ext` | P0 | done (Phase 3) | Human-readable and `--json` forms both stable; documented in [cli.md](cli.md) |
| C-02 | `morid download <url>` | scheduler | P0 | done (Phase 3) | Downloads to the configured folder; `--dry-run` prints argv without running |
| C-03 | `morid engines` | `morid-ext` | P0 | partly in Phase 2 | Resolution half done: every bundled extension printed with platform, selected architecture, argv and verdict, `--json` as one object per line ([cli](cli.md) §2.3). Version reading and `--update/--install/--remove` are Phase 5 |
| C-04 | `morid import <dir>` | `morid-store` | P1 | not started | Imports a Media Downloader data directory |
| C-05 | Exit codes | — | P0 | partly (Phase 3): 1, 2, 3, 4, 5, 6 are produced by tests; 7, 8 and 9 arrive with the store and the import | Every documented code is produced by a test |
| C-06 | `--json` event stream | event model | P1 | done (Phase 3) | One JSON object per line, schema documented and versioned |

## 7. Platforms and packaging

| ID | Feature | Priority | Status | Acceptance criterion |
| --- | --- | --- | --- | --- |
| F-70 | Linux x86_64 and aarch64 | P0 | not started | Release build runs on a clean container of the oldest supported distribution |
| F-71 | macOS arm64 and x86_64 bundle | P0 | not started | Bundle launches from Finder; no unsigned-library rejection |
| F-72 | Windows 10/11 x86_64 installer and portable | P0 | not started | Installer, portable zip and uninstall all work; no admin prompt for portable |
| F-73 | Windows 7/8 | P3 | not carried | Not carried ([D-005](decisions.md)) |
| F-74 | Icons, `.desktop`, AppStream metadata | P1 | not started | `appstreamcli validate` passes; desktop entry validates |
| F-75 | CI on three platforms with tests | P0 | not started | fmt, clippy, test and build green on all three; sizes reported |
| F-76 | Flatpak manifest | P1 | not started | Manifest builds against the current runtime |
| F-77 | Reproducible-ish release artefacts with checksums | P1 | not started | `SHA256SUMS` published and verified by the install instructions |

---

## 7a. Progress after Phase 3, counted from this file

Produced by parsing the tables above, not by memory — the same rule the note in §8 exists for. Phase 3 accepted on
11 November 2026; `partly` counts as half, because half a feature is not a feature but is not nothing.

| Priority | done | partly | not started | not carried | weighted |
| --- | --- | --- | --- | --- | --- |
| P0 | 11 | 6 | 21 | 0 | 37% of 38 |
| P1 | 1 | 0 | 19 | 0 | 5% of 20 |
| P2 | 0 | 0 | 10 | 0 | 0% of 10 |
| P3 | — | — | — | 4 | — |
| **All 72** | **12** | **6** | **50** | **4** | **22%** |

**The number that matters for D-003 is the second one: 26% of parity.** Parity means every P0 **and** P1 row with a
passing test, so 12 done and 6 partly out of 58 is a quarter of the promise, not a third.

A fact worth keeping next to it, because it explains why the percentage looks low while the hard part is done: the
three phases so far built the **foundation** — domain types, the extension reader, the process engine, the stream
classifier, the queue and the CLI. What remains is mostly *breadth*: settings, the store, playlists, the window,
packaging. Breadth is cheaper per feature than foundation, which is why the roadmap's remaining estimates total
less than what has already been spent on Phases 0–3.

Lines of code are a poor proxy and are given here only to answer the question honestly: the original is about
**29 000 lines of C++** (20 000 non-comment `.cpp`, 9 600 headers) plus 3 000 lines of `.ui`, and `morid` is about
**4 400 non-comment lines of Rust** in non-test code — but the Rust replaces the *mechanism*, while the original's
behaviour was captured as **1 200 lines of extension JSON** and **40 fixtures**. Comparing line counts across the
two languages measures the wrong thing; comparing features does not.

---

## 8. Summary

| Priority | Count | Meaning for planning |
| --- | --- | --- |
| P0 | 38 | Everything in this row blocks "usable replacement" |
| P1 | 20 | Parity; may ship with a named gap in a first release, but not in 1.0 |
| P2 | 10 | Later releases |
| P3 / not carried | 4 | F-41, F-53, F-66, F-73 — each has a decision record |
| **Total** | **72** | 68 `F-` rows and 4 `C-` rows |

The first version of this table said 44 / 22 / 14 / 4, which adds up to 84 and was never true: it was written
before the rows were, and nobody counted afterwards. Recounted on 10 November 2026 by parsing this file. The
lesson is the same one the rest of the project keeps relearning — a number in prose is worth nothing until
something checks it. If these counts matter for planning, they should be produced by a script, not by memory.

Two rows are not migrations and are marked as such: **F-26** (automatic subscription checking) and the whole
`C-` section (console interface) are additions. They must not be counted as "already in the original" when
effort or parity is discussed.

The distinction matters for the promise in D-003: *parity* means every P0 and P1 row with a passing test.
Everything else is either a documented gap or a new feature.
