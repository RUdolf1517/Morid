# Task 1 for the coder — Phase 0: reference behaviour and fixtures

**Status:** accepted with conditions, 5 November 2026 · **Issued:** 4 November 2026 · **Estimated:** 5–8
person-days · **Delivered:** PR #2, commit `8fba615`
**Requires:** nothing to exist first. **Must finish before** any Rust code is written.

---

## 0. Review verdict (5 November 2026)

Checked by running the verifier and by independent inspection, not by reading the report.

**Accepted:** 36/40 captured with wrapper transcripts; all invariant, hash, manifest, userdata and
comparator checks pass when re-run (`VERIFY_OK`); `legacy/` is untouched; no Rust, no Cargo artefacts, no
media, no cookie or credential residue in the fixtures.

**Not accepted as delivered, to be fixed in the same branch before merge:** G1–G4 below.

| # | Finding | Why it matters |
| --- | --- | --- |
| G1 | 22 of the 36 goldens in the committed fixtures contain absolute capture-host paths — `/home/user/.cache/morid-capture/runtime/...` — and `fixtures/README.md` claims that only disposable **workspace** paths were replaced. The claim is true for `userdata/`, not for the scenario goldens. | The fixture is not reproducible, so `candidate == golden` is unsatisfiable on any other machine; this is the core artefact of Phase 0. Fixed as T1 below. |
| G2 | 23 goldens contain the capture host's `node:/usr/local/bin/node`. | The application derives this path at run time; the golden must not depend on it. Same fix as T1. |
| G3 | `env.json` exists for s13/s14/s15 but nothing verifies it, and s14 (manual proxy) records `{}` while s15 records the proxy string. | The V of "proxy credentials are passed, not interpolated" is only partly evidenced; an unverified file that looks like evidence is worse than no file. Fixed as T3. |
| G4 | `s28-auto-update-false` is `not-captured` with `backend: wget` while nothing is spawned, and `s35-unknown-extension` reports `backend: custom` while its transcript shows `yt-dlp` running with a full argument vector. | Metadata that names a backend that did not run misleads whoever reads the fixture next. Fixed as T4. **Corrected 5 November:** an earlier draft of this row also accused `s36` of the same thing. That was wrong — `s36` really does run `capture-custom`; its actual defect is the written reason, not the backend field. |

Two smaller things, accepted as they are: `tools/capture/capture_userdata.py` keeps an existing
`fixtures/userdata` unless `--force` is given, so re-running it is a no-op rather than a re-capture — this is
stated in `fixtures/README.md` and is the right trade (regenerating timestamps would create false diffs).
`legacy/images/` and the Qt icons are photographs and artwork, not fixtures; the "no binary artifacts" check
is correctly scoped to `fixtures/`.

The four `not-captured` cases are accepted as real observations, not as gaps in effort. s33 (interpreter
invocation exists only in the Windows branch) and s28 (no external update argv exists) are real observations.
s35 and s36 are *not* one observation: s35 falls back to the default engine after rejecting an unknown name,
while s36 executes an extension whose `Name` is missing. They are recorded separately in
[../migration.md](../migration.md) §6.1 as O1 and O1b.

---

## 1. Goal

Produce the evidence that later phases will be judged against: for a fixed list of user scenarios, the **exact
argument vector** the original C++ program hands to a backend, and the **exact bytes** a backend produces in
reply. No Rust, no refactoring, no new features.

If this task is done badly, every later phase is unverifiable and the rewrite degrades into "looks about right".
If it is skipped, the project has no definition of "same behaviour" at all.

Why this is first: the original has no automated tests ([../audit-original.md](../audit-original.md) §2.13,
defect D4). Its only verification is a hidden mode that spawns real backends over the network. There is
therefore no artefact today that says what correct behaviour is.

---

## 2. Context you need

Read, in this order, before writing anything:

1. [../audit-original.md](../audit-original.md) §2.5 (the extension schema), §2.6 (the download pipeline),
   §2.8 (network), §2.13 (defects D1–D10).
2. [../migration.md](../migration.md) — whole document. §3 is your specification; §5 and §6 are the list of
   places where "identical" is *not* the goal.
3. [../testing.md](../testing.md) §4 — fixture format, provenance and anonymisation.
4. [../extensions.md](../extensions.md) §3–§5 — how a command is selected and how output is split into lines.
5. The code: `legacy/src/utility.h` (`context`, ~1873–2135), `legacy/src/engines.cpp` (command assembly, proxy
   injection), `legacy/src/engines/*.cpp` (`updateDownLoadCmdOptions` in each), `legacy/src/settings.cpp`.

The C++ tree was moved into `legacy/` in commit `9f1c0d5` (the Phase 1 move, done ahead of this task because the
repository had to be in its final layout before Phase 0 started). The paths above are correct as written.

You will have to **build and run the C++ program**; that is expected and is the only permitted reason to build
it. See [../building.md](../building.md) §7. Record the Qt version you used.

---

## 3. What to produce

Everything goes under a new top-level directory `fixtures/`. It is committed. It is small: no binaries, no
media, no cookies.

```
fixtures/
├── README.md                 how the set was captured, how to re-capture, what is missing
├── manifest.json             the index: every case, its feature ID, status, files
├── scenarios/
│   └── <case-id>/
│       ├── argv.json         the argument vector, as a JSON array of strings
│       ├── meta.json         backend + version, app version, OS/arch, capture date
│       ├── log.txt           the raw debug log the argv was read out of
│       └── notes.md          only when the case is deliberately different from the original
├── streams/
│   └── <backend>/
│       ├── version.txt
│       ├── list.json
│       ├── progress.txt
│       ├── destination.txt
│       ├── complete.txt
│       ├── failure.txt
│       ├── interrupted.txt
│       └── non-utf8.bin
└── userdata/
    └── media-downloader/     an anonymised data directory (settings.ini, JSON files, archive)
```

### 3.1 `scenarios/` — the argument vectors

About **40** cases. Each case ID is `s<NN>-<short-name>`, stable forever, and each is tied to a feature ID from
[../features.md](../features.md). The minimum set, which may be extended but not reduced:

| # | Case | Feature | What must be visible in the argv |
| --- | --- | --- | --- |
| 01 | plain yt-dlp download, default options | F-03 | engine defaults, output template, `--newline`-class flags |
| 02 | download with a bundled preset | F-06 | preset option string placement |
| 03 | download with user-supplied `-f 22` | F-02 | user options versus engine defaults order |
| 04 | download with two conflicting user options | F-02 | order preserved, last occurrence wins where the backend says so |
| 05 | format listing | F-01 | list command, `DumptJsonArguments` if any |
| 06 | playlist URL, whole playlist | F-20 | playlist versus single-video flags |
| 07 | playlist URL, item range | F-21 | range syntax as the engine expects it |
| 08 | playlist URL with `--no-playlist` forced by the user | F-21 | last-wins order |
| 09 | download with download-archive | F-23 | `--download-archive` path form |
| 10 | batch download of two URLs | F-10 | whether URLs go as separate arguments or by `BatchFileArgument` |
| 11 | cookies from a browser | F-55 | cookie argument per engine |
| 12 | cookies from a file | F-55 | path form, quoting |
| 13 | proxy: mode `None` | F-54 | nothing added |
| 14 | proxy: mode `Manual`, `http://user:pass@host:8080` | F-54 | how credentials are passed, argument versus environment |
| 15 | proxy: mode `Env` | F-54 | which variables are set |
| 16 | subtitle download enabled | F-08 | `DefaultSubstitlesCmdOptions` (spelling preserved) |
| 17 | subtitle languages specified | F-08 | token splitting |
| 18 | comments download | F-09 | engine-specific flag |
| 19 | chapters split | F-15 | naming template with chapter fields, `--download-sections` |
| 20 | output directory with a space and a non-ASCII name | F-50 | path passing, no shell |
| 21 | filename template containing `%(title)s` from user text | F-50 | template passed through, not expanded by us |
| 22 | URL containing `&`, `?`, `#`, `%` | F-50 | no URL decoding, no splitting |
| 23 | URL-manager rule selects the engine and its options | F-52 | which extension is chosen and where its options land |
| 24 | custom extension, template text with `${...}` substitution | F-39 | substitution result |
| 25 | extension with `Cmd` for the current OS only | F-31 | correct branch selected |
| 26 | extension with `Cmd` for a different architecture | F-31 | fallback behaviour, whatever it actually is |
| 27 | extension with `ExtraOptions` | F-30 | where they land in the order |
| 28 | extension whose `AutoUpdate` is false | F-34 | update command not produced |
| 29 | gallery-dl, default options | F-31 | its own command and defaults |
| 30 | gallery-dl with a per-engine default option | F-02 | placement relative to engine defaults |
| 31 | wget | F-31 | `ReplaceOutputWithProgressReport` effect on argv |
| 32 | aria2c | F-31 | CR-based output engine's argv |
| 33 | you-get | F-31 | interpreter invocation |
| 34 | svtplay-dl | F-31 | its own flag set |
| 35 | unknown extension name | F-30 | error path; empty argv is the expected result |
| 36 | extension with a missing required key | F-30 | load failure, empty argv |
| 37 | yt-dlp with `-x` and an audio format | F-02 | conversion flags, ffmpeg reference |
| 38 | command configured to run after a download | F-60 | the second command's argv and where its output goes |
| 39 | non-UTF-8 encoding setting | F-56 | `EncodingArgument` and the decoder behaviour |
| 40 | Flatpak/AppImage packaging flag | F-30 | `ExtraOptionsFlatpakAppImage` inclusion |

Every feature ID above exists in [../features.md](../features.md) — all of them are `not started`, and a row
without a captured case cannot become `done` later. Some features are deliberately **not** in this set because
they produce no argument vector: history (F-07), themes (F-57), UI scale (F-58), tray (F-59), window geometry
(F-67), library (F-65), thumbnails (F-14). Their evidence is of another kind and belongs to their own phase.

**Where the source list comes from:** the task table above is the specification; the scenario files themselves do
not exist yet and are not supposed to exist before you work. `docs/features.md` gives the feature ID behind each
row, `docs/extensions.md` §3–§5 gives the schema keys named in the right-hand column. If a row names a feature ID
that `docs/features.md` does not contain, report it rather than inventing a scenario.

For each case, `argv.json` is produced by the program under test. **How to obtain it** — pick whichever works
and say which you used in `fixtures/README.md`:

- the debug/verbose log of the original, which echoes the command;
- a wrapper backend: a script that writes `"$@"` to a file, one argument per line, and exits 0. Point the
  extension's `Cmd`/`BackendPath` at it. This is the preferred method: it captures `argv` exactly, with argument
  boundaries, which a log line cannot.

If neither works for a case, say so and mark it `"status": "not-captured"` with the reason. A gap that is
documented is acceptable; a gap that is papered over with a hand-written array is not.

### 3.2 `streams/` — backend output

Raw, unedited bytes, one file per situation, per backend that Morid will support: yt-dlp, gallery-dl, wget,
aria2c, you-get, svtplay-dl, get-sauce. Add `lux` and `safaribooks` if you can produce output; if not, say so.

Capture on a URL class you can describe without revealing the URL: "public domain video, small", "image board
gallery, 5 files", "audio-only", "unavailable video", "geo-blocked video", "live stream interrupted after 5 s".
Store the class in `meta.json`, never the URL.

Also capture, for at least yt-dlp, the exact bytes of a stream with `\r` in it (progress), a partial line at the
end of a chunk, and a non-UTF-8 line (`non-utf8.bin`). These drive the line-splitting and encoding code later.

### 3.3 `userdata/` — a real data directory, anonymised

One directory laid out as the original expects, containing every file listed in [../data-compat.md](../data-compat.md)
§2: `settings.ini` with **all** keys the current version writes (do not trim to the interesting ones),
`presetOptions.json`, `downloadDefaultOptions.json`, `downloadEngineDefaultOptions.json`, `subscriptions.json`,
`autoSavedList.json`, `downloadHistory.json` (at least 20 entries), `archiveFile-yt-dlp.txt`, and one theme.

Anonymise: replace real URLs with `https://example.com/...`, remove cookie values or replace them with
`REDACTED`, replace home directory paths with `/home/user` and `C:\Users\user`, remove any email address or
token. Then verify by grepping the directory for the real values before committing.

### 3.4 The comparison procedure

A script that, given a case ID, prints the golden argv and a candidate argv side by side and exits non-zero on
difference — argument order significant, quoting not. In Phase 0 the candidate side may be empty; the script is
written so that Phase 3 can point it at `morid --dry-run` without modification.

Where it lives: `tools/capture/` (capture helpers, dev-only) and `tools/compare-argv` (the comparison). Shell or
Python 3 is acceptable here — this is tooling, not the product, and it must not become a Cargo dependency. Note
in `fixtures/README.md` that it will be folded into `xtask` in Phase 1 if it is still useful then.

---

## 4. Constraints

- **Do not modify the C++ tree** in `legacy/`. Its behaviour is the reference. If it is wrong, record the defect in
  `fixtures/README.md` and cross-reference [../migration.md](../migration.md) §6.
- **Do not write Rust.** No workspace, no crates, no `Cargo.toml`. That is Phase 1. This task is deliberately
  possible without a Rust toolchain.
- **Do not write documentation beyond `fixtures/README.md`.** Prose documents are updated by the manager after
  review; if a document is now wrong, report it, do not edit it.
- **No binaries, media files, cookies, tokens or personal data** in the repository. Check file sizes.
- **Never invent a fixture.** Every `argv.json` traces to a log or a wrapper capture. Every stream file traces to
  a real run. If you could not run something, record why.
- **Do not "fix" the scenario list by dropping hard cases.** A case that turns out to be impossible to capture is
  marked `not-captured` with a reason.
- Keep the whole `fixtures/` tree under **2 MB**, except `userdata/` which should stay under 200 KB.

---

## 5. Acceptance criteria

The task is accepted only when all of these hold, and each is checked by the reviewer, not taken on trust:

1. `fixtures/manifest.json` lists every case from §3.1, and each entry has `feature`, `status` (`captured` or
   `not-captured`), `method` (log / wrapper / other) and the files it produced.
2. At least **35 of 40** cases have status `captured`; every `not-captured` has a stated reason, not "not
   attempted".
3. Every captured case has a `log.txt` or wrapper transcript that contains the same arguments as `argv.json` —
   the reviewer will spot-check at least five cases against it.
4. `streams/` covers all seven required backends with at least version, progress and failure output, and includes
   the three awkward cases (CR, partial line, non-UTF-8).
5. `userdata/` loads in the original program (the reviewer will run it) and contains no real URL, path, cookie or
   token — this is verified by grep, not by assertion.
6. Running the comparison script on any captured case with the golden argv as candidate exits 0; with one
   argument moved, exits non-zero.
7. `fixtures/README.md` explains how to re-capture, what the wrapper method is, which Qt and backend versions
   were used, and lists the known gaps.
8. `git status` is clean after a re-capture with the same versions — a second run produces no diff (except
   metadata that is explicitly excluded, and it must then be excluded in the script, not by hand).

---

## 6. Testing you must report

- The comparison script's own behaviour: the two checks in criterion 6.
- An idempotency run: recapture, `git diff`, and the explanation of anything that changed.
- Whether `userdata/` is accepted by the original program: which version, on which platform.
- Every case you could not capture, with the command you ran and the error you got.

Report the following, in this order, when you hand the task back:

1. files added or changed;
2. description of the implementation (how capture works, what the wrapper does);
3. results of the checks in §5 and §6 — actual output, not a summary;
4. known problems;
5. anything from this task you did not do.

---

## 7. Documents that change as a result

Updated by the manager after the work is reviewed, not by you:

- [../testing.md](../testing.md) §4 — fixture inventory and provenance become real instead of planned.
- [../migration.md](../migration.md) §3 — any capture method that turned out not to work.
- [../roadmap.md](../roadmap.md) §1 — Phase 0 marked done, estimate corrected against actual.
- [../features.md](../features.md) — a case that revealed an undocumented behaviour gets its note.

Report any inconsistency you find between these documents and the real behaviour of `legacy/`; do not resolve
it yourself.

---

## 8. Follow-up tasks before merge (added 5 November 2026)

Scope: the review findings G1–G4 in §0. Everything else stays as delivered. Do not re-capture anything
that is not listed here, and do not change the scenario list, the reasons of the four `not-captured` cases,
or the raw streams.

### T1 — make the goldens machine-independent (G1, G2)

The problem is not that the paths are absolute; it is that they encode one capture host, so a golden can
only ever match on that host. Fix it by role, not by deleting information.

1. In `tools/capture/capture.py`, when writing `argv.json` (and `argv-N.json`, `post-argv.json`,
   `invocations.json`, `observed-argv.json`), canonicalise each argument:
   - the scenario's runtime root `<runtime>/<case-id>/` becomes the token `<case-dir>`; all deeper paths
     keep their relative tail, e.g. `<case-dir>/appdata/data/archiveFile-yt-dlp.txt`,
     `<case-dir>/downloads`, `<case-dir>/working`;
   - the mirror `/home/user` used by the anonymised userdata stays `/home/user` (it is already a fixed,
     documented placeholder);
   - the host's interpreter path in `node:<path>` becomes `node:<node>`;
   - nothing else is rewritten. Argument count and order must not change; if a substitution can alter
     either, the capture fails loudly instead of writing a fixture.
2. Canonicalise the same way inside `log.txt` / `post-log.txt` JSONL `program` and `cwd` fields, so the
   transcript and the golden stay byte-comparable to each other.
3. Leave `meta.json`'s `command` arrays in `fixtures/streams/**` alone: they already carry `<local-url>` and
   are explicitly a record of how a stream was produced. Do not touch raw stream bytes at all.
4. State the vocabulary in `fixtures/README.md` (a short "Placeholders" section: `<case-dir>`, `<local-url>`,
   `<node>`, `/home/user`), and say that a golden contains no other host-specific path.
5. `tools/compare-argv` must compare canonicalised goldens against a candidate that is **not** canonicalised
   — i.e. a future `morid --dry-run` prints real paths. Add a `--canonicalise` step to the comparator that
   applies the same substitutions to the candidate, using the candidate's own case directory as
   `<case-dir>`. Without this the comparator cannot work on another machine, which is the whole point.
6. Re-run the full capture, then the verifier, then a second capture and confirm `sha256_diff=empty` and
   `git_diff=empty`.

Acceptance: `grep -r "/home/user/.cache" fixtures/scenarios` returns nothing; every captured `argv.json`
contains at most the placeholders above; the comparator's positive run still exits 0 and its negative run 1;
the two consecutive captures produce no diff.

### T2 — prove the idempotency claim, do not assert it

`fixtures/README.md` says a second normal run produces no fixture diff, and your report says it did. Add the
measurement to the tooling so it can be repeated by a reviewer: a `tools/capture/check-idempotency.sh` that
runs the capture twice, hashes `fixtures/` before and after (sorted `sha256sum` manifest, excluding nothing)
and diffs the two manifests plus `git status --porcelain`. Print `IDEMPOTENCY_OK` or the first differing path.

Acceptance: the script exists, is executable, and its output is pasted into the hand-back along with the
count of files hashed.

### T3 — make `env.json` mean something, or remove it (G3)

1. Extend `verify_fixtures.py` to check, for every scenario that has `env.json`: the file is an object of
   string values; its keys are a subset of the wrapper's recorded `PROXY_VARIABLES`; and it equals the `env`
   object of the first transcript record for that case (which is what the wrapper actually observed).
2. Then explain, in the same file or in `fixtures/README.md`, why s14 (manual proxy with credentials) records
   an **empty** `env` while s15 (env mode) records the proxy string. Both are plausible — manual mode passes
   `--proxy-user/--proxy-password` as arguments, env mode passes variables — but the fixture must say so, and
   the check must confirm the recorded evidence matches that explanation rather than merely agreeing with
   itself.
3. If, after the check, `env.json` turns out to add nothing beyond the `env` field already present in
   `log.txt`, delete the three `env.json` files, drop them from `manifest.json` and say so. Evidence that
   duplicates evidence is clutter.

Acceptance: either the strengthened check passes with the explanation written down, or the files are gone
and the manifest is consistent; `verify_fixtures.py` output shows the new check by name.

### T4 — correct the metadata, and one wrong reason (G4)

Corrected on 5 November after the coder questioned this section. The first version told you to record
`backend: yt-dlp` for **both** s35 and s36. That was wrong for s36. The transcripts are the authority:

| Case | Program actually executed (from `log.txt`) | First argument vector |
| --- | --- | --- |
| s35 | `yt-dlp` | full yt-dlp vector with `-o`, `--download-archive`, URL |
| s36 | `capture-custom` | `["https://example.com/v/fixture-01"]` — the URL alone |

1. `s35-unknown-extension`: `backend` becomes `yt-dlp`; add
   `requested_extension: "extension-does-not-exist"` and
   `observed_behaviour: "unknown extension name rejected; fell back to the default engine"`. `status` stays
   `not-captured`, because the expected empty argv did not occur.
2. `s36-missing-required-key`: `backend` becomes `capture-custom`; add `requested_extension: null` (no usable
   identifier exists — a file name is not a `Name`) and `extension_file: "missing-name.json"`. **Rewrite the
   reason.** The current text claims a fallback to another engine; nothing fell back. The original loaded the
   extension with `Name` removed and executed it. Use:
   `"the extension with Name removed was still loaded and executed; the captured argv is the URL alone and no
   fallback occurred, so the task's expected empty argv is not the reference behaviour"`.
3. `s28-auto-update-false`: `backend` must not name an engine that is never spawned. Use `null` (or omit it),
   add `subject: "AutoUpdate / versionInfo"`, set `backend_version: null`, keep `backend_invocations: 0`.
4. `verify_fixtures.py`: a `null` backend must never reach a `PROGRAMS[...]` lookup, and a `not-captured` case
   with `backend_invocations == 0` must not name a spawned backend.

Acceptance: the verifier passes; the three `meta.json` files read correctly to somebody who has not seen this
task; the s36 reason matches what its own `log.txt` and `app.log` show; the manifest is unchanged apart from
file lists, if any changed.

**Do not "fix" s36 by putting `Name` back into the extension.** The case exists to record what the original
does with an invalid extension; making the input valid would delete the observation.

### T5 — write down what Phase 0 taught, in the fixtures README

Add a short section "Behaviour discovered, not specified" to `fixtures/README.md`, listing at least:
s35/s36 fallback instead of empty argv; s26 fallback by architecture; s10's two extra metadata commands
(and the decision to disable metadata preview so the golden holds the download command); s33's Linux-vs-
Windows interpreter difference; s28's update path having no argv. One line each, each pointing at the case
id. This is the raw material for [../migration.md](../migration.md) §6 and [../features.md](../features.md);
the manager will fold it in, you do not edit those files.

### Reporting

For this follow-up, report only: the files changed, the four acceptance outputs (T1 grep, T1 comparator,
T2 idempotency, T3/T4 verifier), the canonicalisation vocabulary you settled on, and anything in these
instructions that turned out to be wrong. No summary of the previous delivery.
