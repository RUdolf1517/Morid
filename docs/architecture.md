# Architecture

Related: [features](features.md) · [migration](migration.md) · [extensions](extensions.md) ·
[dependencies](dependencies.md) · [testing](testing.md)

This document describes the Rust design: what the crates are, what each one is allowed to know, and how a
download actually flows through them. It is written before the code exists, so it is a specification to
implement against, not a description of what is there.

Two constraints shape everything below:

1. **The interface must not be load-bearing.** In the original, no module can be constructed without an
   application object, a UI object and a logger, which is why it has no unit tests
   ([audit](audit-original.md) §2.2). Here the domain must work with no window open, no display and no
   network, or it cannot be tested.
2. **The backend tools are external and hostile.** Morid does not download anything; it spawns
   yt-dlp/gallery-dl/wget/aria2c and interprets their output. That output is not a stable API. Parsing it is
   the central problem, and it must be data-driven and testable against captured samples.

---

## 1. Crate map

```
                        ┌──────────────────────────┐
                        │        morid-gui         │  Slint window, widgets, i18n at the edges
                        └────────────┬─────────────┘
                                     │ events / commands
┌──────────────┐        ┌────────────▼─────────────┐
│  morid-cli   │◄──────►│          morid           │  application service layer:
│  (bin)       │        │   (planning + jobs)      │  queue, concurrency, job lifecycle,
└──────────────┘        └────┬──────────┬──────────┘  command construction
                             │          │
              ┌──────────────▼──┐   ┌───▼──────────────┐
              │  morid-engine   │   │   morid-store    │  settings, presets, subscriptions,
              │  spawn + parse  │   │   files, import  │  archive, history, import
              └────┬───────┬────┘   └───┬──────────────┘
                   │       │            │
        ┌──────────▼──┐  ┌─▼──────────┐ │
        │ morid-ext   │  │ morid-net  │ │
        │ extension   │  │ HTTP, hash │ │
        │ schema      │  │ unpack     │ │
        └──────┬──────┘  └─────┬──────┘ │
               │               │        │
            ┌──▼───────────────▼────────▼──┐
            │          morid-core          │  domain types only: no I/O, no processes
            └──────────────────────────────┘
```

| Crate | Responsibility | Must not contain |
| --- | --- | --- |
| `morid-core` | Domain types and rules: `Url`, `Format`, `DownloadOptions`, `EngineId`, `JobState`, `Error`, option-token parsing (including the playlist range field), version comparison | file I/O, processes, network, time, randomness |
| `morid-ext` | Reading and validating the extension JSON (40 keys in the shipped set, schema 1–4), selecting the command for the current OS/arch, evaluating `ControlJsonStructure` rules | networking, spawning |
| `morid-engine` | Spawning a backend, reading stdout/stderr, applying an engine's parse rules, emitting progress/metadata/destination events, cancelling a whole process tree | knowledge of widgets, knowledge of specific engines beyond their data |
| `morid-store` | Settings (INI-compatible), presets, URL rules, per-engine defaults, subscriptions, archive files, download history, one-time import of a Media Downloader data directory | processes, network (except through an injected trait) |
| `morid-net` | HTTP GET/POST, GitHub release metadata, size + SHA-256 verification, archive unpacking | deciding *what* to download |
| `morid` (lib + bin) | Application service: assembling an engine invocation, the job queue and concurrency limit, lifecycle events, single-instance IPC | rendering |
| `morid-cli` | Argument parsing, exit codes, output formatting, wiring the service layer to a terminal | domain logic of its own |
| `morid-gui` | Slint UI, view models, translation lookup, themes, tray, notifications, file dialogs | process spawning, option-string parsing, file formats |

The arrows matter. `morid-gui` may call `morid`; `morid` may call `morid-engine` and `morid-store`; nothing
below `morid` knows that a user interface exists. This is the one structural rule that the review will hold
you to.

---

## 2. Why this split, in the original's terms

| Original | Problem | Here |
| --- | --- | --- |
| `Context` with 15 references | Nothing is constructible in isolation | Each crate takes only the state it needs, as a struct or a trait |
| `engines::engine::baseEngine` with 57 virtual methods | Every engine overrides a slightly different set of behaviours; the class is the schema | Engines are **data** (`ExtensionSpec`) plus a small number of *named behaviours* selected from an enum |
| `Logger` that parses progress, stores output and writes history | Three concerns in one 986-line class | `morid-engine` emits events; `morid` keeps job state; `morid-store` writes history |
| Tab classes that own processes and queue state | The queue exists three times (basic, batch, playlist) | One scheduler in `morid`; the UI is a view |
| Errors as strings matched by substring (`"Download Failed(ErrorCode=1)"`) | Untestable, breaks on wording changes | `enum Error` with typed variants; the display text is generated, not parsed |
| `QProcess` with `taskkill`/`SIGTERM` | Orphaned children (ffmpeg survives a cancel) | Process group on Unix, Job Object on Windows; cancellation is a tree operation |

---

## 3. The engine model

This is the most important design decision, because it decides whether the project repeats the original's
mistake.

### 3.1 An engine is data

`morid-ext` deserialises the extension JSON into an `ExtensionSpec`. Everything the original expresses as a
virtual method that most engines do not override becomes a field or a table lookup:

| Original virtual method | Rust representation |
| --- | --- |
| `mediaProperties`, `parseJson` | parse rules in the spec (`DumptJsonArguments`, `DefaultListCmdOptions`) handed to a generic JSON-line parser |
| `updateDownLoadCmdOptions`, `updateCmdOptions`, `updateGetPlaylistCmdOptions` | `ArgPolicy` in the spec: which default args apply, where user options go (`DownloadOptionsAsLast`), what is appended |
| `setProxySetting`, `hasConvertArgToEnv`, `convertArgToEnv` | `ProxyMode` in the spec: argument or environment variable, with the exact names |
| `processData`, `Filter`, `filterOutPut` | `ProgressRule` list in the spec: line → event mapping |
| `skipCondition`, `breakShowListIfContains` | `SkipRule` list |
| `parseVersionInfo`, `updateVersionString`, `VersionStringLine/Position` | `VersionRule` in the spec |
| `bundledEngine`, `engineRemovable`, `autoUpdate` | flags in the spec |
| `replaceUndesirableText`, `RemoveText` | `strip_prefixes` / `strip_patterns` |
| `downloadFolder`, `urlFileName`, `foundNetworkUrl` | generic rules, plus *one* `Backend` enum variant when the engine genuinely needs code |

### 3.2 The escape hatch, and its budget

Some engines need behaviour that is not data: yt-dlp's `-o` template rewriting with playlist fields
(`legacy/src/engines/yt-dlp.cpp:1493-1550`), gallery-dl's line-oriented JSON stream, svtplay-dl's file-name
tracking, get-sauce's banner handling. Those live behind a closed enum:

```rust
pub enum BackendKind {
    Generic,        // everything derives from the spec
    YtDlp,          // template rewriting, playlist metadata, comments, subtitles
    GalleryDl,      // line-oriented JSON stream
    SvtPlayDl,      // file name tracking in progress output
    GetSauce,       // banner stripping, progress re-formatting
    Wget,
    Aria2c,
}
```

**Budget: seven variants, and growth is a decision.** Adding an eighth requires a decision record, because
the whole point of the model is that new engines are files, not code. A new engine that only needs different
arguments, a different progress rule or a different URL is an extension JSON file and no Rust change at all.

### 3.3 Consequences

- All 17 bundled extensions must work with `Generic` or one of the six existing variants. This is a Phase 2
  acceptance criterion, not an aspiration.
- The parser can be tested by feeding `ProgressRule`s and a captured line and asserting the event.
- The extension contract becomes a real, documented interface ([extensions.md](extensions.md)) instead of an
  implicit side effect of C++ class structure.

---

## 4. A download, end to end

```
UI / CLI
  │  DownloadRequest { url, engine, options, destination, limits }
  ▼
morid (service)
  ├─ resolve engine: explicit → URL rule → per-tab default → fallback
  ├─ build argv: spec.default_args + user options + engine policy + proxy + cookies + archive
  │    (uses morid-core::option for token parsing, morid-ext for the policy)
  ├─ enqueue: scheduler enforces MaxConcurrentDownloads, assigns job id
  ▼
morid-engine
  ├─ spawn: process group / job object, working directory = download folder, env from spec
  ├─ stream: split stdout+stderr into lines, tolerate partial lines and \r
  ├─ classify: apply ProgressRule/SkipRule → Event
  └─ finish: ExitState { code, signal, cancelled, duration }
  ▼
morid (service)
  ├─ fold events into JobState (queued → running → done/failed/cancelled)
  ├─ on destination/file-name lines: remember what was created
  └─ on finish: update archive, write history, emit JobFinished, run the "after download" command
  ▼
subscribers: morid-gui (render), morid-cli (print / JSON), morid-store (persist)
```

Notes that come from the original's actual behaviour and must not be "improved" away:

- **Line splitting is per engine.** `SplitLinesBy` in the spec decides `\n` only or `\n` and `\r`; aria2c and
  get-sauce need the latter because they redraw a line.
- **Progress lines replace, they do not append.** The original keeps one mutable "current line" per job
  (`Logger::Data::replaceOrAdd`). The event model reproduces this as `Event::Progress` versus
  `Event::Output`.
- **A cancelled job keeps its partial output.** Deleting files on cancel is a setting
  (`DeleteFilesOnCanceledDownload`), not a default.
- **The download folder is created before the process starts** and is the process's working directory; several
  backends write relative paths.

---

## 5. Concurrency and cancellation

- One scheduler owns the queue. Limit from settings (`MaxConcurrentDownloads`, default 4).
- Each job is a `tokio`-free structure: a `std::thread` per running backend is acceptable and simpler than an
  async runtime for a program whose work is dominated by child processes and line parsing. **Decision pending
  in Phase 3** — see [roadmap](roadmap.md) §"Open engineering choices".
- Cancellation sends a termination request to the **process tree**. A cancelled job is not "the process was
  signalled"; it is "no descendant of the job is alive". The mechanism per platform is Phase 3's to choose
  ([tasks/phase-3-one-backend.md](tasks/phase-3-one-backend.md) §3.1 item 6); on Unix a process group is enough,
  and `CommandExt::process_group(0)` creates it without `unsafe` or `pre_exec`.
- On Windows the original shells out to `taskkill`. That is a security and reliability liability (argument
  quoting, missing binary, locale), and job objects replace it — **but job objects only contain what they contain.**
  A process started between `spawn` and `AssignProcessToJobObject` is outside the job, and so is anything it
  created first. Race-free containment needs the child created suspended and assigned before its first instruction
  runs, which is `unsafe` and is why [D-019](decisions.md) exists. A design that assigns after spawn and calls
  itself tree containment is wrong, however well it passes a test that waits for the child to appear first.

---

## 6. Event model

The service layer exposes one stream of events and one set of commands. Everything else subscribes.

```rust
// As delivered in Phase 3 — this is the target model, corrected to what exists.
// The gaps are named in the note below rather than left to be discovered.
pub enum Event {
    JobQueued { job: JobId },
    JobStarted { job: JobId, pid: u32 },              // no argv yet: the CLI adds it from its plan
    JobOutput { job: JobId, stream: Stream, line: String },
    JobProgress { job: JobId, progress: Progress },   // percent, size, speed, eta, line
    JobExited { job: JobId, exit: ExitState },
    JobError { job: JobId, failure: JobFailure },
    JobFinished { job: JobId, state: JobState },
    Notice { job: JobId, key: &'static str, args: BTreeMap<String, String> },
    JobMetadata { job: JobId, meta: MediaMeta },
}

pub enum Command {
    Enqueue(DownloadRequest), Cancel(JobId), CancelAll,
    ListFormats { url, engine }, FetchMetadata { job }, 
    InstallEngine(EngineId), RemoveEngine(EngineId),
    ReloadSettings, ImportData { from: PathBuf },
}
```

Why events rather than callbacks into the UI (the original's approach):

- the console binary and the GUI consume the same stream, so the console cannot drift from the window;
- the whole service is testable by pushing fixture output and asserting the event sequence;
- "show this in the log window" stops being a special case threaded through every layer.

`Notice` carries a translation **key and arguments**, never a pre-formatted string: the service layer does not
know the user's language.

**What Phase 3 delivered, and the three places it is short of the model above.** Recorded here because "the event
stream exists" and "the event stream is the model" are different statements, and the second one is not true yet:

| Gap | Consequence today | Owner |
| --- | --- | --- |
| `JobStarted` carries no `argv`; the CLI re-derives it from its own plan when emitting JSON | the window will have to do the same until the service owns the plan; two sources of one truth | Phase 3 follow-up or Phase 6, whichever touches `JobStarted` first |
| `files` is absent from `JobFinished`; the CLI reports the destination it observed | a GUI cannot show what was written without repeating the CLI's guess | Phase 6 (`JobFinished` gains the field) |
| `EngineVersion`, `EngineInstallProgress`, `EngineListChanged` do not exist | `morid engines --json` reports `engine_version: null`; `--lang` accepts `en` only | Phase 5 for the engine events, Phase 7 for localisation |

None of these is invisible to a user: `engine_version: null` is in the JSON a script reads, and a `finished` event
without `files` is a window that cannot say what it downloaded. They are listed as deviations in the Phase 3 report
for that reason.

---

## 7. Settings and data

- Storage format: INI, at `<appdata>/morid/settings.ini`, overriding `MediaDownloaderAppDataPath` honoured for
  import ([data-compat](data-compat.md)).
- Read once at startup into a typed struct; written back through one function. No module reaches into the
  settings file directly — the original does, from worker threads.
- Paths: `<appdata>/morid/{bin,tmp,themes,archive}` mirroring the original's layout so the import is a copy,
  not a translation.

---

## 8. The interface layer

- Slint, one window with the same five top-level tabs and a Configure tab with the same sub-tabs, because the
  users' habits are part of the compatibility promise ([D-003](decisions.md)).
- The UI holds **view models**, not domain state: a table row is a projection of `JobState`.
- No option-string parsing in the UI. The UI collects text; `morid-core` parses it. The original parses
  quoted option strings in the UI layer, which is why the same parsing is duplicated.
- Translations: gettext `.po` (Slint's supported route), so that the 354 existing messages can be converted
  mechanically and the RTL gap stays the only localisation limitation ([D-002](decisions.md)).

---

## 9. What is deliberately absent

- No plugin system beyond the extension JSON. The original's "extensions" *are* the plugin system.
- No database. The original's JSON files and text archives are the format; a database would break the import
  promise for no gain.
- No async runtime until something proves it is needed.
- No general-purpose dependency-injection container, no service locator. `Context` is what that idea produces
  when it grows unchecked.
- No abstraction over the operating system beyond what the three target platforms genuinely differ on:
  process groups, job objects, app-data paths, and opening a file with the default application.

---

## 10. Open engineering choices (to be settled in the phase that needs them)

| # | Choice | Needed by | Default if nobody decides |
| --- | --- | --- | --- |
| A1 | Threads + channels versus `tokio` for the scheduler | Phase 3 | threads; a process-spawning GUI has little to gain from async |
| A2 | `ureq` versus `reqwest` for `morid-net` | Phase 5 | `ureq`; one blocking HTTP client, smaller tree |
| A3 | Own INI parser versus a crate | Phase 4 | own parser (~150 lines) against a documented subset |
| A4 | gettext `.po` versus bundled message catalogs | Phase 7 | gettext, because it matches Slint and the existing `.ts` files convert mechanically |
