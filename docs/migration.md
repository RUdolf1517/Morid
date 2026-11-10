# Migration plan

Related: [features](features.md) · [roadmap](roadmap.md) · [architecture](architecture.md) ·
[testing](testing.md) · [audit](audit-original.md)

How behaviour moves from the C++ program in `legacy/` to the Rust program, and — more importantly — how
anybody can tell that it moved correctly.

The central problem: **the original has no tests**. There is no expected output to compare against, no
specification beyond the code, and no way to run a regression suite against the C++ version. Therefore the
migration is not "port modules", it is:

> **record behaviour → prove the recording → reimplement against the recording → prove equivalence.**

Everything below is in service of that sentence.

---

## 0. Where the two programs live

The C++ program is in `legacy/` and is read-only ([D-006](decisions.md), [D-015](decisions.md),
[../legacy/README.md](../legacy/README.md)). The Rust program will be in `crates/`. Neither build invokes the
other: the root build never touches Qt, and CI never compiles `legacy/`.

The move into `legacy/` was made on 4 November 2026 with `git mv`, so history follows the files. It changed no
line of C++. The one thing the move broke — `CMakeLists.txt` copying `changelog` into a macOS bundle — is
recorded with its workaround in [../legacy/removed-ci-workflows.md](../legacy/removed-ci-workflows.md).

Everything below refers to files by their path inside `legacy/`.

---

## 1. Why not port the C++ structure

A direct translation of the C++ architecture into Rust would produce a program with:

- a `Context`-like object holding references to everything, so nothing can be constructed in isolation;
- a base "engine" trait with 57 methods, most of which no engine overrides;
- an error type that is a string, matched by substring;
- UI classes that own processes and queue state, so the queue exists three times;
- no unit tests, for the same reasons the original has none.

That is not a rewrite. It is a transliteration that inherits every structural defect and adds the cost of two
languages having been involved. [Architecture](architecture.md) §2 maps each defect to its replacement.

What *is* carried over deliberately:

| Carried over exactly | Because |
| --- | --- |
| The extension JSON schema, all 40 keys used by the shipped files | It is a public contract; users and third parties have files ([extensions](extensions.md)) |
| User data file formats | A returning user must find everything intact ([data-compat](data-compat.md)) |
| The generated argument vectors | The backends are the same; a different argv is a different download |
| The UI structure: five tabs, the same sub-tabs | Habit is part of compatibility |
| The 18 languages and their messages | Translation work should not be redone |
| The archive file semantics | It is what prevents re-downloading everything |
| The data directory layout | Import is a copy, not a translation |

---

## 2. Unit of migration

The unit is a **feature row** in [features](features.md), not a file. A file-oriented migration ("port
`engines.cpp`") cannot be verified, because a file has no observable behaviour; a feature can be.

Each feature goes through six states, and the state is recorded in the features table:

```
not started
   → reference captured      (argv and/or output captured from the original)
   → implemented             (Rust code exists)
   → equivalent              (tests compare against the capture and pass)
   → wired                   (reachable from the CLI and/or the UI)
   → done                    (acceptance criterion in CI, documentation updated)
```

A feature that is `implemented` but not `equivalent` is not progress; it is a hypothesis.

---

## 3. Capturing the reference

### 3.1 The argument vector

The most valuable artefact of the whole migration. For each scenario:

```sh
# what the original runs, captured from its debug log
yt-dlp --newline --ignore-config --no-playlist -o "%(title).200s-%(id)s.%(ext)s" -f 22 <url>

# what Morid runs
morid download <url> -f 22 --dry-run
```

Comparison rules:

- **Argument order matters.** The original appends user options, then engine defaults, then extra options; a
  backend that reads the last occurrence of a flag behaves differently if the order changes.
- **Quoting is not part of the comparison** — the fixture records argv as a list of arguments, not as a shell
  string. This is deliberate: a shell string comparison would hide quoting bugs, which are a security issue
  ([SECURITY](../SECURITY.md)).
- A difference is either a defect or a decision. A decision goes into [decisions](decisions.md) and the fixture
  is annotated with it. No third category.

### 3.2 The output stream

For each backend, captured raw bytes with provenance ([testing](testing.md) §4):

| Capture | Used for |
| --- | --- |
| `--version` output | `VersionStringLine` / `VersionStringPosition` handling (F-32) |
| format list JSON | the format table (F-01) |
| a normal download's output | progress, destination, completion (F-03, F-04) |
| a playlist listing | playlist parsing (F-20) |
| a failure | error classification; the original's codes 1, 2, 4 and the "unknown" case |
| an interrupted download | the cancelled state and partial files (F-05) |
| a non-UTF-8 stream | the encoding setting (F-56) |
| an output with carriage returns | `SplitLinesBy` handling (aria2c, get-sauce) |

### 3.3 The user data

A real Media Downloader data directory, anonymised: paths, engine names in cookie keys, and any personal URL
replaced. It becomes the fixture for F-68 and the round trip of [data-compat](data-compat.md) §6.

### 3.4 What cannot be captured, and what we do instead

| Behaviour | Why it cannot be captured | Substitute |
| --- | --- | --- |
| Network behaviour of a site | changes daily | fixture output plus manual procedure M7 |
| Backend behaviour under load | environment-specific | not tested; stated as a gap |
| Visual rendering | needs a human | manual procedures M1–M3 |
| Timing of the clipboard monitor | racy by nature | tested by injecting the event, not by waiting |
| The original's self-update path | Windows-only, mutates directories | not carried over (D-013); no substitute needed |

---

## 4. Order of migration and why

The order in [roadmap](roadmap.md) §2 is: reference → skeleton → domain and extensions → one backend end to end →
user data → network → remaining backends → interface → release.

Three deliberate choices inside that order:

**Extensions before backends.** The extension schema is the boundary between "what the user configured" and
"what runs". Getting it wrong later means rewriting every backend. Getting it right first makes the remaining
backends mostly data ([architecture](architecture.md) §3).

**One backend end to end before any second backend.** yt-dlp is the default and the most complex; it exercises
argv construction, streaming parse, template rewriting, cancellation and archive. A second backend added before
the first is complete hides design problems behind "we have two backends now".

**User data before network.** Backend installation is the only part that must touch the network, and it is the
part most likely to be blocked in a development environment. Importing user data, by contrast, is local,
deterministic and testable, and it makes the program useful to a real user earlier.

---

## 5. Behaviour differences that are intentional

Each of these changes behaviour relative to the original. They are allowed because a decision says so, and each
is listed in `CHANGELOG.md` when it ships. Any *other* difference is a defect until proven otherwise.

| # | Difference | Decision | Effect on the user |
| --- | --- | --- | --- |
| M1 | `RequiredMinimumVersionOfMediaDownloader: 2.x` is treated as satisfied | D-004 | existing extensions keep working; a real incompatibility surfaces as a clear error instead of a refused extension |
| M2 | `ExtraArguments*` in `ytdl-patched.json` is not honoured; the file is corrected to `ExtraOptions*` | [extensions](extensions.md) §4.1 | that extension now adds its intended JavaScript-runtime arguments, which it previously did not |
| M3 | Extensions are not downloaded from a pinned upstream commit | D-010 | the bundled set is the set; an update requires a Morid release |
| M4 | Cancellation kills the process tree | architecture §5 | no orphaned ffmpeg after a cancel |
| M5 | Windows 7/8 not supported | D-005 | those users stay on the original |
| M6 | Self-update replaced by a version notice | D-013 | update is manual or through a package manager |
| M7 | Errors are typed, not substring-matched | architecture §2 | error messages change wording; the CLI exit code is the stable interface |
| M8 | RTL layout not mirrored | D-002 | Arabic renders correctly but the layout stays LTR |
| M9 | `downloadHistory.json` is written as valid JSON in Morid's own directory; the original's file is left untouched | data-compat §5.1 | two files exist with different shapes; the original's remains usable |
| M10 | Automatic subscription checking exists (F-26) | features §3 | new feature, not parity — must not be presented as "was always there" |
| M11 | *(withdrawn — replaced by D-016)* `settings.ini` was to be read and written as UTF-8. The premise was wrong: the original writes Qt-escaped ASCII, not locale-encoded bytes, and writing UTF-8 would break the original's ability to read the file. | [decisions](decisions.md) D-016 | none: Morid now reads and writes the original's dialect |

---

## 6. Defects in the original that are not reproduced

These are recorded so that nobody "restores compatibility" by rebuilding a bug. Each is either fixed or
deliberately left alone with a reason.

| # | Original defect | Where | Morid |
| --- | --- | --- | --- |
| D1 | A release without a `digest` is considered valid | `legacy/src/engines.h:344-350` | refuse an update with no hash when one is expected |
| D2 | Errors recognised by substring match | `legacy/src/playlistdownloader.cpp:1791-1795` | typed errors |
| D3 | 57 virtual methods as the engine schema | `legacy/src/engines.h` | data + seven named variants |
| D4 | No automated tests | repository-wide | fixtures and CI from Phase 0–1 |
| D5 | Menu lists 10 extensions; 17 ship | `legacy/src/configure.cpp:1231` | the whole bundled set is reachable |
| D6 | `ExtraArguments*` silently ignored | `legacy/src/engines.cpp:1052` | warning, and the file is corrected |
| D7 | Four licence files, none identified as normative | repository root | `LICENSES.md` note names the normative one |
| D8 | `QSettings` read from worker threads | 15 call sites | settings read once, passed as a value |
| D9 | Empty `//?????` completion handlers for extension download | `legacy/src/configure.cpp:983` | download has an outcome and reports it |
| D10 | Self-update renames directories at startup | `legacy/src/utility.cpp:1358` | not carried over |

### 6.1 Observed during Phase 0 capture

Behaviour found by running the original, which no document predicted. Recorded here because it was previously
assumed to be the opposite, and because the fixture records the real thing.

| # | Observation | Case | Consequence for Morid |
| --- | --- | --- | --- |
| O1 | An unknown extension **name** does not produce an empty command. The original rejects the name and downloads with the default engine. | s35 | "Unknown engine" is a warning plus a fallback to the default, not a hard failure. State which engine is chosen and whether the user is told before or after the download starts; the fixture pins the original's choice. |
| O1b | An extension with the `Name` field **missing** is not rejected: the original loads it and executes its command. The captured argv is the URL alone. | s36 | "Missing required key" is a different path from "unknown name", and today it is not a validation failure at all. Decide deliberately whether Morid validates `Name` and what it does when it is absent; record the answer in [extensions](extensions.md) §2. |
| O2 | An extension whose `Cmd` exists only for another architecture is likewise replaced by a working engine. | s26 | The selection table is a fallback chain, not a lookup that can fail. [extensions](extensions.md) §3 must describe the chain. |
| O3 | Adding a URL to batch mode issues metadata commands before the download, so a scenario's transcript holds more than the command under test. | s10, s16, s18, s19 | Any argv test must name the invocation it asserts on, and the fixtures do so explicitly. Metadata preview is switched off in those captures rather than filtered afterwards. |
| O4 | On Linux the shipped `Generic` branch of the you-get extension invokes `you-get` directly; the interpreter invocation exists only in the Windows branch. | s33 | The per-OS command tables are not interchangeable and cannot be reduced to one. Not captured as a golden; the observed command is kept. |
| O5 | `AutoUpdate` produces no external process at all — updating a backend is done in-process through the network layer. | s28 | There is no argv to reproduce for updates; the equivalent behaviour is "fetch release metadata, verify, replace the binary" ([F-34](features.md)). |
| O6 | Subtitles and comments are fetched with `--print`/`--get-comments` and `--no-download`, not by a separate list command. | s16, s18 | The metadata reader is the same code path as the download planner; it must not be modelled as a separate "probe" subsystem. |
| O7 | `settings.ini` is not UTF-8: the original writes Qt-escaped ASCII (`\x<hex>` per non-ASCII code unit) because it never calls `setIniCodec` on a Qt 5 `QSettings`. Under Qt 6 the same file is written as UTF-8, so the two builds of the original disagree with each other. | s20 (the capture also exposed a separate, still-open question about that case) | Morid reads and writes the Qt 5 dialect — D-016. A general-purpose INI crate cannot do this. An earlier version of this row blamed the capture's locale; that explanation was wrong and is corrected here. |

The distinction between §5 and §6: §5 changes something users see; §6 fixes something that was broken. Both are
documented, because "we silently improved it" is how a replacement becomes untrustworthy.

---

| O10 | The extension JSON is **not** the whole story for three engines: `yt-dlp-nightly`, `yt-dlp-ffmpeg` and `yt-dlp-aria2c` are transformed by hand-written C++ (`legacy/src/engines/yt-dlp.cpp`) after `yt-dlp.json` is loaded, dispatched by exact file name (`engines.cpp:244-258`). | `extensions/` | Honest limitation of D-003: "full compatibility" here means reproducing three functions in Rust, not reading more JSON. A third-party file cannot be a nightly variant unless it is named exactly that. Recorded so that "the JSON schema is the contract" is not overclaimed — see [extensions](extensions.md) §1. |
| O9 | `UserName` / `Password` are stored as plain strings inside three extension files. | `extensions/` | Inherited, not introduced. Morid treats them as opaque, never logs them (I7), and does **not** move them to a keyring: an extension file written for the original must keep working. See [extensions](extensions.md) §4.2. |
| O8 | The "Resetting download folder to default" message **never appears** in the application log, even when the reset happens: `settings::downloadFolder()` passes a default-constructed `sLogger` whose `add()` is a no-op without a `Logger*` (`legacy/src/settings.cpp:1631`). The reset is observable only through the resulting working directory and the rewritten `DownloadFolder` value. | s20, `f187923` | Do not use the log line as evidence of anything. Morid should log its own equivalent properly — this is one of the defects not to reproduce (D-016 note). |

## 7. Equivalence, precisely defined

"Same behaviour" needs a definition that can be checked, so:

| Aspect | Equivalent means |
| --- | --- |
| Arguments | the argv list is identical, in the same order, for the captured scenarios |
| Files | the same files appear in the download folder with the same names |
| Archive | the same entries are added; the existing file is never rewritten |
| Progress | the same sequence of (percent, size, speed, eta) for a replayed output |
| Errors | the same classification and the same CLI exit code; wording may differ |
| Settings | the same settings are honoured with the same defaults |
| Extension files | the same files load and resolve to the same command |
| User data | imported without loss; round trip readable by the original |
| Interface | the same screen does the same thing; pixel layout is **not** promised |

Not promised: identical log wording, identical window layout, identical timing, identical handling of a backend
that changed since the fixture was captured.

---

## 8. Risks specific to this migration

| Risk | Consequence | Mitigation |
| --- | --- | --- |
| Fixtures captured from one backend version | a backend update silently breaks parsing | nightly manual diff job ([testing](testing.md) §4.3); rules are data, so the fix is a JSON edit |
| The schema has undocumented corners | an extension in the wild behaves differently | 17 files plus hostile mutations in tests; unknown keys preserved and warned |
| "While I am here" improvements | scope explodes, compatibility erodes | any difference needs a decision record (§5, §6) |
| Interface phase underestimated | the risky part is done and then stalls | interface is last precisely because it is the largest *known* known |
| The owner's decisions get lost in conversation | the coder implements an older plan | `docs/decisions.md` is the source of truth; every task references it |

---

## 9. Definition of "migration complete"

Migration is complete when, for every P0 and P1 row of [features](features.md):

1. the acceptance criterion is a test that runs in CI on three platforms;
2. the argv matches a golden fixture for every scenario that produces a command line;
3. the behaviour differences, if any, are in §5 with a decision record;
4. the documentation that describes it matches the implementation;
5. a user coming from Media Downloader can import their data and continues without re-downloading anything.

Everything after that is maintenance, and maintenance is a different document.
