# Roadmap

Related: [features](features.md) · [migration](migration.md) · [architecture](architecture.md) ·
[decisions](decisions.md) · [testing](testing.md)

The roadmap is ordered by **risk**, not by visibility. The riskiest thing in this project is not the interface:
it is that nobody can prove the reimplementation behaves like the program it replaces. So the first phase
produces nothing a user can see, and the phases that follow build the core before the window.

Two rules govern the order:

1. **Nothing is reimplemented before its expected behaviour is recorded.** A feature built against a guess has to
   be rebuilt.
2. **No phase starts before the previous one is accepted.** Acceptance means the phase's criteria are met and
   stated, not that the work "looks done".

Estimates are person-days for one experienced Rust developer, including tests and documentation. They are
planning aids, not commitments.

---

## 1. Decisions already made

| Decision | Choice | Why |
| --- | --- | --- |
| Interface toolkit | Slint | D-001: the only Rust toolkit matching the original's declarative model |
| Qt | not part of the final architecture | D-001: the purpose of the project is to leave C++ behind |
| RTL mirroring | deferred | D-002: the toolkit does not support it; self-implementing costs more than the rest of the UI |
| Compatibility | full — extensions, settings and data files | D-003: otherwise this is a different program, not a replacement |
| Extension versions | `2.x` requirement treated as satisfied, with clear diagnostics | D-004 |
| Windows | 10/11 x86_64 only | D-005: old Windows is a permanent tax, not a one-off task |
| Repository | this fork, C++ reference in `legacy/` | D-006, D-015 |
| Slint licence | royalty-free with attribution | D-007 |
| Binary size | measured and published, not gated | D-008 |
| Console interface | a product feature, two binaries | D-009: it is what makes the behaviour testable |
| Extension source | `extensions/` in this repository | D-010 |
| "Profiles" | not a feature; covered by presets, URL rules, per-engine defaults | D-011 |
| Documentation language | English | D-012 |
| Self-update | notice only | D-013 |
| Backend update endpoints | the backends' own projects, not ours | D-014 |

---

## 2. Phases

Estimates are for the work, not for the waiting.

### Phase 0 — Reference behaviour (no Rust code)

**Deliverable:** fixtures and a comparison procedure, so that later phases can be judged.

- A set of scenarios (about 40): format selection, option strings, cookies, proxy in each mode, playlist range,
  archive, batch file, subtitles, chapters, error cases.
- For each scenario, the **exact argv** the original produces, captured by running it with a debug log.
- For each backend in use, captured raw output: version, list, progress, destination, completion, failure,
  interrupted, non-UTF-8.
- A note per fixture: backend version, capture date, URL class, what was anonymised.
- The comparison script: given a scenario name, print the Rust argv and the golden argv side by side.

**Estimate:** 5–8 days. **Not** a coding task in the Rust sense, and it must not be skipped.

**Status 5 November 2026:** delivered as `fixtures/` (PR #2, commit `8fba615`) — 36 of 40 cases captured with
wrapper transcripts, seven backends of raw output, an anonymised data directory reloaded by the original
program, and a comparator. Goldens were then made independent of the capture host (`b344cdc`), the verifier was
given teeth (`f187923`), and both late findings of round 2 were closed. **Accepted 7 November 2026.** Six review rounds, seven commits from the coder, and the last four rounds were about
one file: `settings.ini`. The result is a Qt INI dialect — values and keys, reader and writer — reproduced from
`qsettings.cpp` and proved by round trip against a port of Qt's own reader, 30 stdlib tests plus a seeded fuzz, with
`VERIFY_OK` over 36 captured cases and 7 backends.

The slowness has one cause worth recording: **the manager specified three wrong mechanisms in a row** (the field name
in a log record, the locale in s20, the escape notation and the quoting rules), and each wrong mechanism cost a round.
A fourth followed in the Phase 2 task. All four had the same origin — a mechanism inferred and written down as fact —
and the answer adopted on 10 November 2026 is not "read the source harder" but the opposite: tasks specify the
**contract** (behaviour, interface, acceptance criterion) and point at the file; the coder reads the old code and
implements what it does. See [tasks/README.md](tasks/README.md), "Rule for the manager".

Nothing in this repository runs automatically, and that is a decision rather than a gap: D-017 defers both checks to
the Phase 1 workflow, which Phase 1's exit criteria require to run them and to report the test count. The four uncaptured cases are real observations,
not omissions: see [migration](migration.md) §6.

**Exit criteria:** a reviewer can take any scenario and see both argument vectors; every backend that Morid will
support has at least one captured output.

---

### Phase 1 — Skeleton

**Implementation status:** accepted and merged (PR #4, `b6c2a58`). Green on all three platforms:
[CI 42461122995](https://github.com/RUdolf1517/Morid/actions/runs/42461122995). The Windows runner cannot initialize
OpenGL, so GUI opening there remains unproven. The Phase 0 suite has **30** tests; CI asserts that count rather than
a number written down here.

**Deliverable:** a workspace that builds, tests and reports size on three platforms, with no features.

- Cargo workspace, the eight crates of [architecture](architecture.md) §1, all empty but compiling.
- **Done ahead of the phase** (4 November 2026): the C++ tree is in `legacy/`, `changelog` is
  `CHANGELOG-legacy.md`, and the 10 inherited CI workflows are retired — see
  [legacy/removed-ci-workflows.md](../legacy/removed-ci-workflows.md).
- New CI: fmt, clippy, test, build, size report, `cargo deny`, `cargo audit` on Linux, macOS, Windows.
- Root documentation in its final structure, in English (D-012); `CHANGELOG.md` restarted with
  `CHANGELOG-legacy.md` holding the original history.
- Licence files reviewed: `LICENSE`, `LICENSE.txt`, `GPLv2`, `GPLv3`, `THIRD-PARTY.md`, and a note on which is
  normative (audit defect D7).

**Estimate:** 5–7 days. **Exit criteria:** green CI on three platforms; `cargo build --offline` works;
`morid --version` prints a version; size is reported; **and the same workflow runs the Phase 0 checks** —
`python3 -m unittest tools.capture.test_qt_ini_roundtrip` and `tools/capture/verify_fixtures.py` — because D-017
deferred them to here rather than dropping them.

---

**Status 8 November 2026:** delivered in PR #4 (`b6c2a58`) and green on all three platforms — run
[42461122995](https://github.com/RUdolf1517/Morid/actions/runs/42461122995). Eleven of the twelve criteria met as
written; two of those by an equivalent rather than literally (criterion 8 used a disallowed-licence probe crate;
criterion 10 builds the CLI for musl instead of proving a C-toolchain-free GUI build). Two deviations accepted and
recorded in [tasks/phase-1-skeleton.md](tasks/phase-1-skeleton.md) §7a: `deny.toml` stops failing on transitive
*unmaintained* advisories — vulnerabilities still fail — and Slint is pinned `~1.13` because the specification's
`version = "1"` resolves to an edition-2024 release the pinned 1.85 compiler cannot build. Windows GUI launch stays
unproven: the hosted runner has no `glCreateShader`.

The workflow now also runs the Phase 0 checks and asserts their test count, which is what D-017 deferred here.

### Phase 2 — Domain and extension schema

**Status: in progress, not submitted as complete.** The Phase 2 branch implements domain values,
extension reading/resolution, the three filename transformations, progress expressions and the
`engines` CLI. `PlaylistRange` is blocked pending clarification: the original forwards the
playlist field as option tokens (`legacy/src/playlistdownloader.cpp:1061–1069`) rather than
validating a numeric range. Its refusal grammar must not be invented. The original 10–14
person-day task estimate is not yet replaced by a completed-work estimate.

**Deliverable:** all 17 bundled extensions load, validate and resolve to a command, with no process started.

- `morid-core`: URL handling, option-token parsing, playlist ranges, version comparison, `Error`.
- `morid-ext`: the extension schema (40 keys in the shipped files — see [extensions](extensions.md) §1), the validation order of [extensions](extensions.md) §8, command selection per
  OS/architecture, `ControlJsonStructure` evaluation, unknown-key warnings, the `ExtraArguments` defect handled
  as documented.
- Round-trip: every bundled file parses and reports what it resolved to.

**Estimate:** 10–14 days. **Exit criteria:** the negative fixtures of [testing](testing.md) §2 pass (I4, I10);
no `unsafe`; no process is spawned by any test.

---

### Phase 3 — One backend, end to end

**Deliverable:** `morid list` and `morid download` work against a real yt-dlp, and the argv matches the golden
fixtures.

- `morid-engine`: spawn, stream, split lines, classify, emit events, cancel a process tree.
- `morid`: the service layer, the queue, the concurrency limit, job state.
- `morid-cli`: `list`, `download`, `--dry-run`, `--json`, exit codes.
- The fake backend for tests ([testing](testing.md) §3).

**Estimate:** 12–16 days. **Exit criteria:** `--dry-run` argv equals the golden fixture for every Phase 0
scenario that uses yt-dlp; a cancelled job leaves no descendant; exit codes of [cli](cli.md) §3 are covered.

This is the phase that answers the question the whole project rests on: *can the Rust version produce the same
command line as the C++ version?* If the answer is no, everything after this point changes.

---

### Phase 4 — User data

**Deliverable:** a real Media Downloader data directory imports without loss, and the round trip works.

- `morid-store`: settings (INI), presets, URL rules, per-engine defaults, subscriptions, archive handling
  (**never rewriting it**), history (tolerant read of the concatenated-JSON file).
- `morid import` / `morid export`, with a report.
- The round-trip test of [data-compat](data-compat.md) §6.

**Estimate:** 10–14 days. **Exit criteria:** F-68 done; unknown settings keys preserved; the original program can
read what Morid exports.

---

### Phase 5 — Network, backend installation, updates

**Deliverable:** backends install and update themselves with integrity checking, from their own projects.

- `morid-net`: release metadata, download, size + SHA-256 verification, archive unpacking with path-traversal
  refusal.
- Extension installation/removal, supporting engines (ffmpeg, deno, quickjs-ng, python, bsdtar).
- The endpoint split of [D-014](decisions.md) — backends from their own projects, the application's own update
  check pointing at Morid, and **no** extension download at all (D-010).

**Estimate:** 10–14 days. **Exit criteria:** F-34, F-35, F-38 done; a corrupted download leaves the old binary
intact; no request is made to a project that is not a backend's own release page.

---

### Phase 6 — Remaining backends

**Deliverable:** parity of the command line and the parsed events for every supported backend.

- gallery-dl, you-get, svtplay-dl, get-sauce, wget, aria2c, plus the named `BackendKind` variants they need.
- Every `BackendKind` variant added here is counted against the budget of seven
  ([architecture](architecture.md) §3.2); an eighth requires a decision record.

**Estimate:** 15–20 days. **Exit criteria:** F-30…F-39 done; each backend has fixtures; the budget is respected.

---

### Phase 7 — Interface

**Deliverable:** a window that does what the original's window does, minus the documented gaps.

Order inside the phase, because each depends on the previous:

1. One window, tab bar, settings plumbing, language selection, theme.
2. Basic Downloader: URL, format table, option field, progress, cancel.
3. Batch Downloader: table, thumbnails, intervals, chapters, file import, save.
4. Playlist Downloader: playlist table, ranges, archive interaction, subscriptions.
5. Library: browse, rename, delete, sort.
6. Configure: general options, per-engine defaults, presets, URL manager, proxy, extensions, UI scale, import.
7. Tray, notifications, clipboard monitoring, single instance, window geometry.

Translations are done per screen as it is built, not at the end: a screen without its 18 languages is an
unfinished screen.

**Estimate:** 30–45 days. **Exit criteria:** every P0 UI feature `done`; manual procedures M1–M7 of
[testing](testing.md) §6 performed and recorded; the RTL limitation stated in the interface's own About text.

---

### Phase 8 — Parity, packaging, release

**Deliverable:** version 1.0: installable on the three platforms, import works, gaps documented.

- Packaging per [building](building.md) §6, release gates per [releases](releases.md) §2.
- Compatibility sweep: every P0 and P1 row in [features](features.md) with a passing test.
- Release notes written for a user coming from Media Downloader.

**Estimate:** 15–20 days, excluding external delays (code signing, notarisation, package reviews).

---

## 3. Totals and what they mean

| Phase | Estimate | Cumulative |
| --- | --- | --- |
| 0 Reference behaviour | 5–8 | 5–8 |
| 1 Skeleton | 5–7 | 10–15 |
| 2 Domain + extensions | 10–14 | 20–29 |
| 3 One backend | 12–16 | 32–45 |
| 4 User data | 10–14 | 42–59 |
| 5 Network + installation | 10–14 | 52–73 |
| 6 Remaining backends | 15–20 | 67–93 |
| 7 Interface | 30–45 | 97–138 |
| 8 Parity + release | 15–20 | 112–158 |

**112–158 person-days is not a schedule.** It excludes review time, the owner's decisions, and the fact that the
estimates were made before a line of code existed. The number's usefulness is in its shape: the interface is
roughly a third, not the bulk, and the parts that make it trustworthy are the other two thirds.

The first usable console release (after Phase 6) is roughly two thirds of the total. That is the milestone worth
watching: at that point the risky part is done and the interface is "just" a lot of work.

---

## 4. Deliberately deferred

Each item is a documented gap, not a forgotten wish.

| Item | Why deferred | Where it would go |
| --- | --- | --- |
| RTL layout mirroring | toolkit support absent (D-002) | when Slint ships interface direction, or if user demand justifies a custom layer |
| Windows 7/8 | permanent cost for a shrinking audience (D-005) | never, unless the decision is revisited |
| Self-update | risk of a program that cannot start (D-013) | after 1.0, with a security review |
| Named download profiles | covered by three existing mechanisms (D-011) | after parity, as an improvement |
| Automatic subscription checking | new feature, not a migration (F-26) | after parity |
| 32-bit builds | see [platforms](platforms.md) §1 | not planned |
| Binary size gates | owner chose reporting over blocking (D-008) | revisit at 1.0 |
| Universal macOS binary | doubles the largest artefact | if users ask |
| Fuzzing, property tests, coverage gates | see [testing](testing.md) §5 | if Morid ever parses a file format |
| A database for history | would break the round-trip promise | never |

---

## 5. Open engineering choices

These are implementation choices, not product decisions; each is settled in the phase that needs it.

| # | Choice | Phase | Default |
| --- | --- | --- | --- |
| A1 | Threads + channels versus `tokio` | 3 | threads |
| A2 | `ureq` versus `reqwest` | 5 | `ureq` |
| A3 | Own INI parser versus a crate | 4 | own parser |
| A4 | gettext versus bundled catalogs | 7 | gettext |
| A5 | Job object strategy on Windows | 3 | `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` |

---

## 6. How this document is kept honest

- A phase's status is written here only after its exit criteria are met, with the evidence (a CI run, a test
  name, a report).
- Estimates are not retro-fitted. If a phase takes three times the estimate, the number stays and the variance is
  noted, because a roadmap that quietly matches reality teaches nobody anything.
- When a decision changes, the phase affected is named explicitly in [decisions](decisions.md), so this document
  can be updated in the same commit.
