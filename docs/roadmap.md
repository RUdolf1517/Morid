# Roadmap

Related: [features](features.md) · [migration](migration.md) · [architecture](architecture.md) ·
[decisions](decisions.md) · [testing](testing.md) · [tasks](tasks/README.md)

The roadmap is ordered by **risk**, not by visibility. The riskiest thing in this project is not the interface:
it is that nobody can prove the reimplementation behaves like the program it replaces. So the first phase
produces nothing a user can see, and the phases that follow build the core before the window.

Three rules govern the order:

1. **Nothing is reimplemented before its expected behaviour is recorded.** A feature built against a guess has to
   be rebuilt.
2. **No phase starts before the previous one is accepted.** Acceptance means the phase's criteria are met, checked
   by running them, and written down — not that the work "looks done".
3. **A phase is a contract, not a transcription.** The task states what must be true afterwards and where the
   answer lives; the coder establishes the mechanism from `legacy/`. This is [D-018](decisions.md), and it exists
   because the alternative was tried and produced four wrong mechanisms in a row (see §6).

Estimates are person-days for one experienced Rust developer, including tests and documentation. They are
planning aids, not commitments, and they are **not retro-fitted** when a phase runs long.

---

## 1. Where the project stands, 10 November 2026

| Phase | Estimate | Status | Evidence |
| --- | --- | --- | --- |
| 0 Reference behaviour | 5–8 | **accepted 7 Nov 2026** | PR #2 (`8fba615`); 36 of 40 cases, 7 backends, `VERIFY_OK` |
| 1 Skeleton | 5–7 | **accepted 8 Nov 2026** | PR #4 (`b6c2a58`), [CI 42461122995](https://github.com/RUdolf1517/Morid/actions/runs/42461122995) |
| 2 Domain and extensions | 10–14 | **accepted 10 Nov 2026** | PR #8 (`fe97a75`), [CI 42881799249](https://github.com/RUdolf1517/Morid/actions/runs/42881799249) |
| 3 One backend, end to end | 12–16 | **next — task not yet written** | — |
| 4–8 | 10–20 each | not started | — |

**What exists in code:** eight crates that compile on three platforms; a Qt-INIdialect settings reader/writer with
30 tests and a seeded fuzz; `morid-core` (URL, format, option tokens, playlist-range tokens, versions, typed errors);
`morid-ext` (17 extensions read, validated in the documented order, three thin-file transformations, `ControlJsonStructure`,
command selection, warnings); `morid engines` and `--json`. **29 tests** in four suites. Nothing spawns a process
and nothing touches the network.

**Days spent: ≈12** against estimates of 20–29 for the same three phases. The gap is not speed — these three
phases are readers and scaffolding, and the expensive ones are 3–8. The variance is recorded here rather than
retro-fitted into the table.

---

## 2. Decisions already made

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
| `settings.ini` dialect | Qt 5 escaping reproduced exactly | D-016 |
| Phase 0 checks | manual until Phase 1's workflow | D-017, delivered |
| How tasks are written | contract + source pointer; mechanism from `legacy/` | D-018 |

---

## 3. How a phase runs

The same shape every time, so that "accepted" means something:

1. **Task written** in `docs/tasks/` by the manager, as a contract (§[D-018](decisions.md)): goal, what to produce,
   constraints, acceptance criteria, what to report, which documents change.
2. **Work on its own branch**, PR opened, CI green on three platforms.
3. **Hand-back** with output, not summaries: test names, commands, sizes, known problems, and every place where the
   task contradicted the code.
4. **Review by breaking it**: read the code, not the report; re-run the criterion the phase exists for; try one
   mutation that should fail. Acceptance is a comment on the PR with a criterion-by-criterion table.
5. **Documents updated by the manager**, then merged with a merge commit — the history of decisions is the point.

A phase that ends with an unproven criterion ends with a **recorded gap**, not with a green tick. Phase 2 shipped
one: `url_round_trips_byte_for_byte` tests invented URLs rather than the fixture shapes, and it is written down as
unmet in [tasks/phase-2-domain.md](tasks/phase-2-domain.md) §3.4.

---

## 4. Phases

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
given teeth (`f187923`), and both late findings of round 2 were closed. **Accepted 7 November 2026.** Six review
rounds, seven commits from the coder, and the last four rounds were about one file: `settings.ini`. The result is a
Qt INI dialect — values and keys, reader and writer — reproduced from `qsettings.cpp` and proved by round trip
against a port of Qt's own reader, 30 stdlib tests plus a seeded fuzz, with `VERIFY_OK` over 36 captured cases and
7 backends.

The slowness had one cause: **the manager specified wrong mechanisms as fact**, three times in Phase 0 and once
more in the Phase 2 task. The answer adopted on 10 November is not "read the source harder" but the opposite —
tasks state the **contract** and point at the file. See [tasks/README.md](tasks/README.md), "Rule for the manager".

The four uncaptured cases are real observations, not omissions: see [migration](migration.md) §6.

**Exit criteria:** a reviewer can take any scenario and see both argument vectors; every backend that Morid will
support has at least one captured output.

---

### Phase 1 — Skeleton

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

**Estimate:** 5–7 days.

**Status 8 November 2026:** accepted in PR #4 (`b6c2a58`), green on all three platforms — run
[42461122995](https://github.com/RUdolf1517/Morid/actions/runs/42461122995). Eleven of the twelve criteria met as
written; two of those by an equivalent rather than literally (criterion 8 used a disallowed-licence probe crate;
criterion 10 builds the CLI for musl instead of proving a C-toolchain-free GUI build). Two deviations accepted and
recorded in [tasks/phase-1-skeleton.md](tasks/phase-1-skeleton.md) §7a: `deny.toml` stops failing on transitive
*unmaintained* advisories — vulnerabilities still fail — and Slint is pinned `~1.13` because the specification's
`version = "1"` resolves to an edition-2024 release the pinned 1.85 compiler cannot build. Windows GUI launch stays
unproven: the hosted runner has no `glCreateShader`.

**Exit criteria:** green CI on three platforms; `cargo build --offline` works; `morid --version` prints a version;
size is reported; **and the same workflow runs the Phase 0 checks** — that is what D-017 deferred here, and it
runs.

---

### Phase 2 — Domain and extension schema

**Deliverable:** all 17 bundled extensions load, validate and resolve to a command, with no process started.

- `morid-core`: URL handling, option-token parsing (including the playlist range field, which is option text and not
  a range grammar — [extensions](extensions.md) §5.4), version comparison, `Error`.
- `morid-ext`: the extension schema (40 keys in the shipped files — [extensions](extensions.md) §1), the validation
  order of §8, command selection per OS/architecture, `ControlJsonStructure` evaluation, unknown-key warnings, the
  `ExtraArguments` defect handled as documented.
- Round-trip: every bundled file parses and reports what it resolved to.

**Estimate:** 10–14 days; it took one working session after the specification settled, because the phase is a
reader, not a downloader.

**Status 10 November 2026:** accepted in PR #8 (`fe97a75`), green on Linux, Windows and macOS —
[CI 42881799249](https://github.com/RUdolf1517/Morid/actions/runs/42881799249). 29 tests in four suites, 17
extensions resolved and 17 JSON objects on every platform, both crate-boundary greps empty, no `unsafe`, no process
spawned by any test. The macOS job ran on `macos-26-intel`: GitHub could not acquire a hosted ARM64 runner
(`The job was not acquired by Runner`), so aarch64 command selection is proved by the resolver tests, not by a
native run — the difference is stated rather than glossed.

**One criterion not met.** `url_round_trips_byte_for_byte` rounds-trips invented URLs (`example.test`, `magnet:`,
an IDN host) instead of the shapes the Phase 0 fixtures contain. It proves the type is transparent and nothing
about the URLs Morid will receive. Recorded as a gap in [tasks/phase-2-domain.md](tasks/phase-2-domain.md) §3.4 and
in [testing](testing.md) §2; closing it is the first item of Phase 3.

**Specification errors this phase exposed.** Four in one task — three disproved by the coder from `legacy/` before
writing code, one by the manager at review: thin files dispatch by exact file name, not by inheriting from a family;
`ControlJsonStructure` does support `||` and a connector-less `lhs`; the playlist range field is free text that
neither program validates; and `yt-dlp-aria2c` publishes no transformation at all, so `Cmd` is unchanged — an
**absent** argument, which no "contains" assertion would have caught. All four are in
[tasks/README.md](tasks/README.md) with their cost.

**Exit criteria:** the negative fixtures of [testing](testing.md) §2 pass (I4, I10); no `unsafe`; no process is
spawned by any test. **All met.**

---

### Phase 3 — One backend, end to end

**Status: review candidate, not accepted.** The service queue/cancellation/stream increments are followed by
`list`/`download`/batch adapters, the JSON settings override seam, fifteen in-scope CLI dry-run goldens, version-1
JSONL and executable-adjacent → per-user extension lookup. Offline fixture-process tests and the separate real
loopback-HTTP yt-dlp acceptance are distinct CI steps. Evidence and explicit interface limitations are in PR #12;
this paragraph does not promote the phase to accepted. The original 12–16-day estimate below is retained: the
work spanned multiple implementation/review rounds, but active engineering days were not measured, so an actual
person-day figure cannot honestly be substituted.

**Deliverable:** `morid list` and `morid download` work against a real yt-dlp, and the argv matches the golden
fixtures.

- `morid-engine`: spawn, stream, split lines (per `SplitLinesBy`), strip `RemoveText`, drop `SkipLineWithText`,
  classify, emit events, cancel a **process tree**.
- `morid`: the service layer, the queue, the concurrency limit, job state.
- `morid-cli`: `list`, `download`, `--dry-run`, `--json`, exit codes 0/2/3/5/6.
- The fake backend for tests ([testing](testing.md) §3), including one that spawns a child and ignores the first
  signal, so I5 is testable at all.

**Decided before it started:** [D-019](decisions.md) — cancelling a process tree cannot be written without `unsafe`,
so `morid-engine` drops to `#![deny(unsafe_code)]` with two narrowly allowed platform modules. Every other crate
keeps `forbid`. This is the first requirement in the project that changed a standing rule instead of being bent to
fit it.

**Carried in from Phase 2:** `url_round_trips_byte_for_byte` must use the fixture URLs
(`fixtures/scenarios/*/argv.json`), and the range-field token split must be checked against `s07` — both are named
tests in [Task 4](tasks/phase-3-one-backend.md) §3.6, and both are acceptance criteria, not wishes.

**Estimate:** 12–16 days. **Exit criteria:** `--dry-run` argv equals the golden for every scenario **Task 4 §5 puts
in scope** — 15 yt-dlp cases, not all 40, because the rest need the playlist planner, the store, the proxy
construction or another backend, and pretending otherwise would be a criterion that cannot be met in this phase —
a cancelled job leaves no descendant on Linux and Windows; every documented exit code is produced by a test; no
test touches the network; a release binary finds `extensions/` outside a checkout (A6).

This is the phase that answers the question the whole project rests on: *can the Rust version produce the same
command line as the C++ version?* If the answer is no, everything after this point changes.

**Risks, named now:**

| Risk | Mitigation |
| --- | --- |
| Line splitting differs from QProcess's `readyRead` chunking, so a progress line arrives in two pieces | the fake backend writes in hostile chunk sizes, including mid-UTF-8-sequence splits |
| Cancellation leaves an orphan on Windows | Job objects with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (A5), tested with the child-spawning fake backend. **Known sharp edge:** assign-after-spawn is not race-free — a process started between `spawn` and `AssignProcessToJobObject` sits outside the job. The phase must either close that window (`CREATE_SUSPENDED` + assign + resume, which needs the `unsafe` D-019 allows) or state the residual race and test it; Phase 3's report on the primitive as delivered says assign-after-spawn, which is why this risk is still open |
| The golden fixtures do not cover an option combination the CLI offers | the planner is built from `ArgPolicy` and the extension data, not from a scenario table; an option combination with no golden is **reported**, and the review deliberately dry-runs one (Task 4 §8) |

---

### Phase 4 — User data

**Status:** implementation in progress; not accepted. The store/CLI checking draft covers the Qt
string/list INI cycle, typed JSON schemas and transactional import/export. Opening an exported directory
with a live build of the original remains a required gate; a Morid-only round trip is not its substitute.
No trustworthy actual person-day measurement is available yet; the estimate below is unchanged.

**Deliverable:** a real Media Downloader data directory imports without loss, and the round trip works.

- `morid-store`: settings (the Qt dialect of D-016), presets, URL rules, per-engine defaults, subscriptions,
  archive handling (**never rewriting it**), history (tolerant read of the concatenated-JSON file).
- `morid import` / `morid export`, with a report.
- The round-trip test of [data-compat](data-compat.md) §6.

**Estimate:** 10–14 days. **Exit criteria:** F-68 done; unknown settings keys preserved; the original program can
read what Morid exports — checked by actually opening the exported directory with `legacy/`'s build, not by
inspecting the bytes.

**The lesson of Phase 0 applies here in full:** this phase is one file format after another, and each format has a
detail that is silent when wrong. Every rule must come from `legacy/` or from a fixture, and every rule must have a
test that fails when the rule is inverted.

---

### Phase 5 — Network, backend installation, updates

**Deliverable:** backends install and update themselves with integrity checking, from their own projects.

- `morid-net`: release metadata, download, size + SHA-256 verification, archive unpacking with path-traversal
  refusal.
- Version reading (`VersionArgument`, `VersionStringLine`/`Position`) — deferred from Phase 2, F-32.
- Extension installation/removal, supporting engines (ffmpeg, deno, quickjs-ng, python, bsdtar), F-35 and F-38.
- The endpoint split of [D-014](decisions.md) — backends from their own projects, the application's own update
  check pointing at Morid, and **no** extension download at all (D-010).

**Estimate:** 10–14 days. **Exit criteria:** F-32, F-34, F-35, F-38 done; a corrupted download leaves the old
binary intact; no request is made to a project that is not a backend's own release page.

---

### Phase 6 — Remaining backends

**Deliverable:** parity of the command line and the parsed events for every supported backend.

- gallery-dl, you-get, svtplay-dl, get-sauce, wget, aria2c, plus the named `BackendKind` variants they need.
- Every `BackendKind` variant added here is counted against the budget of seven
  ([architecture](architecture.md) §3.2); an eighth requires a decision record.

**Estimate:** 15–20 days. **Exit criteria:** F-36, F-37, F-39 done and the rest of F-30…F-39 closed; each backend
has fixtures; the budget is respected.

This is the point at which the console interface is feature-complete: `morid` alone is a usable replacement for a
user who lives in a terminal or a script.

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

## 5. Totals and what they mean

| Phase | Estimate | Cumulative | Status |
| --- | --- | --- | --- |
| 0 Reference behaviour | 5–8 | 5–8 | accepted |
| 1 Skeleton | 5–7 | 10–15 | accepted |
| 2 Domain + extensions | 10–14 | 20–29 | accepted |
| 3 One backend | 12–16 | 32–45 | next |
| 4 User data | 10–14 | 42–59 | — |
| 5 Network + installation | 10–14 | 52–73 | — |
| 6 Remaining backends | 15–20 | 67–93 | — |
| 7 Interface | 30–45 | 97–138 | — |
| 8 Parity + release | 15–20 | 112–158 | — |

**112–158 person-days is not a schedule.** It excludes review time, the owner's decisions, and the fact that the
estimates were made before a line of code existed. The number's usefulness is in its shape: the interface is
roughly a third, not the bulk, and the parts that make it trustworthy are the other two thirds.

The first usable console release (after Phase 6) is roughly two thirds of the total. That is the milestone worth
watching: at that point the risky part is done and the interface is "just" a lot of work.

**Where the estimate has already been wrong.** Phases 0–2 were estimated at 20–29 days and took about 12. That is
not evidence that the rest will run at half the estimate: those phases produced readers and scaffolding, while
Phases 3–6 produce behaviour that has to match a program nobody documented. The honest reading is that the
estimate for 0–2 was too high because it assumed a C++ reading effort that D-018 replaced with "ask the coder to
read it", and that Phases 3–7 have no equivalent saving available.

---

## 6. What went wrong, kept because it predicts the next phase

Four specification errors in Phase 2, three caught by the coder before a line of code was written. The pattern in
all of them: **a mechanism inferred from data and written down as fact.**

| Written | Reality | What it cost |
| --- | --- | --- |
| thin files inherit their family by `Name` | dispatch is by exact file name, `engines.cpp:244-258` | one exchange, no code |
| `\|\|` is not implemented by the original | `engines.cpp:1574` implements it and `gallery-dl.json` ships it | one exchange, no code |
| the playlist range is a validated grammar | free text split by `splitPreserveQuotes`, `playlistdownloader.cpp:1061` | one exchange, no code — but the requirement was **withdrawn**, not specified |
| `yt-dlp-aria2c` replaces the Generic command | `json.add` without `done()` publishes nothing; `Cmd` is unchanged | found at review; would have survived any "contains" test |

Two conclusions, both applied:

- **the contract belongs to the manager, the mechanism to the coder** ([D-018](decisions.md)); a task that
  paraphrases C++ moves errors one level down, where nobody checks them;
- **an invented requirement is worse than an invented mechanism.** A wrong mechanism costs a round. A wrong
  requirement — a validator the original does not have — costs the compatibility promise, and the test written from
  it would have passed.

Full record: [tasks/README.md](tasks/README.md).

---

## 7. Deliberately deferred

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
| Native ARM64 CI | GitHub could not acquire the runner on 10 Nov 2026 | revisit when the hosted pool has capacity; until then aarch64 is covered by resolver tests |

---

## 8. Open engineering choices

These are implementation choices, not product decisions; each is settled in the phase that needs it.

| # | Choice | Phase | Default |
| --- | --- | --- | --- |
| A1 | Threads + channels versus `tokio` | 3 | threads |
| A2 | `ureq` versus `reqwest` | 5 | `ureq` |
| A3 | Own INI parser versus a crate | 4 | own parser |
| A4 | gettext versus bundled catalogs | 7 | gettext |
| A5 | Job object strategy on Windows | 3 | `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` |
| A6 | How the CLI locates bundled `extensions/` outside a checkout | 3 | **decided:** beside the binary, then the per-user data directory; `CARGO_MANIFEST_DIR` in release builds is forbidden ([tasks/phase-3-one-backend.md](tasks/phase-3-one-backend.md) §3.7) |

A6 cannot be deferred past Phase 3 and is no longer an open choice: **Task 4 fixes the search order** and requires
a release binary to be demonstrated from outside a checkout. The reason it cannot slide: `morid engines` currently
finds its data through a compile-time path that exists only in a developer's checkout. That is acceptable for a
phase that only reports, and unacceptable for a binary a user installs.

---

## 9. How this document is kept honest

- A phase's status is written here only after its exit criteria are met, with the evidence (a CI run, a test
  name, a report).
- **A criterion that is not met is written down as not met**, next to the ones that were. Phase 2 shipped one and
  it is in §4 rather than in a footnote.
- Estimates are not retro-fitted. If a phase takes three times the estimate, the number stays and the variance is
  noted, because a roadmap that quietly matches reality teaches nobody anything.
- When a decision changes, the phase affected is named explicitly in [decisions](decisions.md), so this document
  can be updated in the same commit.
- Counts and statuses are checked against the files they describe before being written here. The previous version
  of [features](features.md) §8 said 44/22/14/4 for a list of 72 rows; that number had never been counted, and it
  was found while writing this document.
