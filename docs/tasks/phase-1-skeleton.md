# Task 2 for the coder — Phase 1: workspace skeleton and CI

**Status:** not started · **Issued:** 4 November 2026 · **Estimated:** 5–7 person-days
**Requires:** Task 1 (`docs/tasks/phase-0-fixtures.md`) accepted.
**Produces:** a repository that builds, lints, tests and reports size on three platforms, with no features.

---

## 1. Goal

Turn this repository into a Rust workspace that is **continuously verifiable** before it contains anything
useful. At the end of this task `cargo build --offline` succeeds, `morid --version` prints a version, CI is green
on Linux, macOS and Windows, and the C++ tree sits in `legacy/` out of the build path.

This phase exists to make every later phase cheap to check. Its deliverable is not code that does something; it
is a pipeline that will tell us when later code is wrong. Resisting the temptation to add features here is part
of the task.

---

## 2. Context you need

1. [../workspace-manifest.md](../workspace-manifest.md) — **the specification for this task.** The workspace
   `Cargo.toml`, per-crate manifest shape, `rust-toolchain.toml`, `deny.toml` and `.gitignore` are written out
   there. Copy it; do not redesign it. If you believe a value is wrong, stop and report, do not change it
   unilaterally.
2. [../architecture.md](../architecture.md) §1 — the eight crates and, more importantly, the *dependency rule*
   (nothing below `morid` knows a UI exists) and the "must not contain" column.
3. [../roadmap.md](../roadmap.md) — Phase 1, including exit criteria.
4. [../building.md](../building.md) §2–§4 — build, layout, size measurement.
5. [../testing.md](../testing.md) §7 — the exact CI checks.
6. [../platforms.md](../platforms.md) §5 — which runners.
7. [../dependencies.md](../dependencies.md) — the accepted crates, the six-question procedure, and why several
   obvious crates are refused.
8. [../legacy/README.md](../../legacy/README.md) — rules for the directory you are about to create.

---

## 3. What to produce

### 3.1 The move — already done

The move was carried out on 4 November 2026, before this task was issued, so that the repository had its final
layout when Phase 0 started. **Do not repeat it.** Verify it instead:

- `src/`, `translations/`, `images/`, `CMakeLists.txt`, `cmake_uninstall.cmake.in` and the four build scripts are
  in `legacy/`, moved with `git mv` in one commit so that history is preserved.
- `changelog` is now `CHANGELOG-legacy.md` in the root, contents unedited.
- `extensions/` and the four licence files (`LICENSE`, `LICENSE.txt`, `GPLv2`, `GPLv3`) are still in the root;
  [../../legacy/README.md](../../legacy/README.md) §"What is inside" explains why.
- The ten inherited workflows are gone. What each did, and the path references the move broke, are recorded in
  [../../legacy/removed-ci-workflows.md](../../legacy/removed-ci-workflows.md). That file also says how to build
  the C++ tree from its new location without editing it.

Your `git log --follow` check in §5 criterion 5 proves the move was a rename and not a copy. Nothing else about
the layout is yours to change.

### 3.2 The workspace

- Create the eight crates from [../workspace-manifest.md](../workspace-manifest.md) §1, each with the manifest
  shape in §2, each containing a `lib.rs` with a module-level doc comment stating the crate's responsibility and
  the rule it must not break — copied from [../architecture.md](../architecture.md) §1's table.
- `morid-cli` and `morid-gui` are **binary** crates. At the end of this phase:
  - `morid --version` prints `morid 0.1.0` and exits 0;
  - `morid-gui` opens a window containing the text "Morid 0.1.0" and one button that closes it. Nothing else. It
    exists to prove Slint links and the binary is measurable.
- `morid-cli` must **not** depend on `slint` or on `morid-gui`. Verify with `cargo tree`; put the output in the
  pull request. This is the one measurable guarantee that D-009's two-binary split is real.
- Add `rust-toolchain.toml` and `deny.toml` exactly as specified in [../workspace-manifest.md](../workspace-manifest.md)
  §3–§4.
- Commit `Cargo.lock`.
- `xtask` per [../building.md](../building.md) §3: a subcommand that measures and prints both release binaries'
  sizes and appends nothing (writing the table is manual, by the manager). Do not build a task runner framework;
  a `match` on `args` in `main.rs` is enough.

### 3.3 CI

Replace the deleted workflows with **one** workflow, `.github/workflows/ci.yml`, matrix over
`ubuntu-latest` / `macos-latest` / `windows-latest`, running exactly the checks in
[../testing.md](../testing.md) §7: `fmt`, `clippy -D warnings`, `test`, `build --release`, size report,
`cargo deny check`, `cargo audit`.

Requirements:

- a single job definition, not three copies that will drift;
- `cargo build --offline` must pass on a warm cache — add a job step that proves it, or explain in the pull
  request why the step cannot exist on that runner;
- the size report prints the numbers into the job log and into the step summary; it **does not** fail the job
  ([D-008](../decisions.md)). If the report is missing, the job fails — a missing measurement is a failure even
  though a bad measurement is not;
- caches keyed on `Cargo.lock`;
- no secret, no token and no write permission beyond reading the repository;
- total wall-clock under 15 minutes per platform.

**The Phase 0 checks run here too, on one platform only** (D-017 deferred them to this phase rather than dropping
them):

```
python3 -m unittest tools.capture.test_qt_ini_roundtrip     # 16 tests, stdlib only
python3 tools/capture/verify_fixtures.py                    # prints VERIFY_OK
```

Two things about them that are the whole point of putting them here:

- they need nothing but `python3` — no Qt, no network, no fixtures download — so a failure is a real failure, not
  an environment problem;
- the first one is **not** a formality: without `tools/__init__.py` and `tools/capture/__init__.py`, discovery runs
  zero tests and exits 0. If a workflow step can pass while running nothing, it will. Assert that the run reports
  16 tests, not merely exit 0 — count them in the step's output.

If `verify_fixtures.py` cannot run on the runner for a reason you can demonstrate, say so in the pull request and
propose the narrowest alternative that still executes both checks; do not silently skip them.

### 3.4 Documentation of the result

- `README.md`: replace "There is no Rust code yet" with an accurate one-paragraph status — what builds today, and
  that it does nothing yet. Do not overstate.
- `docs/roadmap.md` Phase 1: mark it done only after CI is green on all three platforms, and correct the estimate
  against the actual.
- `docs/dependencies.md` §6: append the first real row with the measured sizes.
- `docs/workspace-manifest.md`: if any value had to differ from the specification, change the specification in the
  same pull request and state why in the description.

---

## 4. Constraints

- **No functionality.** No URL parsing, no extension loading, no process spawning, no settings reading. The
  crates are empty by design; their shape is the deliverable.
- **No new dependency** beyond the accepted list in [../dependencies.md](../dependencies.md). Every one carries
  the six-question argument in the pull request. If you need a dependency that is not listed, **stop and ask** —
  do not add it and explain afterwards.
- **No `build.rs`** anywhere.
- **No `unsafe`.**
- **No async runtime.** The design is synchronous threads ([../architecture.md](../architecture.md) §6).
- **Do not touch `legacy/` beyond the move**, and do not "tidy" the C++ code while it is in your hands.
- **Do not touch the prose documents** other than the four named in §3.4. If another document is now wrong,
  report it.
- **Do not reformat the moved tree.** `git diff --stat` for the move commit should show renames, not edits.
- Platform-specific dependencies (`libc`, `windows-sys`) go in `morid-engine` under
  `[target.'cfg(...)'.dependencies]` and nowhere else.

---

## 5. Acceptance criteria

1. `cargo build --release` succeeds on Linux, macOS and Windows; `cargo build --offline` succeeds from a warm
   cache.
2. `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, `cargo test`, `cargo deny
   check`, `cargo audit` all pass locally and in CI on all three platforms. Links to the green runs are attached.
3. `morid --version` prints `morid 0.1.0`; `morid-gui` opens a window and closes without a crash on each of the
   three platforms (screenshot or CI artefact for the two that have one).
4. `cargo tree -p morid-cli` contains neither `slint` nor `morid-gui` — output attached.
5. `git log --follow legacy/src/main.cpp` shows the original history, i.e. the move used `git mv`.
6. `ls .github/workflows/` shows exactly one file; the root has no `src/`, no `CMakeLists.txt`, and
   `extensions/` and the four licence files are still there.
7. `Cargo.lock` is committed, `target/` is not.
8. `deny.toml` exists and `cargo deny check` is actually enforcing it — demonstrate by adding a crate with a
   disallowed licence locally, showing the failure, and reverting it. Paste the output.
9. The size report step prints both binary sizes in the CI log and in the step summary; the job does not fail
   because of the value.
10. The repository builds on a machine with **no C compiler** installed — demonstrate on Linux by uninstalling or
    hiding `cc`/`gcc` and building, or explain precisely why that test cannot be performed in your environment
    and what you did instead.
11. Total CI wall-clock per platform is under 15 minutes; the actual times are reported.
12. The workflow runs the Phase 0 checks and **reports 16 tests**, not just exit 0: paste the step where the count
    appears. A run that discovers nothing must fail the job — demonstrate the guard by pointing the step at an
    empty test pattern locally and showing it fails.

---

## 6. Testing you must report

- The five CI checks, run locally on your machine, with the commands and their exit codes.
- The three-platform CI results, with links.
- `cargo build --offline` with the network disabled, and how you disabled it.
- `cargo tree` for both binaries.
- The deliberate `cargo deny` failure experiment from criterion 8.
- The no-C-compiler build from criterion 10.
- A `git diff --stat` of the move commit showing renames rather than content changes.
- The first size measurement, with the raw `ls -l` output.

Report in this order when you hand the task back:

1. files added, moved or deleted;
2. description of what was built and what it does not do;
3. the results above, with actual output;
4. known problems;
5. anything in this task you did not do, and anything in the documents that contradicts what you found.

---

## 7. Documents that change as a result

Updated **by you**, in the same pull request, and only these: `README.md` status paragraph,
`docs/roadmap.md` Phase 1, `docs/dependencies.md` §6, `docs/workspace-manifest.md` (only to correct the
specification where reality differed, with the reason stated).

Updated by the manager after review: everything else, including
[../testing.md](../testing.md) §7 if the CI shape changed, and
[../building.md](../building.md) if the build procedure changed.

---

## 8. What comes next, so you can design accordingly

Phase 2 loads all 17 extension files and resolves a command **without spawning anything**
([../roadmap.md](../roadmap.md) Phase 2). The decisions you make now that will be judged then: `morid-core` has
no I/O, `morid-ext` has no network, and errors are typed rather than strings. If a shortcut in this phase would
make Phase 2 need `unsafe`, a dependency, or a redesign, choose the slower path now and say so in the pull
request.
