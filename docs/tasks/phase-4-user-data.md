# Task 5 for the coder — Phase 4: user data, import and export

**Status:** ready to issue · **Issued:** 4 November 2026 · **Estimated:** 10–14 person-days
**Requires:** Tasks 1–4 accepted and merged (`main` at `f16034b` or later).
**Produces:** `morid-store` reading and writing every file a Media Downloader user already has, and
`morid import` / `morid export` moving that data in and out **without loss**.

---

## 1. Goal

Phase 3 proved a backend can be driven with the original's command line. This phase proves the second half of
D-003: **a user can move here without losing anything, and can go back.**

At the end of this task:

- `morid-store` reads `settings.ini` and writes it back in the Qt dialect of D-016 — values *and* keys — and owns
  the six JSON data files plus the archive files it must never touch.
- `morid import <dir>` copies an existing Media Downloader data directory into a Morid data directory, reports
  every imported, skipped and unknown item, and exits `0`, `7` or `8` accordingly.
- `morid export <dir>` produces a directory the **original program can be pointed at** and read.
- `morid download` and `morid list` take their settings from the real data directory instead of the JSON seam,
  and the `--settings` flag keeps working exactly as it does today.

The acceptance surface is a round trip, not a diff of bytes: original data → import → export → **the original
program opens it and its settings and lists are intact**.

There is no window in this phase, no network, and no backend installation. Resist all three.

---

## 2. Context you need

1. [../data-compat.md](../data-compat.md) — **the specification for this task**, in this order: §1 paths, §2 the
   data files and their real field names plus the three rules that are not visible in the bytes, §3.0 and §3.0a
   the two Qt escapes, §3.1 the 94 keys, §4 the import procedure, §5 the questionable formats, §6 export.
2. [../architecture.md](../architecture.md) §1 — `morid-store` has no processes and no network. §7 — where the
   data directory lives and what mirrors what.
3. [../cli.md](../cli.md) §2.4 (`import` / `export`), §3 (exit codes `0`, `1`, `7`, `8`, `9`), §2.2 (`--config`,
   `--settings`).
4. [../decisions.md](../decisions.md) — D-003 (compatibility is the promise), D-016 (the INI dialect, and why
   compatibility beats tidiness), D-013 (self-update settings are **not** imported).
5. [../../fixtures/README.md](../../fixtures/README.md) and the captured directory itself:
   `fixtures/userdata/media-downloader/` — 94 settings keys, 15 presets, 1 URL rule, 2 engine-default rows,
   1 subscription, 2 saved list entries, 20 history objects, 1 archive line, 2 themes, and `meta.json` recording
   that the original reloaded all of it.
6. `tools/capture/test_qt_ini_roundtrip.py` — **the corpus** every rule must reproduce. The specification is
   executable, but it is split across two modules and this task previously named the wrong one:
   **decoders** in `tools/capture/qt_ini.py` (`qt_ini_unescape`, `qt_ini_unescape_key`, `qt_ini_lines`,
   `read_settings_ini`) and **encoders** in `tools/capture/canonicalise.py` (`escape_qt_ini_value` line 181,
   `escape_qt_ini_key` line 233). Port from the code; do not re-derive from the prose.

---

## 3. What to produce

### 3.1 The two escapes, as two functions and nothing else

`morid-store` gets a value escaper/unescaper and a key escaper/unescaper, with the rules of
[data-compat](../data-compat.md) §3.0 and §3.0a. They may share a UTF-16 code-unit splitter and **nothing else** —
a single "smart" function is how a `%U` sequence ends up inside a path.

Your source of truth is that Python, not the prose: where the two disagree, the Python is what was proved against
Qt's own reader and writer.

**A value is not always a string, and this is the trap in this section.** `qt_ini_unescape` returns `str | list[str]`:
a comma outside quotes starts a `QStringList`. The captured fixture has exactly one such key —
`BasicDownloaderOptionsHistory-yt-dlp`, whose raw value is 21 comma-separated entries — and `escape_qt_ini_value`
**raises** when handed the list, because encoding a list is not its job. Writing the items back requires joining
them, one escaped item per element, with `", "`.

The whole-file byte comparison is the test that catches this, and it was verified before this task was issued:
re-encoding every line of `fixtures/userdata/media-downloader/settings/settings.ini` through
`qt_ini_unescape` → (`", "`-join for a list) → `escape_qt_ini_value` / `escape_qt_ini_key` produces 3 351 bytes
**identical** to the fixture. Treating that one value as a single string cannot produce the same bytes, and a
writer that quotes it instead — which is what `escape_qt_ini_value` does with any value containing a comma —
changes what the original reads: 21 history entries become one 400-character option string, with no error on
either side. `test_qstringlist_splitting_is_not_emitted` in the corpus states this on purpose.

So: the store has to carry "this value is a list" through a read and a write. Choose the representation you think
is right and **say which you chose and why**; what is not allowed is flattening it to a string.

(The fixture is pure ASCII, so the byte comparison is unambiguous. Read it as bytes, not as UTF-8 — `qt_ini_lines`
decodes Latin-1 because that is what Qt 5 does, and a value that is not ASCII is a finding to report, §3.0.)

Tests, named:

- `ini_value_round_trips_qt_corpus` — every case of `check_all()` in the Python suite, in Rust, including the
  digit defence (`…\x4385` is a different string), the quoting rule (`;`, `,`, `=`, edge spaces) and the
  control-character escapes.
- `ini_key_round_trips_qt_corpus` — `%XX` as a **Latin-1 code point**, `%UXXXX` uppercase only, a non-BMP
  character as **two** escapes (`%UD83D%UDE00`), and a malformed escape **refused** (documented divergence, §3.0a).
- `ini_list_value_round_trips_as_a_list` — `a,b` reads back as two items, not one; re-encoding those items with
  `", "` gives `a, b`; and the 21-entry fixture key survives with 21 entries.
- `settings_ini_from_fixture_decodes_without_loss` — read `fixtures/userdata/media-downloader/settings/settings.ini`,
  get **94 keys**, and re-encode to bytes that equal the input byte for byte. If the fixture cannot be reproduced
  exactly, that is a finding about the fixture or about your writer — report it, do not normalise it away.

### 3.2 Settings as a value, not a global

`Settings` already exists as a plain value in `crates/morid/src/planner.rs` and Phase 3 builds it from defaults →
`--settings` → flags. Keep that shape; add a constructor that reads a data directory.

- Unknown keys are preserved verbatim under an `[imported]`-prefixed section, never used, never deleted, and
  **reported** on import.
- Values that are valid UTF-8 but not ASCII (what a Qt 6 build writes) and values that are neither are **reported
  as findings**, never silently decoded as Latin-1 (§3.0).
- Keys `WindowsUpdateChannel`, `CheckForUpdates`, `ShowVersionInfoAndAutoDownloadUpdates` are **not imported**
  (D-013) and the user is told they were dropped.
- Out-of-range values (`MaxConcurrentDownloads` outside 1–64) are clamped **with a report**.

Tests: `unknown_settings_keys_are_preserved_and_reported`, `ck_reports_a_utf8_value_instead_of_decoding_it`,
`update_settings_are_dropped_with_a_notice`, `concurrency_out_of_range_is_clamped_and_reported`.

### 3.3 The data files

One reader and one writer per file, each with its real field names from [data-compat](../data-compat.md) §2 — the
fixtures are the authority, and the headers are not. The rules a reader written from the bytes alone gets wrong:

- **Presets:** 15 entries, shipped in their post-rewrite form. Reproduce the constructor's two replacements from
  `legacy/src/configure.cpp` (`presetOptions::presetOptions`, the `data.replace` calls) when generating the
  bundled file; when importing, copy what is on disk.
  Test: `bundled_presets_match_the_captured_file` — the generated default file equals
  `fixtures/userdata/media-downloader/data/presetOptions.json` entry for entry, in order. (This is the check that
  makes "15, not 17" true rather than quoted.)
- **URL rules:** `*x` ends-with, `x*` starts-with, `*x*` contains, bare contains; **first** matching rule in file
  order wins. Test: `url_manager_takes_the_first_match` with a table covering all four operators and two rules
  that both match, asserting the earlier one wins.
- **Per-engine defaults:** the first row for that engine whose `default` is `"yes"`.
  Test: `engine_defaults_take_the_first_enabled_row` — the fixture has two `yt-dlp` rows with opposite values.
- **The archive file:** written only by the backend, passed through, **never parsed for logic beyond F-24's two
  flags and never rewritten**. Test: `archive_is_never_rewritten` — import, export and a full download cycle leave
  a byte-identical copy; assert on bytes, not on "unchanged".
- **History:** a stream of concatenated objects, read tolerantly; an unparsable fragment is kept in the report and
  the rest survives. Test: `history_reads_20_objects_from_the_fixture` and
  `history_keeps_the_rest_when_one_fragment_is_corrupt` (inject a break at a byte offset, assert 19 or 20 and the
  report).
- **`autoSavedList.json`:** the key set varies per entry; read `url` always and the rest when present; a running
  entry is restored as *not started* (§5.2).
- **Themes:** `themes/*.json` are colour maps with `rgba` arrays, `GlobalColor` and `Qt::GlobalColor` indirections.
  This phase **stores and round-trips** them; resolving a palette is Phase 7. Test:
  `themes_round_trip_byte_for_byte`.

### 3.4 `morid import` / `morid export`

Follow [data-compat](../data-compat.md) §4 exactly, and note what each rule is for:

1. **Verify** the source looks like a Media Downloader directory (`settings/settings.ini` or the JSON files).
   Otherwise exit **7**.
2. **Copy, never move.** The source is opened read-only and left byte-identical. Test:
   `import_leaves_the_source_byte_identical` — hash every file before and after.
3. **Write into `<appdata>/morid/`** with the same names and structures.
4. **Report** imported / skipped / unknown keys / warnings — to stdout, and as JSON with `--report <file>`.
5. **Never overwrite** an existing Morid file without `--force`; with `--force`, back up first.
6. **Idempotent**: a second run produces the same result and duplicates nothing. Test: `import_is_idempotent` —
   run twice, compare the tree.
7. **Non-destructive on failure**: a copy that fails halfway removes the half-written Morid files and keeps the
   previous state.
8. **Partial is not success**: anything not imported → exit **8**, and the report says what.

`export` writes a directory the original can open (§6). It is a **copy, not a conversion**: structure and field
names are the original's, and nothing Morid invented appears in it.

### 3.5 Wire the CLI to the real directory

`morid download`, `morid list` and `morid engines` resolve their data directory in this order: `--config <dir>` →
`MediaDownloaderAppDataPath` → the platform default ([data-compat](../data-compat.md) §1). Settings come from
`settings.ini` there. The `--settings <file.json>` flag keeps working as an override on top, because scripts use it
and the Phase 3 goldens are run with it — **do not remove it, and do not make it a second format** (it is already
documented as not being one).

Test: `download_reads_the_fixture_settings_ini` — with `--config fixtures/userdata/media-downloader`, `--dry-run`
picks up the fixture's engine, archive and encoding values rather than the built-in defaults.

### 3.6 Exit codes and error paths

`7` (source not recognised) and `8` (partial) are new in this phase and both must be produced by a test; `9`
(internal) stays a bug path. `C-05` closes when every documented code has one.

---

## 4. Constraints

- No `unsafe`, no `build.rs`, no async runtime, no new dependency without asking. In particular: **no general INI
  crate** — D-016 says why, and [dependencies](../dependencies.md) §2 carries the rejection.
- `morid-store` gets no process handling and no network; if something needs either, it does not belong there.
- `legacy/` is read-only.
- Every rule you implement comes from `legacy/`, from a fixture, or from the Python port — and the test fails when
  the rule is inverted. A rule you inferred but cannot point at is reported, not implemented.
- **No new fixture-capture round in this phase.** If the captured directory cannot answer a question, say which
  question and stop on that point; that is a Phase 0 follow-up, not something to invent.

---

## 5. Acceptance criteria

1. `cargo build --release`, `cargo test`, fmt and clippy `-D warnings` green on Linux, macOS and Windows in CI.
2. The four escape tests and the 94-key round trip pass, and the re-encoded fixture is **byte-identical** (3 351
   bytes, one of them a 21-item list) — seen in the output, not asserted in prose.
3. `bundled_presets_match_the_captured_file` passes: 15 entries, post-rewrite, in order.
4. `url_manager_takes_the_first_match` and `engine_defaults_take_the_first_enabled_row` pass, and each fails if the
   direction is inverted (show that: flip it locally, paste the failure, flip it back).
5. `archive_is_never_rewritten` asserts on bytes across import, export and a download cycle.
6. `import_leaves_the_source_byte_identical` and `import_is_idempotent` pass; a second import changes no file.
7. A real round trip is demonstrated, not described: original data → `morid import` → `morid export` → **the
   original program opens the exported directory and finds its settings and lists**. Paste the original's output
   (or its own log) and the exported file list.
8. Exit codes `7` and `8` are each produced by a test; `docs/cli.md` §3 matches what the binary does.
9. `download_reads_the_fixture_settings_ini` passes, and the Phase 3 goldens still pass unchanged.
10. `grep -rn "unsafe" crates/morid-store` is empty, and the crate-boundary check in CI still passes.
11. No test needs the network; `cargo test` passes with networking unavailable.
12. The size row for [dependencies](../dependencies.md) §6 is added with the CI run, not guessed.

---

## 6. Testing you must report

Paste output, not summaries: `cargo test` with test names per platform; the byte comparison of the re-encoded
`settings.ini`; the presets comparison; the two inverted-rule failures and the resulting passes; the round trip
with the original's own output; a sample of the import report (human and `--report` JSON); the `--dry-run` that
proves `--config` reaches the planner; and anything in §2 that surprised you.

Order: (1) files added or changed; (2) what is implemented and what is deliberately not; (3) the outputs above;
(4) known problems, including anything guessed; (5) every place where this task or a document contradicts what the
code does, with the evidence.

**If a claim in this task is wrong, say so and do not implement it that way.** Two such reports in Phase 2 and one
in Phase 3 cost nothing; the same claim implemented costs a phase.

---

## 7. What the review will try to break

- A round trip that "works" because both sides are Morid. The review exports and then **opens it with `legacy/`'s
  build**, and reads the settings back with Qt's reader.
- The escapes passing on ASCII only. The review feeds Cyrillic, emoji (non-BMP), a value ending in a hex digit, a
  value containing `;` and `=`, a key with Cyrillic, a `%u` lowercase escape (which must **not** be accepted), and
  a malformed `%zz`.
- The list value flattened. The review writes a settings file with an options-history key, reads it with Qt's own
  reader, and counts the items: 21 expected, one long string is a fail.
- An import that silently drops unknown keys. The review plants one key that appears in no document and checks it
  survives and is named in the report.
- An import that is idempotent because it does nothing on the second run: the review imports into a directory that
  already has a *different* value for one key and checks the report says so instead of quietly keeping either.
- `--force` overwriting without a backup: the review looks for the backup file.
- The archive check passing because the archive was never exercised: the review runs a download cycle with
  `--download-archive` and compares bytes.
- A history reader that loses everything on one bad fragment: the review corrupts the middle object.

Each is a criterion, not an opinion.

---

## 8. Documents that change as a result

Updated **by you**, in the same pull request, and only these:

- `docs/roadmap.md` Phase 4 — status paragraph and the estimate corrected against the actual.
- `docs/dependencies.md` §6 — one new row in the size table. Do not rewrite existing rows.
- `docs/cli.md` — only if the implemented flags or exit codes differ from §2.4 and §3; say what changed and why.

Updated by the manager after review: `docs/features.md` statuses (F-06, F-07, F-16, F-23, F-24, F-25, F-50, F-51,
F-52, F-57, F-64, F-68, C-04, C-05), `docs/data-compat.md` where the review finds it wrong, and
`docs/testing.md` §2.

---

## 9. What comes next, so you can design accordingly

- **Phase 5** adds `morid-net` and engine installation: a real download with SHA-256 verification, and version
  reading. `morid-store` owns *where* the `bin` directory is; `morid-net` owns *what* goes into it. Do not let the
  store grow a downloader.
- **Phase 7** draws the Configure screen: presets, URL manager, per-engine defaults, proxy, import. Every one of
  those screens must call what you build here, so a value the store cannot represent becomes a screen that cannot
  be built. If a field has no home in this design, say so now.
