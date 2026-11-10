# Task 4 for the coder — Phase 3: one backend, end to end

**Status:** ready to issue · **Issued:** 10 November 2026 · **Estimated:** 12–16 person-days
**Requires:** Tasks 1–3 accepted and merged (`main` at `f3d0dba` or later).
**Produces:** `morid list`, `morid download`, `morid download --dry-run` and `--json` working against a **real
yt-dlp**, with the argv byte-identical to the Phase 0 goldens — and one process tree that cancellation actually
kills.

---

## 1. Goal

Phase 2 proved that Morid can *say* what it would run. This phase proves that what it says is what the original
said, and then actually runs it.

At the end of this task:

- `morid-engine` spawns a backend, streams its output, splits it into lines the way the original does, classifies
  it into events, and cancels the whole process tree.
- `morid` owns a job: queue, concurrency limit, state machine, event stream. One implementation, used by the CLI
  now and by the window later.
- `morid list` and `morid download` exist, including `--dry-run` (prints argv, never spawns) and `--json`
  (one object per line, [cli](../cli.md) §5).
- **The argv for the yt-dlp scenarios in `fixtures/scenarios/` is identical to the golden**, element for element,
  in the same order.

This is the phase that answers the question the whole project rests on: *can the Rust version produce the same
command line as the C++ version?* If the answer is no, everything after this point changes.

It also closes **A6** (§3.7): after this phase the binary finds its extension files outside a developer's checkout,
or there is no installable product.

There is no window here and no settings file format. Resist both — §3.5 explains how settings are supplied and
why.

---

## 2. Context you need

1. [../architecture.md](../architecture.md) — §4 (a download end to end), §5 (concurrency and cancellation),
   §6 (the event model, which is a contract, not a suggestion).
2. [../cli.md](../cli.md) §1–§3 and §5 — flags, exit codes, the JSON stream. §2.3 already carries the
   Phase 2 reality; `list` and `download` are specified here and **not** yet implemented.
3. [../extensions.md](../extensions.md) §3–§5 — command selection, `SplitLinesBy`, `RemoveText`,
   `SkipLineWithText`, `ControlJsonStructure`.
4. [../migration.md](../migration.md) §7 — what "equivalent" means, clause by clause. Arguments identical, files
   identical, progress sequence identical, error *classification* identical and wording free.
5. [../testing.md](../testing.md) §1 (I1, I2, I4, I5), §3 (test layers), §4 (fixtures).
6. [../../fixtures/README.md](../../fixtures/README.md) — provenance, including §"Behaviour discovered, not
   specified", which lists five ways the original does **not** behave as one would guess.

---

## 3. What to produce

### 3.1 `morid-engine` — spawn, stream, classify, cancel

1. **Spawn.** `Command`, working directory, environment, stdin closed. No shell, ever: argv is a vector, and the
   operating system receives exactly it.
2. **Stream.** stdout and stderr are read **separately** but interleaved into one ordered stream per job, and the
   order must be *the order the bytes arrived*, not "all of stdout, then stderr". Whatever you choose, say what it
   is and prove it with a test that writes to both in a known order. The original merges them; §"equivalence" in
   [migration](../migration.md) §7 does not require identical interleaving, so **any documented, deterministic
   choice passes** — an undocumented, racy one fails.
3. **Split.** Line splitting follows the spec, not the platform: `SplitLinesBy` decides whether `\r` is a
   terminator. A trailing fragment with no terminator arrives as a final line when the process exits. Partial
   reads must never produce a truncated line — `fixtures/streams/yt-dlp/partial-line.bin` exists for this.
4. **Classify.** `RemoveText` substrings are stripped before matching; lines containing any `SkipLineWithText`
   entry are dropped; `ControlJsonStructure` decides whether a line is progress. Progress lines **replace** the
   current progress and are not also emitted as output; non-progress lines are output. This is
   [architecture](../architecture.md) §4 and it is the difference between a usable log window and a flood.
5. **Progress values.** The generic reader extracts percent, total, speed, ETA and file name from the line.
   For this phase, only the yt-dlp shape is required — the `--progress-template` string in the goldens is the
   contract, and the parser may be specific to it **if** a test says so and the generic path is where it lives.
6. **Cancel.** Process group on Unix (`setsid` + `killpg`), Job Object with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` on
   Windows. `libc` and `windows-sys` are already accepted dependencies for exactly this
   ([dependencies](../dependencies.md) §2.8, §2.9). **A cancelled job is not "the signal was sent"; it is "no
   descendant is alive"** — and the test proves it by checking the process table.
7. **Exit state.** `ExitState { code, signal, cancelled, duration }`, mapped to a typed `Error` variant. A
   backend that fails to start is `FailedToStart`, not `code = -1`.

### 3.2 `morid` — the job, the queue, the limit

1. **Job state machine.** `Queued → Running → Succeeded | Failed | Cancelled`, with no other transitions and no
   silent resets. Bad transitions are a bug, not an `unwrap`.
2. **Queue and concurrency.** `MaxConcurrentDownloads` bounds how many jobs run at once; the rest wait. The
   default is **4** ([features](../features.md) F-13), and what happens to `0`, to an empty value and to a value
   that is not a number is defined by the original, not by this paragraph —
   `legacy/src/settings.cpp` `maxConcurrentDownloads` and `legacy/src/configure.cpp` are the source, and the
   clamping (if any) goes in a test. Do not invent an upper bound. Order is submission order unless you choose
   otherwise, and then it is your call to document.
3. **Event stream.** `Event` as specified in [architecture](../architecture.md) §6. The service emits; nothing
   below it prints. `Notice` carries a key and arguments, never a formatted string.
4. **Cancellation is per job and per whole run.** Cancelling a queued job removes it; cancelling a running job
   kills its tree; `CancelAll` leaves the process table clean.
5. **The download folder is created before the process starts** and is the process's working directory — several
   backends write relative paths ([architecture](../architecture.md) §4). A folder that cannot be created is exit
   code `6`, not a panic.

### 3.3 argv construction — the part that has to be right

`morid download <url> --dry-run` prints the vector that would be passed. The vector is built from:

- the extension's `Cmd` for this platform and architecture (Phase 2 did this);
- the **default download arguments** for the request kind (`DefaultDownLoadCmdOptions` and what the engine adds);
- the user's options, tokenised by `morid-core` (`DownloadOptions`), placed according to `DownloadOptionsAsLast`;
- the flags that depend on settings: destination, archive file, cookies, proxy;
- the URL last.

**The order is the contract**, and it is not derivable from the extension JSON. The goldens are the specification.
Several of them show that it is not a fixed template with a variable tail; read these before writing the planner:

| Golden | What it proves |
| --- | --- |
| `s01-plain-yt-dlp` | the base vector, in order |
| `s03-user-format-22` | `-f 22` lands **before** `--no-js-runtimes`, not after it |
| `s04-conflicting-user-options` | two `-f`, then two playlist flags, both passed through in the order given |
| `s20-output-dir-space-unicode` | `-P <path>` lands directly after the `-o` group, before the runtime arguments, and the path is one argument containing a space and Cyrillic |
| `s09-download-archive` | a user archive replaces the internal `--download-archive`, and moves to where the user option belongs |
| `s11-cookies-browser`, `s12-cookies-file` | the cookie argument pair is **not** in the user-options slot: it comes after `--progress-template` and before `--download-archive` |
| `s17-subtitle-languages` | subtitle arguments land before the runtime arguments |
| `s12-cookies-file` | the cookie path contains a space and stays **one** argument |

Read all of these, and the rest of §5's in-scope set, before designing the insertion rules. A planner that appends
the user's options at the end passes none of these. A planner built from `ArgPolicy` and the extension data passes all
of them — which is what Phase 2's three warnings were for.

What I saw, offered as a summary and not as the specification: the `-o` group, then `--output-na-placeholder`, then
the engine's runtime arguments, then the user's options in their `ArgPolicy` positions, then `--progress-template`,
then the cookie pair, then `--download-archive`, then the URL. If this summary and a golden disagree, **the golden
wins and the summary gets reported as wrong.**

A conflict between two user options is **passed through**, not resolved: `s04-conflicting-user-options` carries
both `-f 22` and `-f 18` and the original sends both. Deduplicating or "fixing" that is a compatibility break.

### 3.4 `morid list` and `morid download`

Per [cli](../cli.md) §2.1 and §2.2, with the flags those sections list. In this phase:

- `list` runs the backend with the list arguments and parses the yt-dlp JSON line into a format table. The
  `--json` form emits a `metadata` event.
- `download` runs one URL, or `--batch <file>` runs several with the concurrency limit.
- `--dry-run` prints argv and exits `0` **without spawning**. A test asserts that no process was created — not
  that the code "looks like" it would not spawn.
- Exit codes come from `Error::exit_code()` and are the whole table of [cli](../cli.md) §3 that this phase can
  reach: `0`, `1`, `2`, `3`, `5`, `6`. `4` was Phase 2. `7`, `8`, `9` arrive later; the mapping already exists.

### 3.5 Settings — the seam, and why it is a seam

The goldens depend on settings: the default engine, the destination, the archive file, cookies, the proxy mode.
Phase 4 builds the INI reader. This phase must not wait for it, and must not grow a second settings format that
Phase 4 then has to undo.

**Do this:** define one `Settings` value in `morid` (plain data, documented defaults), used by everything that
plans a command. `morid-cli` builds it from, in order: built-in defaults → `--settings <file.json>` if given →
explicit flags (`--to`, `--engine`, `--archive`, `--no-archive`).

**The JSON override is a test and scripting surface, not a second product format.** It exists so the argv tests
can be fed the settings each capture used, and so `--dry-run` is usable from a script. It is documented as such in
[cli](../cli.md) and it is **not** the user's `settings.ini`.

**A warning that saves a wasted round.** The goldens were **not** captured with the product's default settings, and
the "expected" behaviour is not "defaults produce the golden". `tools/capture/capture.py` writes a `settings.ini`
per case: it turns off engine updates, forces `UseSystemEngine`, sets `MaxConcurrentDownloads` and, for the
yt-dlp download cases, installs a **fictional progress template** and `--no-js-runtimes` with
`--js-runtimes node:<node>`. Two consequences:

- `--progress-template` in the golden exists because the harness told the app to use that string. Do not implement
  a hard-coded template; implement the setting, and the golden becomes reproducible.
- `s26` and `s40` have no `--no-js-runtimes` at all, which is why their vectors are shorter. That is the same
  mechanism, not a special case.

Which settings each golden needs is not recorded in the golden files. The scenario's dict in `capture.py` is the
source. If a value cannot be recovered from that file and is needed for the argv, say so — that is a fixture gap,
and it gets recorded rather than guessed.

**Do not** implement the INI dialect here. D-016 governs it, Phase 4 owns it, and a mini-reader now is a thing to
delete later.

### 3.6 Tests

Named, and visible in CI output:

- `dry_run_argv_matches_golden_<scenario>` — table-driven over the scenarios in §5.
- `dry_run_never_spawns` — a counting double, or an empty `PATH`; the count is asserted to be zero.
- `list_parses_fixture_metadata` — replay of `fixtures/streams/yt-dlp/list.json`.
- `progress_lines_replace_and_do_not_append` — replay of `progress.txt`; the event sequence is asserted, including
  that progress lines are not duplicated into output.
- `partial_line_is_not_truncated` — replay of `partial-line.bin` fed in hostile chunk sizes, including a split
  inside a UTF-8 sequence.
- `carriage_return_splits_only_when_the_spec_says_so` — `SplitLinesBy` `["\n"]` versus `["\n","\r"]`.
- `remove_text_and_skip_line_apply_before_matching`.
- `queue_respects_concurrency_limit` — limit 2, five jobs, never three at once, all five finish.
- `cancel_kills_process_tree` (Linux, Windows) — the fake backend spawns a child and ignores SIGTERM; after cancel,
  **neither pid exists**.
- `unsafe_stays_in_the_two_platform_modules` — a script check, run in CI next to the Phase 0 checks: the only files
  containing `unsafe` are the two named modules and `morid-engine`'s crate root, and `#![forbid(unsafe_code)]` is
  still present in every other crate. It fails if a third module appears.
- `cancelled_job_keeps_files_unless_the_setting_says_otherwise`.
- `backend_failure_maps_to_exit_code_3`, `missing_destination_maps_to_exit_code_6`.
- `json_stream_is_one_object_per_line_and_versioned` — every line parses, every line has `type` and `v`, values
  match [cli](../cli.md) §5.
- `playlist_range_tokens_match_s07` — **carried in from Phase 2.** The range field produces the same tokens the
  golden does (`--playlist-items 2-3` in `s07`). The playlist *planner* is out of scope (§5); the tokenisation is
  not, because Phase 2 shipped it and nothing has yet checked it against a real capture.
- `url_round_trips_byte_for_byte_fixture_urls` — **carried in from Phase 2.** The existing test rounds-trips
  invented URLs; this one uses every URL that appears in `fixtures/scenarios/*/argv.json`, including
  `https://example.com/watch?a=1&b=two#frag%20x`. The old test stays; this one proves the shapes that matter.

**The fake backend.** A small binary in the test tree ([testing](../testing.md) §3) that reads a scenario file and
writes bytes with realistic chunking and timing, can spawn a child, and can ignore the first termination signal.
It is the only way I5 is testable, and it is part of this phase's deliverable, not a test helper you add later.

### 3.7 Where `extensions/` is found — A6, decided here

Today the CLI locates the bundled extension files through `CARGO_MANIFEST_DIR`, which exists only in a checkout.
That is the last thing in the binary that a user's installation cannot satisfy, and Phase 3 is where it stops.

**Order of search, and it is now fixed ([roadmap](../roadmap.md) §8, A6):**

1. beside the executable (the `extensions/` directory next to `morid`, or next to `morid.exe`);
2. then in the per-user data directory that [architecture](../architecture.md) §7 names — `<appdata>/morid`, next
   to the settings file and the `bin` directory the engines will live in;
3. **not** `CARGO_MANIFEST_DIR` in release builds. Keeping it as a debug-only fallback is acceptable **only** if a
   test proves a release build does not depend on it — and a real installation is demonstrated in §6.10.

A missing directory is not a crash: `morid engines` reports what it searched and exits `4`. An empty list of
searched paths is a bug, not a state.

### 3.8 What must not change

- `fixtures/` — read-only. CI runs the Phase 0 checks.
- `legacy/` — read-only.
- `docs/extensions.md`, `docs/cli.md` and the rest of the prose: if a document is wrong, report it. Phase 2's four
  correction rounds are the reason this line exists.
- The extension reader's public behaviour: 17 files, 17 resolutions, warnings unchanged.

---

## 4. Constraints

- **`unsafe` is confined to two modules in one crate** — [D-019](../decisions.md), decided on the coder's report
  that §3.1.6 could not be met under an unconditional `#![forbid(unsafe_code)]`. Cancelling a process *tree* needs
  `setsid`/`killpg` on Unix and Job Objects on Windows, and every crate that exposes them exposes them as `unsafe`
  or as a wrapper around `unsafe`; the ban was written before that was checked. The rule now:
  - `morid-engine` replaces `#![forbid(unsafe_code)]` with `#![deny(unsafe_code)]` and exactly **two** modules carry
    `#![allow(unsafe_code)]`: one for the Unix process group, one for the Windows Job Object. Every other crate
    keeps `forbid` unchanged.
  - Every `unsafe` block carries a `// SAFETY:` comment naming the invariant that makes it sound. An `unsafe` block
    outside those two modules fails review, not a test — the review greps for it.
  - The public API stays safe: no `unsafe` function, no raw handle, no `isize`/`*mut` in a signature across the
    crate boundary. A caller cannot reach the `unsafe`.
  - **Allowed mechanisms:** `libc` (`setsid`, `killpg`, `kill`) on Unix, `windows-sys`
    (`CreateJobObjectW`, `SetInformationJobObject`, `AssignProcessToJobObject`, `TerminateJobObject`) on Windows,
    and `std::os::unix::process::CommandExt::process_group` where it is enough. Anything else is a new dependency
    and needs asking first.
  - **Still forbidden:** `unsafe` in `morid-core`, `morid-ext`, `morid-store`, `morid-net`, `morid-cli`, `morid`,
    `morid-gui`; `unsafe` in `morid-engine` outside the two modules; any `unsafe` whose justification is a comment
    like "the handle is valid".
  - The report must say which two modules, how many `unsafe` blocks in each, and paste the grep that shows nothing
    else in the workspace.
- **No async runtime.** Threads and channels (A1 in [roadmap](../roadmap.md) §8; if you choose otherwise, that is
  a decision to bring to me with a reason, not a silent change).
- **No new dependency** beyond what [dependencies](../dependencies.md) accepts. `libc` and `windows-sys` are
  accepted (§2.8, §2.9). A third-party wrapper such as a safe Job Object crate is **not** accepted: it would add a
  supply-chain edge for the sake of hiding twenty lines we can see, and D-019 chose the visible version. Anything
  else: ask first.
- **No network in any test.** The fake backend and the fixtures are the only sources of bytes.
- **No spawning outside `morid-engine`.** `grep -rn "std::process" crates/` names one crate.
- **No panics on input or on process failure.** A backend that dies mid-line is a state, not an `unwrap`.
- **No `unwrap` in non-test code** without a justification in the PR.
- Errors stay typed and keep carrying what the user needs: which engine, which exit code, which file.

---

## 5. Scenarios this phase must reproduce exactly

**In scope — argv identical, element for element.** These are the yt-dlp download cases this phase's CLI can
express:

| Scenario | What it adds |
| --- | --- |
| `s01-plain-yt-dlp` | the base vector: defaults, engine runtime args, archive, URL |
| `s02-bundled-preset` | user options from a preset position |
| `s03-user-format-22` | `-f 22` in the user-options slot |
| `s04-conflicting-user-options` | two `-f` and two playlist flags, both passed through |
| `s05-format-list` | the list command's vector (also covers `morid list`) |
| `s09-download-archive` | an explicit archive path instead of the internal one |
| `s11-cookies-browser` | cookie argument from a browser |
| `s12-cookies-file` | cookie argument from a file, path with a space |
| `s17-subtitle-languages` | subtitle arguments |
| `s19-split-chapters` | chapter arguments |
| `s20-output-dir-space-unicode` | `-P` with a space and Cyrillic in the path |
| `s21-user-output-template` | a second `-o`, both present |
| `s22-url-metacharacters` | URL with `&` and `%20`, passed as one argument |
| `s37-audio-extraction` | `-x --audio-format mp3`, order preserved |
| `s10-batch-two-urls` | one process **per URL**, two entries in `log.txt`, and the golden's `argv.json` holds only the first — the two differ in the last element and nothing else |

**Out of scope this phase, with the reason stated:**

| Scenario | Why not now |
| --- | --- |
| `s06`, `s08` (playlists) | need the playlist planner, F-20/F-22; Phase 6 |
| `s07` (item range) | the argv needs the playlist planner, but its `--playlist-items` tokens **are** checked — Phase 2's range tokenisation is verified against this golden (§3.6) |
| `s13`–`s15` (wget, proxy) | proxy argument/environment construction is Phase 6 |
| `s16`, `s18` (subtitles list, comments) | metadata paths, not the download planner |
| `s23` (URL manager) | `morid-store`, Phase 4 |
| `s24`–`s27`, `s40` (custom extensions, packaging) | extension features pinned by the Phase 0 harness, not by the CLI |
| `s29`–`s34` (other backends) | Phase 6 |
| `s38` (command after download) | needs the settings the store owns and a shell, Phase 4 |
| `s39` (non-UTF-8 encoding) | P1, F-56; the plumbing is here but the golden needs the encoding setting |

If one of these turns out to be trivially in reach, take it and say so. If one of the in-scope rows turns out to
need something from another phase, **that is the more important finding** — report it before bending the scope.

---

## 6. Acceptance criteria

1. `cargo build --release` and `cargo test` green on Linux, macOS and Windows in CI, links attached; fmt and
   clippy `-D warnings` clean.
2. `--dry-run` argv equals the golden for **every** in-scope scenario of §5, on the platform the golden was
   captured on, and the test names appear in the output.
3. `s04` proves pass-through: both `-f 22` and `-f 18` are present, in the original's order.
4. `dry_run_never_spawns` proves zero process creation.
5. `cancel_kills_process_tree` passes on Linux **and** Windows in CI, and asserts on the process table, not on the
   return value of `kill`.
6. The `--json` stream is one object per line, versioned, and matches [cli](../cli.md) §5 for the events this
   phase emits. Paste the first and last line of a real run.
7. `grep -rn "std::process" crates/` names `morid-engine` only, and `grep -rn "unsafe" crates/` names only the two
   modules of §4 plus the crate attribute in `morid-engine/src/lib.rs`.
8. Phase 0 checks still pass in the same job (`Ran 30 tests`, `VERIFY_OK`).
9. `url_round_trips_byte_for_byte_fixture_urls` exists and uses the fixture URLs.
10. A real run and a real installation are demonstrated: paste `morid download` against the deterministic HTTP server
    the capture harness uses, with progress events and the resulting file name — and run the release binary from a
    directory that is not a checkout, finding `extensions/` per §3.7. This phase is "end to end": an argv test alone
    does not show that a download happened, and a working checkout does not show that a user can install it.
11. Every new `Error` path has a test that maps it to a documented exit code.
12. No network in any test; `cargo test` passes with networking unavailable.
13. A6 is closed: the search order of §3.7 is implemented, `morid engines` reports the paths it searched, and the
    release binary is demonstrated from outside the checkout.

---

## 7. Testing you must report

Run these and paste the output, not a summary:

- `cargo test` with test names, per platform.
- The `--dry-run` output for all in-scope scenarios, next to the golden.
- One real download: the command, the event stream, the file that appeared.
- The `--json` first and last lines and `wc -l`.
- The two `grep` checks with their exit codes (`std::process`, and `CARGO_MANIFEST_DIR` in release paths).
- The cancel test output on Linux and Windows, including the process-table assertion.
- Anything from §5 that surprised you.
- The release binary's `morid engines` output from a directory that is not a checkout, with the searched paths.

Report in this order: (1) files added or changed; (2) what is implemented and what is deliberately not; (3) the
outputs above; (4) known problems, including anything you had to guess; (5) every place where this task or a
document contradicts what the code actually does — with the evidence.

**If a claim in this task is wrong, say so and do not implement it that way.** Phase 2 established that this is
the cheapest thing that can happen to a task; four such reports there cost nothing and prevented a phase of wrong
work.

---

## 8. What the review will try to break

Named here so it is not a surprise, and so a report that says "the criteria pass" is not the end of the round:

- A golden made to pass by **special-casing a scenario id**, or by a lookup table from scenario to argv. The
  review runs a dry-run for an in-scope scenario whose user options were **not** in the golden (a third `-f`, a
  `--to` with a space) and checks the placement is still right.
- `s04` passing because the planner drops the second `-f`. The review asserts **both** are present, by index.
- Cancellation passing because the fake backend exits on SIGTERM by itself. The review swaps in a fake that ignores
  it and checks the process table again.
- The `--json` stream passing because it is a single object at the end. The review pipes a real run through
  `jq -c .` and counts lines against progress events.
- Line splitting passing on a nice file. The review replays `partial-line.bin` split at every byte, and
  `non-utf8.bin` for the error path.
- `--dry-run` "not spawning" asserted by a flag. The review runs it with a `PATH` that contains only a script that
  touches a file, and checks the file does not exist.
- A6 "closed" with `CARGO_MANIFEST_DIR` still in the release path. The review greps for it in the built binary's
  source path selection and runs from `/tmp`.

Each of these is a criterion, not an opinion; failing one is a failed round, and it is cheaper to fail it before
the hand-back than after.

---

## 9. Documents that change as a result

Updated **by you**, in the same pull request, and only these:

- `docs/roadmap.md` Phase 3 — status paragraph and the estimate corrected against the actual.
- `docs/dependencies.md` §6 — one new row in the size table: date, commit, both binaries, and the CI run.
  Do not rewrite the existing rows.
- `docs/cli.md` — **only** if the implemented flags or the JSON fields differ from §2.1, §2.2, §5; say what
  changed and why, in the PR, not silently.

Updated by the manager after review: `docs/features.md` statuses (F-01…F-05, F-13, F-21, F-23, F-50, C-01, C-02,
C-05, C-06), `docs/testing.md` §2, and anything else the review finds wrong.

---

## 10. What comes next, so you can design accordingly

Phase 4 replaces the settings seam of §3.5 with the INI reader of D-016 and adds the files: presets, URL rules,
subscriptions, archive, history, import. Two consequences for today's design:

- **`Settings` must be a value, not a global.** Phase 4 will construct it from a file; if the service reads a
  global at any point, the import path cannot be tested.
- **The archive file is passed as a path now and owned by the store later.** Do not write to it in this phase
  beyond what a backend does; the store owns the format.
