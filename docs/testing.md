# Testing

Related: [features](features.md) · [migration](migration.md) · [architecture](architecture.md) ·
[extensions](extensions.md) · [cli](cli.md)

A reimplementation earns trust by being comparable to the thing it replaces. That is the whole test strategy in
one sentence, and it dictates the order of work: **the reference behaviour is captured before the Rust code is
written**, because otherwise "compatible" means "whatever we happened to build".

The original has no automated tests at all — no assertion in any of the 10 CI workflows, and the only
verification is a hidden CLI mode that spawns real backends against the network
(`legacy/src/engines/tests.cpp`). That mode is not carried over (F-41): a test that needs the internet is not a
test.

**What a green suite will mean:** the cases below pass. **What it will not mean:** that Morid behaves like the
original in general. Where a clause has no test, this document says so instead of implying coverage.

---

## 1. Invariants under test

| ID | Invariant | How it is tested |
| --- | --- | --- |
| I1 | For a given input, the argv passed to a backend equals the original's | golden argv fixtures captured from `legacy/`, per scenario ([migration](migration.md) §3) |
| I2 | Captured backend output produces the same parsed events | replay fixtures: raw bytes in, expected event sequence out |
| I3 | No data file the user had is lost or altered on import | import a real anonymised data directory; compare every key and entry |
| I4 | An invalid extension never runs a process | negative fixtures for each validation rule of [extensions](extensions.md) §8 |
| I5 | A cancelled job leaves no live descendant process | spawn a fake backend that starts a child; cancel; assert both are gone (Linux and Windows) |
| I6 | Nothing is written outside the download folder and the Morid data directory | test with a read-only sandbox and a destination symlink pointing outside; assert refusal |
| I7 | No credential reaches a log, history file or console stream | fixtures containing a cookie value and a proxy password; assert absence in every sink |
| I8 | Documented exit codes are produced | one test per row of [cli](cli.md) §3 |
| I9 | A hash or size mismatch on a backend download leaves the old binary intact | fixture HTTP responses with a corrupted body |
| I10 | The extension files shipped in `extensions/` all load | one test iterating the directory; failure names the file and the rule |

---

## 2. Traceability

Each acceptance criterion in [features](features.md) maps to a test. The ID is the `F-` or `C-` number plus a
suffix. A criterion with no test says so in the row; an untested row is a gap, not a plan.

| Feature | Test name | Covers |
| --- | --- | --- |
| F-01 | `formats_table_matches_golden_fixture` | I2 |
| F-02 | `option_parsing_matches_golden_argv` (table-driven: bare spec, `-f`, `default`, empty, quoted) | I1 |
| F-03 | `single_download_writes_expected_file` | I2 |
| F-04 | `progress_sequence_matches_golden` | I2 |
| F-05 | `cancel_kills_process_tree` (Linux, Windows) | I5 |
| F-06 | `bundled_presets_load_and_count` | I3 |
| F-10/F-11 | `list_roundtrip_through_saved_file` | I3 |
| F-13 | `scheduler_respects_concurrency_limit` | — |
| F-20/F-21 | `playlist_parse_and_range_arguments` | I1, I2 |
| F-23 | `archive_is_never_rewritten` | I3 |
| F-30…F-33 | `all_shipped_extensions_load`, `invalid_extension_never_spawns`, `min_version_2x_is_accepted`, `min_version_9x_is_refused` | I4, I10 |
| F-34 | `update_rejects_bad_hash_and_keeps_binary` | I9 |
| F-52 | `url_rule_precedence_matches_original` | I1 |
| F-54 | `proxy_modes_produce_expected_argv_or_env` | I1 |
| F-55 | `cookie_arguments_are_quoted_per_platform` | I1 |
| F-61 | `all_18_languages_load`, `missing_translation_falls_back_to_english` | — |
| F-62 | `second_instance_forwards_url_and_exits`, `stale_socket_does_not_block` | — |
| F-68 | `import_real_data_directory_loses_nothing` | I3 |
| C-01…C-06 | `cli_exit_codes`, `cli_json_stream_is_one_object_per_line`, `cli_dry_run_never_spawns` | I8 |

Rows that have **no** test and are known gaps, stated here on purpose:

- **RTL rendering.** Arabic text is not rendered correctly or incorrectly by any automated test; there is no
  headless rendering check. This is a manual procedure (§6).
- **Tray icon and desktop notifications.** Platform-specific and not exercised in CI.
- **Visual layout of the window.** No screenshot comparison. A UI that renders the right text in the wrong place
  will pass the suite.
- **Backend behaviour changes over time.** Fixtures capture a backend's output on a date; a backend that changes
  its wording breaks Morid without breaking a test, because the test still replays the old bytes. This is the
  central blind spot of the whole approach and is handled by §4.

---

## 3. Test layers and what belongs in each

| Layer | Runs | Belongs here |
| --- | --- | --- |
| Unit (`#[cfg(test)]` in each crate) | in-process, no I/O | option parsing, progress rule evaluation, version comparison, JSON validation, path rules |
| Fixture replay (`tests/` per crate) | in-process, files from `tests/fixtures/` | captured backend output → events; captured argv → comparison; extension loading |
| Integration (`tests/` in `morid`) | spawns a **fake backend** | queue, concurrency, cancellation, archive, data import |
| CLI (`tests/cli.rs`) | spawns the built `morid` binary | exit codes, JSON stream, `--dry-run`, `--quiet`, signal handling |
| Packaging (CI only) | on a clean runner | install, launch, uninstall, AppStream validation |
| Manual | human | rendering, RTL, tray, notifications |

**No test may touch the network.** The HTTP layer is tested against recorded responses through a trait, so
`morid-net` has tests without a socket. A test that fails on a plane is a test that will be deleted.

**The fake backend** is a small binary in the test tree that reads a scenario file and writes the captured bytes
to stdout with realistic timing, optionally spawning a child process and ignoring the first termination signal.
It is how I5 is tested at all.

---

## 4. Fixtures: capture, storage, provenance

**As built (5 November 2026):** the fixtures exist, in `fixtures/`, not in `tests/fixtures/`. The layout that was
specified in [tasks/phase-0-fixtures.md](tasks/phase-0-fixtures.md) and delivered differs slightly from the table
below, and the delivered one wins: `fixtures/scenarios/<case-id>/` (an `argv.json` plus the wrapper transcript it
was copied from), `fixtures/streams/<backend>/` (raw bytes with a SHA-256 per file in `meta.json`),
`fixtures/userdata/media-downloader/` (an anonymised data directory, reloaded by the original program),
`fixtures/manifest.json` (the index) and `fixtures/README.md` (provenance, re-capture, gaps). The capture helpers
are dev tooling in `tools/`, not a Cargo dependency.

Two properties of the delivered set matter for everything below. The argv goldens record **argument boundaries
as passed to `QProcess`**, not a shell string — the capture went through a wrapper backend that wrote
`sys.argv[1:]`, so quoting bugs cannot hide in the fixture. And the goldens are being made host-independent
(`<case-dir>`, `<node>`, `<local-url>` placeholders) so that a comparison on another machine is meaningful; until
that lands, a golden only matches on the capture host.

### 4.1 What is captured, and from where

| Fixture class | Source | Anonymised? |
| --- | --- | --- |
| Backend output (progress, list, errors, version) | real backend runs against a local deterministic HTTP server, merged stdout+stderr, unedited bytes | URL classes recorded instead of URLs; no cookies, no tokens |
| argv golden files | a wrapper backend recording `sys.argv[1:]`, `program`, `cwd` and proxy environment as JSONL; the golden is copied mechanically from the first record | capture-host paths and the Node path replaced by placeholders |
| Extension JSON | copies of `extensions/*.json` plus deliberate mutations | — |
| Data directory | a real Media Downloader data directory with paths and names replaced | yes, and reviewed before commit |
| HTTP responses | recorded GitHub API bodies | URLs kept; hashes are real values from a real release |

### 4.2 Storage rules

- fixtures live in `fixtures/`, one directory per class, with a `README.md` naming each backend version and the
  capture date; `fixtures/.gitattributes` disables EOL normalisation and whitespace checks for
  `fixtures/streams/**` because those bytes are evidence, not source;
- a fixture over 256 KiB is not committed; large captures are trimmed to the lines that matter, and the trimming
  script is committed instead;
- **no fixture may contain a credential, a session cookie, a private URL or personal data.** Review rejects it,
  and a test greps the fixture tree for the obvious shapes as a second line of defence.

### 4.3 The drift problem

A captured fixture documents what a backend did, not what it does. When yt-dlp changes its output format, Morid
breaks and the suite stays green. The mitigations, in order of importance:

1. a **nightly manual job** (documented, not a unit test) that runs the current backend against a fixed public
   URL and diffs the output against the newest fixture; a difference opens an issue;
2. every progress rule is written against the *structure* the fixture demonstrates, not against incidental
   wording, and the rule set is data ([extensions](extensions.md) §5.1), so a wording change is a JSON edit;
3. unknown output is never fatal: a line that matches no rule is passed through as `JobOutput` and shown, not
   dropped, so a broken parser degrades into noise rather than into silence.

---

## 5. What is explicitly not tested, and why

- **Fuzzing.** Not planned for Phase 1–6. The parser input is line-oriented text from a trusted local binary,
  not arbitrary bytes from the network. Revisit if Morid ever parses a file format.
- **Property-based testing.** No framework is introduced; targeted cases with named fixtures are preferred
  because a failure must be explainable to the owner in one sentence.
- **Performance and memory.** No benchmarks and no budget. The dominant cost is a child process doing network
  I/O; the GUI's own cost is not the risk this project is exposed to.
- **Mutation testing, coverage thresholds.** Coverage is reported, not gated. A percentage would reward testing
  getters.
- **Concurrency stress.** The scheduler is tested for the limit and for cancellation, not for "no race under
  load". A deliberate choice: races that only appear under load are better found by review of the state machine.

---

## 6. Manual procedures (kept as procedures, not wishes)

Recorded here so that "we tested it" has a meaning for things automation does not reach.

| Procedure | Steps | Pass condition |
| --- | --- | --- |
| M1 Arabic rendering | Set language to Arabic, open every tab | Text is right-to-left within each label, not clipped, not overlapping; layout stays LTR ([D-002](decisions.md)) |
| M2 Tray and notification | Enable tray, disable "quit on close", finish a download | Icon present, notification shown, window hides and restores |
| M3 Theme switch | Switch to the bundled dark theme and to a custom theme | Every widget readable; no white-on-white |
| M4 Player integration | Configure a player, double-click a finished row | File opens in the configured player |
| M5 Drag and drop / clipboard monitoring | Paste a URL with clipboard monitoring enabled | URL lands in the active tab |
| M6 Sleep and resume | Start a download, suspend the machine, resume | Job survives or fails with a comprehensible state; no zombie process |
| M7 Real world compatibility | 10 URLs covering the supported backends, run through Morid and the original | Same files, same names, same options |

---

## 7. CI

Target: **six** required checks on every pull request, all three platforms (`docs/platforms.md`). (This
paragraph said "four" while the table listed six; the table is the specification and the sentence was wrong.)

| Check | Command | Fails the build |
| --- | --- | --- |
| Format | `rustfmt --edition 2021 --check` over the tracked `.rs` files — **not** `cargo fmt --check`, see the note below | yes |
| Lint | `cargo clippy --all-targets --all-features -- -D warnings` | yes |
| Tests | `cargo test --all-features` | yes |
| Build | `cargo build --release` | yes |
| Size report | measure the release binaries, print and store the numbers | **no** ([D-008](decisions.md)) |
| Licence and advisories | `cargo deny check`, `cargo audit` | yes |

**Why not `cargo fmt --check`.** On the pinned toolchain (1.85.1) `cargo fmt` prints its own usage text and exits 1
for *both* documented spellings, `--all --check` and `-- --check`. That is a defect in the wrapper, not in the
formatting, and a CI step that cannot pass is worse than no step. Calling `rustfmt` directly over `git ls-files -z
'*.rs'` was the fix that produced a real diff and then a clean run. If a future toolchain repairs `cargo fmt`,
switching back is a one-line change — record it if you do.

**Why six checks and not "four".** Format, lint, test and build gate the pull request; the size report is printed
and stored but never fails it ([D-008](decisions.md)); licences and advisories (`cargo deny check`, `cargo audit`)
gate it. That is four gating checks plus a report, in six steps — the phrase "four required checks" counted only the
gating ones and confused everyone who read the table instead.

The size check is a report, not a gate: growth over 10 % from the previous measurement must be explained in the
pull request, and the history lives in [dependencies](dependencies.md). The gap this leaves — a program that
grows steadily because nothing stops it — is recorded as a risk in the changelog and is to be revisited at 1.0.

---

## 8. Definition of done for a feature

A row in [features](features.md) moves to `done` only when:

1. the acceptance criterion is a test that fails before the change and passes after;
2. that test runs in CI on all three platforms, or is listed in §6 as a manual procedure with the reason;
3. argv-compatibility is proven against a golden fixture where the feature touches a command line;
4. a refusal case exists for the feature's input domain;
5. documentation affected by the change is updated in the same pull request;
6. the binary size delta is recorded.
