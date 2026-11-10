# Task 3 for the coder — Phase 2: domain, extension schema, and command resolution

**Status:** ready to issue · **Issued:** 8 November 2026 · **Estimated:** 10–14 person-days
**Requires:** Task 1 (`phase-0-fixtures.md`) and Task 2 (`phase-1-skeleton.md`) accepted and merged.
**Produces:** all 17 bundled extensions load, validate and resolve to a command — and nothing is ever spawned.

---

## 1. Goal

Make the two crates that carry knowledge — `morid-core` and `morid-ext` — real, and prove them against the 17
extension files this repository ships plus the negative cases that must be refused.

At the end of this task:

- `morid-core` knows what a URL is, what a format spec is, how option tokens are spelled, how a playlist range is
  written, and how versions compare. It has no I/O, no processes, no network, no time, no randomness.
- `morid-ext` reads an extension file, validates it in the documented order, resolves the command for the current
  platform and architecture, evaluates `ControlJsonStructure`, and reports what it decided.
- `morid-cli` gains one command — `morid engines` — that prints the resolution result for every bundled file. It is
  the observable end of the phase and the thing a reviewer runs.
- **No test in this phase spawns a process.** That is invariant I4 and the phase exists to establish it before
  anything can start a downloader.

There is no downloader here and no window. Resist both.

---

## 2. Context you need

1. [../extensions.md](../extensions.md) — **the specification for this task.** §1 schema versioning and thin files,
   §2 identity keys, §3 command selection and placeholders, §4 arguments and `ExtraArguments*` (§4.1), §4.2
   credentials, §5 output parsing keys, §8 validation order. Every key the shipped files use is documented there; the
   files themselves are the source of truth and there are **40** keys, not the "34" older text says.
2. [../architecture.md](../architecture.md) §1 — the crate map and the *must not contain* column. `morid-core` has no
   I/O. `morid-ext` has no networking and no spawning. The compiler will not enforce this; you must.
3. [../roadmap.md](../roadmap.md) — Phase 2 exit criteria.
4. [../testing.md](../testing.md) §1 — invariants I1, I3, I4, I10. §2 — the test names this phase owes:
   `all_shipped_extensions_load`, `invalid_extension_never_spawns`, `min_version_2x_is_accepted`,
   `min_version_9x_is_refused`.
5. [../decisions.md](../decisions.md) — D-004 (soft version check, `2.x` treated as satisfied), D-005 (no Windows 7
   architecture keys), D-011 (no profiles).
6. [../extensions.md](../extensions.md) §3 — `win7*` keys are **not** used on supported Windows. A file that only
   defines them is reported unsupported, not silently downgraded to x86.
7. [../fixtures/README.md](../../fixtures/README.md) — the fixtures from Phase 0. They are argv goldens for later
   phases; this phase does not consume them, but do not break the Phase 0 checks that CI now runs.

---

## 3. What to produce

### 3.1 `morid-core` — types and rules, no I/O

1. **`Url`** — a parsed URL that keeps the original string for output. It must round-trip byte for byte: the argv
   fixtures contain URLs with `&`, `#`, percent-escapes and non-ASCII, and a URL that comes back normalised breaks
   every later golden. Validation is "does this look like a URL a backend could accept", not RFC 3986 policing.
2. **`Format`** — the user's format specification as the original models it: a bare token (`mp4`, `best`), an explicit
   `-f`-style spec, an empty value meaning "engine default", and a quoted value. Parsing produces the token list that
   later becomes argv; it does not decide what an engine does with it.
3. **`DownloadOptions`** — the user's option string. Parsing is **platform-independent**: quote handling for the
   platform's own shell grammar belongs to the caller (§3.2), and the fixtures are the specification of what the
   result must be.
4. **`PlaylistRange`** — `1-3`, `1,3,5`, `1-3,7`, open-ended forms. Refuse what the original refuses, accept what it
   accepts; when in doubt, the fixture argv from Phase 0 is the answer.
5. **`EngineId`** — the `Name` of an extension, plus the archive-file naming the original derives from it
   (`archiveFile-<Name>.txt`). This is a data contract: renaming an engine is a migration.
6. **Version comparison** — dotted numeric, tolerant of the suffixes the shipped files use (`2.0.0`, `2.x`, `1.0`,
   absent). D-004: the `2.x` family is treated as satisfied.
7. **`Error`** — typed variants, not strings. Every variant that a user can trigger maps to one exit code of
   [../cli.md](../cli.md) §3; the mapping lives here, in one function, not in `morid-cli`.

### 3.2 `morid-ext` — reading, validating, resolving

1. **The reader.** `serde` for the known keys plus `serde_json::Value` kept alongside, so unknown keys are
   **preserved and reported as a warning** and never fatal ([extensions](../extensions.md) §8 rule 7). A struct that
   silently drops them fails the task.
2. **Thin files — by exact file name, not by inheritance.** Three shipped files carry only `Name` and `Version`
   (`yt-dlp-nightly`, `yt-dlp-ffmpeg`, `yt-dlp-aria2c`). The original dispatches on the file name
   (`legacy/src/engines.cpp:244-258`), loads `yt-dlp.json` as the base, and applies one hard-coded transformation per
   file (`legacy/src/engines/yt-dlp.cpp`): nightly replaces the whole `Cmd` and the update URL; ffmpeg and aria2c
   append `--downloader <x>`; aria2c also replaces the Generic command with `stdbuf -o L yt-dlp`.
   [../extensions.md](../extensions.md) §1 states the exact difference per file. A generic "resolve the family by
   `Name`" mechanism is **wrong** and was the first version of this task; it would produce working-looking commands
   with the wrong arguments. Implement the three transformations explicitly, keyed by file name, and make each one a
   test with its expected argv-visible effect.
3. **Validation, in the documented order**, with a message naming which rule failed and the file:
   1 valid JSON · 2 `Name` · 3 `Version` ≤ supported · 4 `RequiredMinimumVersionOfMediaDownloader` (D-004) ·
   5 a command resolves for this platform/arch, after thin-file resolution · 6 executable present, else "needs
   installation" and **kept in the list** · 7 unknown keys → warning · 8 `ExtraArguments*` → warning.
   Rules 1–5 keep the file out; rule 6 does not. That distinction is the whole point of the rule set.
4. **Command selection.** platform block (`Windows`/`MacOS`) before `Generic`; architecture key within it; `win7*`
   keys never used (D-005). Placeholders `${default}`, `${BackendPath}`, `${CommandName}` resolved as documented.
   Name resolution order: absolute path → application bin directory → `PATH`.
5. **`ControlJsonStructure`.** An expression over `startsWith`, `contains`, `endsWith` and a `Connector` (`&&`, `||`),
   evaluated against an output line. It is data, not code: no parsing of arbitrary expressions beyond the documented
   shape. A malformed structure is a validation failure naming the file, not a panic.
6. **`ArgPolicy`** — the rule set that decides which default arguments apply and where the user's options go
   (`DownloadOptionsAsLast`). You are **not** building the final argv in this phase; you are building the policy that
   Phase 3 will use, and a test proves it resolves the right pieces for a given extension and platform.

### 3.3 `morid-cli engines` — the observable end

One subcommand, no downloads:

```
morid engines            # every bundled extension: name, schema version, platform/arch, resolved command, verdict
morid engines --json     # one JSON object per line, same content
```

Exit codes follow [../cli.md](../cli.md) §3: `0` when every file resolved, `4` when at least one is unavailable (and
the per-file lines say which rule failed). `--json` emits one object per line, no pretty printing, no array wrapper —
the CLI contract fixes this and Phase 3 depends on it.

### 3.4 Tests

In the crates, plus one integration test that runs the CLI:

- `all_shipped_extensions_load` — iterates `extensions/`, names the file and rule on failure (I10). It must pass for
  all 17 on Linux, macOS and Windows CI, at the runner's architecture.
- `invalid_extension_never_spawns` — the negative set below, each producing a validation failure and **no process**
  (I4). Use a test double or an empty `PATH`; do not rely on "we did not call it".
- `min_version_2x_is_accepted`, `min_version_9x_is_refused` — D-004.
- `unknown_keys_are_warned_not_fatal`, `extra_arguments_keys_are_warned`, `thin_file_resolves_through_family`,
  `win7_keys_are_not_used`, `executable_missing_means_needs_installation_not_invalid`.
- `url_round_trips_byte_for_byte` — against the URL shapes in the Phase 0 fixtures.
- `option_tokens_parse_table_driven` — the table in §3.1 point 3.
- `playlist_range_accepts_and_refuses` — the documented forms and the closest invalid neighbours.
- `control_json_structure_evaluates` — positive and negative, including a malformed structure.
- `error_variants_map_to_documented_exit_codes` — every `Error` variant to its code, exhaustively, in one match.

**Negative fixtures you must add** (small files under `crates/morid-ext/tests/data/invalid/`): not JSON · no `Name` ·
`Version` above supported · `RequiredMinimumVersionOfMediaDownloader` `9.x` · `win7`-only file on a supported Windows
target · malformed `Cmd` · malformed `ControlJsonStructure` · `ExtraArguments` present. Each is one rule; a fixture that
trips two rules cannot prove which one ran.

### 3.5 What must not change

- Do not touch `fixtures/`. CI runs the Phase 0 checks; if you change a fixture the job tells you.
- Do not touch `legacy/`.
- Do not add a dependency. `serde` and `serde_json` are already accepted; anything else needs a question first, per
  [../dependencies.md](../dependencies.md).
- Do not edit `docs/extensions.md` or any other prose document. If it is wrong, report it — that is not a courtesy,
  it is the only way the specification stays true.

---

## 4. Constraints

- **No `unsafe`.** `#![forbid(unsafe_code)]` stays in both crates.
- **No `build.rs`.**
- **No async runtime.** Synchronous threads only.
- **No I/O in `morid-core`** — not even `std::fs::metadata`. File existence belongs to `morid-ext`.
- **No spawning anywhere in this phase.** `std::process` appears once in the workspace, in `morid-engine`, and not
  yet.
- **No panics on bad input.** A malformed extension file is a validation result, not an `unwrap`. `#[deny(clippy::unwrap_used)]`
  is not required, but every `unwrap` in non-test code will be questioned in review.
- Errors are typed and carry the file name and the failing rule. "invalid extension" without a rule is a bug.

---

## 5. Acceptance criteria

1. `cargo build --release` and `cargo test` green on Linux, macOS and Windows in CI, links attached; `cargo fmt` and
   `clippy -D warnings` clean.
2. `morid engines` prints one line per bundled file, all 17 resolved on each of the three platforms at the runner's
   architecture — output attached for all three.
3. `morid engines --json` emits exactly one JSON object per line; `jq -c .` accepts every line; no array wrapper.
   Attach the first and last line and `wc -l`.
4. The Phase 0 checks still pass in CI (`Ran … tests`, `VERIFY_OK`) — evidence is the same job, attached.
5. `invalid_extension_never_spawns` and the other named tests exist, pass, and are visible by name in the `cargo test`
   output — paste the list of test names, not just "ok".
6. `grep -rn "std::process" crates/` finds nothing outside `morid-engine`.
7. `grep -rn "std::fs\|std::net\|SystemTime\|rand" crates/morid-core/` finds nothing. If one of those is genuinely
   needed, stop and report — the crate boundary is the point.
8. `cargo tree -p morid-cli` still contains neither `slint` nor `morid-gui`.
9. Unknown keys and `ExtraArguments*` produce warnings that name the file and the key; demonstrate on a copy of
   `ytdl-patched.json`, which has both, and show the warning in `morid engines` output.
9a. The three thin files resolve through their named transformations, not through a family lookup: show, for each of
   `yt-dlp-nightly`, `yt-dlp-ffmpeg` and `yt-dlp-aria2c`, the resolved command and the `DefaultDownLoadCmdOptions`
   they end up with, and confirm the generic `yt-dlp.json` base is unchanged by having been used. A negative control
   is required: a fourth file with only `Name` and `Version` (not one of the three) must be reported invalid, not
   resolved.
10. Every `Error` variant has a test that maps it to a documented exit code.

---

## 6. Testing you must report

Run these and paste the output, not a summary:

- `cargo test` output with test names for the two crates and the CLI integration test.
- `morid engines` and `morid engines --json` on your machine.
- The CI runs for the three platforms, as links.
- The two `grep` checks of criteria 6 and 7, with their exit codes.
- The `ytdl-patched.json` warning demonstration.
- The list of negative fixtures you added, one line each: file, rule it trips, message the user sees.

Report in this order: (1) files added or changed; (2) what is implemented and what is deliberately not; (3) the
outputs above; (4) known problems, including anything you had to guess; (5) every place where this task or a
document contradicts what the code actually does — with the evidence, not an opinion. If a claim in the task is
demonstrably wrong, say so and do not implement it that way.

---

## 7. Documents that change as a result

Updated **by you**, in the same pull request, and only these:

- `docs/roadmap.md` Phase 2 — status paragraph, estimate corrected against the actual.
- `docs/dependencies.md` §6 — the size table, with the measured numbers for both binaries.

Updated by the manager after review: `docs/extensions.md`, `docs/testing.md` §2 (new test names), `docs/cli.md` if
the `engines` output shape needed to differ, and anything else the review finds wrong.

---

## 8. What comes next, so you can design accordingly

Phase 3 builds **one** backend end to end and compares its argv against the Phase 0 goldens. Three decisions you make
now will be judged then:

- **`ArgPolicy` must be data.** Phase 3 assembles argv from it for seven backends; if the policy is a pile of `if`s
  keyed on engine names, Phase 3 becomes seven special cases.
- **Option-token parsing must be platform-independent.** Phase 3 adds the shell quoting per platform on top; a parser
  that bakes in POSIX quoting has to be rewritten there.
- **`Error` must be exhaustively matched.** Exit codes are a public contract, and the mapping is tested once, in one
  place.

If a shortcut here would force `unsafe`, a dependency, or a redesign in Phase 3, take the slower path and say so in
the pull request.
