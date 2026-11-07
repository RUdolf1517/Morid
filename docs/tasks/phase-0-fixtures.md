# Task 1 for the coder — Phase 0: reference behaviour and fixtures

**Status:** accepted with conditions, 5 November 2026 · **Issued:** 4 November 2026 · **Estimated:** 5–8
person-days · **Delivered:** PR #2, commit `8fba615`
**Requires:** nothing to exist first. **Must finish before** any Rust code is written.

---

## 0. Review verdict (round 2, 5 November 2026)

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

### Round 2 — what changed, and what is still open

The follow-up commit `b344cdc` was re-checked independently, not by reading the report.

**Verified fixed, reproduced by hand:**

| Check | Result |
| --- | --- |
| G1 host paths in goldens | `grep -r /home/user/.cache fixtures/scenarios` returns nothing (exit 1); no `argv*.json` contains an absolute path |
| G2 Node path | all 23 occurrences are `node:<node>` |
| Placeholder discipline | only `<case-dir>` (39 files) and `<node>` (24 files) appear; the one remaining literal is `/home/user/Downloads` in s20's `cwd`, a documented token |
| G3 `env.json` | the three files are gone; proxy evidence is verified from the transcript `env` objects, and s14/s15 behave as described |
| G4 metadata | s28 `backend: null` + `subject`; s35 `yt-dlp` + `requested_extension`; s36 `capture-custom` + `null` + rewritten reason |
| Comparator | positive 36/36; a foreign candidate (`/home/ci/...`, `node:/opt/node/bin/node`) matches with `--canonicalise`; a changed path tail, a split case root or a path outside the case dir each exit 1 or 2 correctly |
| Regeneration honesty | I diffed the old goldens against the new ones with my own canonicaliser: exactly three arguments changed, all because the harness path moved inside the case directory (s09, s12, s20). No golden was edited by hand. |
| Idempotency | now backed by `check-idempotency.sh`. I did **not** re-run it — it needs a Qt build this environment does not have — so that single result rests on the coder's output. Said plainly rather than implied. |

**Still open — two findings, both missed in round 1:**

| # | Finding | Why it matters |
| --- | --- | --- |
| **G5** | **The verifier does not check goldens for host paths at all.** I injected `/home/user/.cache/...` back into `s01`'s `argv.json` *and* its `log.txt` in a copy of the tree; the verifier still printed `VERIFY_OK`. | T1 is enforced only by a `grep` that runs in nobody's CI, so the exact regression it was written against can return unnoticed. The check belongs in `verify_fixtures.py`. Task **T6**. |
| **G6** | **s20's `cwd` is the capture host's real Downloads directory, and the explanation in the report (and in my first version of this row) is wrong.** The report said the original "ignores `DownloadFolder` and takes `QStandardPaths::DownloadLocation`" when `-P` is given; I then said the cause was the locale codec. Both are wrong. `-P` is never consulted for the working directory (`utility::context::whenCreated` uses `m_events.downloadFolder()` → `settings::downloadFolder()`), and that function falls back to `downloadLocation()` **only when the configured path does not exist** (`legacy/src/settings.cpp:688-728`). The real mechanism is in [data-compat](../data-compat.md) §3.0: the original writes non-ASCII values as Qt `\x<hex>` escapes and reads them back through Qt's unescaper — so a `settings.ini` written by our harness as **literal UTF-8** is decoded by the original into a path that does not exist, and the folder is reset. The harness, not the original, is what is wrong here. | The case does not reproduce what it is named for: s20 exists to prove a non-ASCII download path works, and instead proves the reset path, because the harness wrote the file in a dialect the original cannot read. Task **T7** fixes the harness; the compatibility rule that produced this finding is D-016. |

### Round 3 — verdict on `f187923` (5 November 2026)

Re-checked by running the verifier on a fresh `git archive` of the branch, and by reading the Qt 5.15 reader and
writer in `qtbase/src/corelib/io/qsettings.cpp` rather than the documentation prose.

**Accepted.** `VERIFY_OK`, byte-for-byte identical output to the coder's report. T7's fix works and is
demonstrated honestly: the coder ran it *before* and *after* with everything else unchanged, showed the before
state losing the folder, and did not claim the log line that I had wrongly told them to look for.

| Check | Result |
| --- | --- |
| T6 host-path check exists and runs | `host_paths checked_files=126 allowed_absolute_paths=0 outside_vocabulary=0`, printed by name |
| T6 negative | re-injected `/home/user/.cache/morid-capture/runtime/s01/...` into `argv.json` **and** `log.txt` myself: `host_paths outside the fixture vocabulary (...argv.json, ...log.txt)`, exit 1; clean tree returns `VERIFY_OK`. A second injection with a Cyrillic path outside the vocabulary (`/srv/Проекты/загрузки/...`) also fails, so the check is not ASCII-only |
| T7 before/after | reproduced in the coder's log; `cwd` moves from `/home/user/Downloads` to `<case-dir>/output dir/Загрузки` and stays there |
| T7 side effect | the original, on the *before* run, rewrote the value itself as `\xd0\x97\xd0\xb0...` — the UTF-8 bytes `D0 97` read as Latin-1 and written back per code unit. This is direct evidence for the mechanism in [data-compat](../data-compat.md) §3.0, better than my documentation-based argument |
| Blast radius | `git diff --stat b344cdc f187923` = 6 files, +119/−8; 39 of 40 cases untouched |
| Idempotency | re-run on the new tree: `IDEMPOTENCY_OK files=191`, `sha256_manifest_diff=empty` |

**Corrected — the coder is right and I was wrong about the log line.** `settings::downloadFolder()` passes a
default-constructed `sLogger`, whose `add()` is a no-op when its `Logger*` is null (`legacy/src/settings.cpp:1631`).
The reset is therefore **not** logged: the `logger.add(tr(Settings::Resetting download folder to default))` path
in `downloadFolderImp` only produces output when a `Logger&` overload is called. My acceptance criterion "the log
contains no 'Resetting download folder to default'" was satisfied by every run ever made, including the broken
ones. The coder replaced it with the observable facts — `cwd` and the rewritten `DownloadFolder` in
`settings.ini` — which is the correct choice. Recorded as **O8** in [../migration.md](../migration.md) §6.1.

**Still open — one finding (G7), one self-inflicted defect in the verifier (G8):**

| # | Finding | Why it matters |
| --- | --- | --- |
| **G7** | **`escape_qt_ini_value` is correct for the values we have and wrong for one class we do not.** Qt 5's writer sets `escapeNextIfDigit` after any `\x` escape and then re-escapes a following ASCII digit or hex letter as `\x<ch hex>` (`qsettings.cpp`, `iniEscapedString`). So Qt writes `Загрузки5` as `...\x438\x35`, while our helper writes `...\x4385`, which Qt's reader turns into `Загрузк䎅` — the folder is lost again, silently. I verified the round trip with Qt's own unescape algorithm: `/x/Загрузки5`, `/x/Папка1`, `/x/тестa`, `/x/é5` all break; `/x/Загрузки` and `/x/Загрузки-5` survive. This is the exact silent-loss failure s20 was built to catch, one character away from the current fixture. | Any download folder whose name contains a non-ASCII character followed by a digit or an `a-f`/`A-F` letter round-trips incorrectly. A user folder named `Отпуск2024` or `фото1` is not exotic. Task **T9**. |
| **G8** | I told the coder "do not invent quoting rules", and they obeyed too literally: `escape_qt_ini_value` does not quote. Qt 5 quotes a value when it contains `;`, `,` or `=`, or begins or ends with a space (`iniEscapedString`, `needsQuotes`). A path containing a comma is written by the original as `"..."` and read back with the quotes stripped — our writer without quoting would produce a value the original reads *including* the quote characters. Not exercised by any current case. | Combined with G7, the harness's writer differs from Qt's in two documented ways, so nothing in the fixture set may be called "the Qt dialect" yet. The verifier also only sanity-checks one hardcoded Cyrillic string, so it cannot catch either divergence. Task **T10**. |

**Also recorded, not a defect:** `fixtures/streams/**/meta.json` keeps `/home/user/.cache/morid-capture/...` inside
`command` arrays (four backends), and the new `host_paths` check is scoped to `fixtures/scenarios` only — so it
does not fire on them. This is the same deliberate exclusion as in round 2, now load-bearing: if someone widens
the check to all of `fixtures/`, those provenance records must be exempted explicitly rather than by accident.

**Standing environment limitation, confirmed twice:** the capture cannot be re-run in the review environment —
no Qt prefix, no driver, no legacy binary, and `download.qt.io` / `mirror.qt.io` are outside the egress
allowlist here (TLS failure and no resolution, respectively). `check-idempotency.sh` therefore exits 2 at
`QT_PREFIX`, in this round and the previous one. Any acceptance criterion that requires the capture to be re-run
is unprovable from here and must say so, or be replaced by an argument from the code — as T12 does for the writer.

**Recorded so it is not raised again:** `fixtures/streams/**/meta.json` keeps the capture host's backend paths inside `command` arrays. T1 excluded them deliberately — those arrays are provenance, not goldens, and rewriting them would make the record of the capture untrue.

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

### T6 — make the verifier enforce the placeholder rule (G5)

1. In `verify_fixtures.py`, add a check over every file under `fixtures/scenarios/**`: reject any absolute
   path outside the documented vocabulary — in particular any occurrence of `/home/user/.cache`, of
   `/morid-capture/`, of the capture runtime root, and any absolute path whose prefix is neither `/home/user`
   nor `<case-dir>`.
2. Prove it works: temporarily inject `/home/user/.cache/morid-capture/runtime/s01/...` into `s01`'s
   `argv.json` **and** `log.txt`, show the verifier failing with the file and the value named, then revert.
   Paste both outputs. Editing both files matters: it leaves transcript equality intact, so only a path
   check can catch the regression.
3. The new check prints its name in the verifier's output, so a reviewer can see it ran.

Acceptance: the injection fails loudly, the clean tree prints `VERIFY_OK`, the check is visible by name.

### T7 — s20: write `settings.ini` in the dialect the original reads (G6)

The cause is established and is not the locale. The harness writes `settings.ini` as literal UTF-8
(`capture.py`, `settings_path.open("w", encoding="utf-8")`). The original reads that file through `QSettings`
with no `setIniCodec`, so under Qt 5 it expects Qt-escaped ASCII — `\x<hex>` per non-ASCII UTF-16 code unit — and
a literal UTF-8 byte sequence decodes into a different string. The path then does not exist, and the original
resets the download folder to the platform default, which is exactly what s20's `cwd` shows.

1. Establish it, do not assume it: write the harness's `settings.ini` with a path containing Cyrillic escaped
   the Qt 5 way, re-run s20, and report whether `cwd` becomes the configured directory and whether the original
   stops logging "Resetting download folder to default". If the folder is still reset, report the evidence and
   the new cause — do not paper over it.
2. Make the escaping a single helper next to `canonicalise.py`, used for every value the harness writes into
   `settings.ini`. Scope it to what we can *verify*, not to everything Qt's writer can do:
   - non-ASCII character → `\x<hex>` of its UTF-16 code unit: lowercase, no padding, and the digit defence from
     T9 (a non-BMP character becomes two escapes);
   - backslash → `\\`;
   - pure-ASCII values → byte-identical to today's output, so no other case changes.
   Do **not** invent quoting rules (when Qt wraps a value in `"`, how it handles commas, spaces or empty values)
   from memory or from this text: none of it is exercised by s20. If a case ever needs it, derive it from the
   reference program's own output. If you find that Qt 5 does something here that our helper gets wrong, say so
   in the report.
3. A point worth stating because it looks inconsistent and is not: the argv fixture for s20 already contains
   `<case-dir>/output dir/Загрузки` as literal UTF-8, and that is correct — argv is passed as real bytes on Linux.
   Only the `settings.ini` file is subject to Qt's escaping. Do not "fix" the argv.
4. Re-capture s20. The golden must show `<case-dir>/output dir/Загрузки` as the `cwd`, and the argv must keep
   the `-P` argument it already records.
5. Add one line to `fixtures/README.md` saying the harness writes `settings.ini` in the Qt 5 escape dialect and
   why; `meta.json` for s20 records that the folder was not reset.

Acceptance: s20's `log.txt` `cwd` is `<case-dir>/output dir/Загрузки`; the original's log contains no
"Resetting download folder to default"; every other case is byte-identical to `b344cdc`; the verifier passes.

### T8 — report the dialect, do not "fix" the original

Do **not** change `legacy/` and do not change the userdata fixture. Add to `fixtures/README.md`, under
"Behaviour discovered, not specified":

> **settings.ini is Qt-escaped ASCII, not UTF-8** — the original never calls `QSettings::setIniCodec`, so on Qt 5
> a non-ASCII value is written as `\x<hex>` per code unit and the file stays pure ASCII (Qt documentation, "Format
> enum"). A UTF-8 file placed there by anything else is decoded into a different string. Evidence: s20 before
> T7. Consequence for Morid: it reads and writes this dialect deliberately — D-016 in `docs/decisions.md`.

Also add one line about it to the top of that section. The manager folds it into
[../data-compat.md](../data-compat.md) §3.0; do not edit that file.

### T9 — the escape helper must survive a digit or hex letter after an escape (G7)

Qt 5's writer does not simply emit `\x<hex>` per non-ASCII unit: it sets a flag after any escape and, if the next
character is `0-9a-fA-F`, escapes **that** character too, because otherwise the reader's greedy hex scan would
swallow it. The reader is greedy in exactly this way (`iniUnescapedString`, `StHexEscape`). Result: `Загрузки5`
is written by Qt as `...\x438\x35`; our helper writes `...\x4385`, and Qt reads that as `Загрузк䎅`.

1. Fix `escape_qt_ini_value` to track that flag: after emitting a `\x` escape, if the next character is an ASCII
   digit or one of `a-f`/`A-F`, emit it as `\x<lowercase hex, unpadded>` as well. Keep lowercase and unpadded;
   keep `\\` for backslash.
2. Prove it with a round trip in the verifier, not in the commit message: implement Qt's unescape (greedy hex scan,
   `\x` any number of hex digits; the single-character escapes are exactly `a b f n r t v " ? ' \`, and a
   backslash before anything else **drops** that character) as a test helper, and assert
   `unescape(escape(x)) == x` for at least: `Загрузки`, `Загрузки5`, `Папка1`, `тестa`, `é5`, `Отпуск2024`,
   `фото1`, `a\\b`, and the plain-ASCII value used by the other 39 cases. The test runs as part of
   `verify_fixtures.py` and prints its name, like the `host_paths` check does.
3. Keep the hardcoded assertion on `Загрузки` — it documents the expected bytes — but the round-trip table is what
   makes the helper trustworthy.
4. If any of those round trips cannot be made to pass, stop and report which one and why. Do not adjust the
   expected values to match the implementation.

Acceptance: the round-trip table passes, `Загрузки` still escapes to the same bytes as in `f187923`, no scenario
file changes, `VERIFY_OK` on the clean tree.

### T10 — quote like Qt does, and say how far the helper goes (G8)

Qt 5 wraps a value in `"` when it contains `;`, `,` or `=`, or when it starts or ends with a space
(`iniEscapedString`, `needsQuotes`). Our writer does none of this, so it produces values the original reads back
with quote characters in them.

1. Add quoting to `escape_qt_ini_value` for exactly those conditions. Nothing else — but note what the failure
   mode actually is, because I had it wrong: an unquoted `,` does not leave stray quote characters behind, it
   turns the value into a **QStringList**; `=` and `;` are read back verbatim; a trailing space is chopped by
   `iniChopTrailingSpaces` and a leading one survives:
   no escaping of newlines, tabs or other control characters, because no fixture needs them and each addition is
   a claim we would have to test.
2. **A wrong decision here is worse than an incomplete one.** If implementing the five conditions raises a
   question you cannot settle from the Qt source, report it instead of guessing.
3. State the boundary in `fixtures/README.md` in one sentence: the helper covers non-ASCII escaping, backslash,
   and Qt's quoting conditions; it deliberately does not cover the rest of Qt's writer (control characters,
   `@ByteArray(...)`/`@Variant(...)` value prefixes, QStringList comma splitting). Phrase it as a limit, so nobody
   later reads "the Qt dialect" as "all of Qt".
4. Add to the round-trip table from T9 one quoted value (a path containing a comma) and assert `unescape` strips
   the quotes.

Acceptance: the comma case round-trips, no scenario changes, `VERIFY_OK`, the limit is written down.

### T11 — prove the folder survives a digit, end to end (optional, only if a case is cheap)

Everything above is unit-level. If an existing case can be pointed at a download folder containing a digit after a
Cyrillic letter without inventing a new scenario, do it and let the capture show `cwd` unchanged. If that means a
new case in the manifest, skip it and say so — a unit-level round trip plus s20 is enough for Phase 0, and I would
rather have the smaller diff.

### Round 4 — verdict on `f0f0746` (5 November 2026)

Re-checked by running the verifier on a fresh `git archive`, by reading `iniEscapedString` / `iniUnescapedStringList`
/ `iniChopTrailingSpaces` in `qtbase` v5.15.16, and by **fuzzing the helper against my own transcription of Qt's
reader** — 30 000 random values over ASCII, punctuation, `; , =`, quotes, backslashes, space, tab, control
characters, Cyrillic and an emoji. Zero unexplained failures; the only cases that do not round-trip are control
characters, which the helper writes as `\x9` and Qt reads as U+0009-coded-as-hex — a documented limit, not a
defect.

**Accepted.** `VERIFY_OK`, output identical to the report. `Загрузки` still escapes to the same bytes, all four
quoting conditions match Qt's `needsQuotes`, and every round-trip value in the table is genuinely checked.

**The coder was right three times, and the third one is a hole I did not know about:**

| Coder's point | Verdict |
| --- | --- |
| (a) my `\,`/`\=` "single-character escapes" do not exist; a backslash before an unknown character **drops** it | correct — `escapeCodes` is `a b f n r t v " ? ' \`, and the `else` branch skips the character. My T9.2 listed escapes Qt does not have. |
| (b) the digit flag is set only after `\x…`/`\0`, and the digit branch `continue`s before clearing it, so a **run** of digits escapes digit by digit | correct — and a naive implementation passes the round-trip table too, so this is the one place where matching the source rather than the tests was the only way to be right. |
| (c) my G8 premise was wrong: an unquoted `,` does not produce stray quote characters, it makes the value a **QStringList**; `=` and `;` read back verbatim; a trailing space is chopped by `iniChopTrailingSpaces` | correct, and worse than I described — the `,` case silently changes the *type* of the value, so `DownloadFolder=/a,/b` would have been read as a two-element list. |
| **New: the third hole — `"` was written raw** | correct, and mine to have found. Qt writes `\"`; a raw quote toggles `inQuotedString`, so `Он сказал "да"` read back as `Он сказал да`. Fixed, and the round-trip table now carries it. |

**Still open — one finding (G9):**

| # | Finding | Why it matters |
| --- | --- | --- |
| **G9** | **The round-trip proof is not reachable from CI, and the helper has no test file at all.** It lives inside `verify_fixtures.py`, which runs only when someone runs it; there is no `pytest`, no `#[test]`, no CI workflow in this branch. That was tolerable while the helper was wrong in ways a reviewer could eyeball; it is not tolerable now — the helper encodes five rules of a foreign file format, each of which fails silently, and the only thing standing between a future edit and a broken `settings.ini` is a script nobody runs automatically. The fuzz I ran this round is not in the repository either. | The same reasoning as G5, one level up: the check exists, and nothing forces it to run. Task **T12**. |

**Accepted as reported, not defects:**

- **T2 idempotency is unprovable here, and the coder's substitute is sound.** I confirmed the route by
  `grep`: `escape_qt_ini_value` is called from exactly one place (`capture.py:322`), so the helper cannot move the
  capture unless it moves `settings.ini`, and the coder demonstrated 40/40 byte-identical. `check-idempotency.sh`
  stops at `QT_PREFIX` here too (exit 2), for the same reason: no Qt, and `download.qt.io` is outside the egress
  allowlist. Recorded as a standing environment limitation, not a coder failure.
- **s38's `CommandOnSuccessfulDownload` is hand-written** (`capture.py:343`) and bypasses the helper. I checked it
  against Qt's reader myself: the hand-written line decodes to the intended value, so the fixture is correct
  today. It is still a second, unproved writer — folded into T12 rather than made a separate task.

### T12 — make the round trip something that runs without being remembered (G9)

1. Move the Qt-reader port and the round-trip table out of `verify_fixtures.py` into a test that runs with no
   arguments and no fixtures — and **make sure discovery actually finds it**, which a bare test file does not
   reliably do: `python3 -m unittest` or `pytest` (state which, and why — precedent matters for
   the Rust workspace later). It must fail if `escape_qt_ini_value` regresses, and it must not require anything
   installed beyond the standard library.
2. Put the fuzz into the test, not just the table: a seeded fixed corpus (seed and iteration count in the file, so
   results are reproducible), covering ASCII, `; , =`, `"`, `\`, edge spaces, tab, newline, control characters,
   Cyrillic and a non-BMP character. Control characters and the `@ByteArray(`/`@Variant(`/`@DateTime(` prefixes
   are asserted to be **out of scope**, by a test that says so, so the limit is executable rather than prose.
3. Replace the hand-written line for s38 with the helper, so there is exactly one writer for `settings.ini`. If the helper's output for that value differs from the current hand-written line, say so and
   show both — the fixture must be re-captured only if the value the original reads changes.
4. `verify_fixtures.py` calls the same test module rather than keeping a second copy, so a reviewer sees one
   implementation and two callers.
5. State in `fixtures/README.md` which command runs the test and whether CI runs it. If nothing else in this
   repository runs automatically either, say that plainly — do not describe the test as "in CI" if there is no CI.

Acceptance: `python3 -m unittest discover` (or the chosen equivalent) passes from a clean checkout with only the
standard library; deliberately breaking the digit flag makes it fail; one writer for `settings.ini`.

### Round 5 — verdict on `2919964` (5 November 2026)

Re-checked on a fresh archive. `VERIFY_OK`, `settings_ini_roundtrip values=31 fuzz=500`, output identical to the
report. I then broke the helper four ways myself and confirmed each one fails: digit flag removed (4 failures),
quoting condition narrowed (6), `"` written raw (4), and the tree restored to `OK` afterwards.

**Accepted.** T12 is done and the coder found the defect that would have made it worthless.

| Coder's point | Verdict |
| --- | --- |
| **Bare discovery silently runs zero tests.** Without `__init__.py` markers, `python3 -m unittest discover` prints `Ran 0 tests … OK`, exit 0 — so the test file alone would have reproduced exactly the G5/G9 failure it was written to fix | correct. I reproduced it: same two files, no `__init__.py`, `Ran 0 tests`, exit 0. The two empty package markers are the load-bearing part of this delivery, not a formality. |
| **My round-4 note on trailing tabs was wrong.** `\x9` is read as an ordinary tab, not as a hex-coded control character; the loss comes from `iniChopTrailingSpaces` running from the length the value had before its last escape, and `needsQuotes` not firing for a tab | correct, and I verified it against the source: `chopLimit = stringResult.length()` is set after an escape, `needsQuotes` is only set for `; , =` (`qsettings.cpp:633`). Reproduced: `a\t` → `a\x9` → `a`; `"a\t"` → survives. It is Qt losing the character in its own writer/reader pair, and the helper deliberately matches Qt instead of diverging. Pinned by `test_a_trailing_tab_is_chopped_by_qt_itself`. |
| **T12.1 assumed the file was the work** — it is not, discovery is | correct. |
| **s38's line: bytes change, the decoded value does not** | correct. I decoded both forms with the ported reader myself: identical value, and `post-argv.json` follows from the value, so no re-capture is needed. There is now exactly one writer (`capture.py:329`). |
| **T12.5: there is no CI on this branch** — `.github/` is absent, the ten workflows were removed, new CI is Rust in Phase 1 | correct, and the README says so plainly rather than implying otherwise. |

**Open — one question for the owner, one small gap:**

| # | Item | Why it matters |
| --- | --- | --- |
| **Q-owner** | **Answered 5 November: stay manual until Phase 1 (D-017).** No workflow is added in Phase 0; the obligation moves to Phase 1's exit criteria, which now name both commands and require the test count to be reported. The row below is kept for the record, not as an open item. **Should Phase 0's checks run in CI now, or stay manual until Phase 1?** The coder asks and is right that this is not their call. Today nothing in this repository runs automatically: `verify_fixtures.py` and the new test both exist and both must be remembered. The cost of a one-job workflow now is one file plus a runner definition; the cost of waiting is that the checks can rot silently until Phase 1 CI lands, and Phase 3 will trust them. | This is the G5/G9 failure mode one more time, at the level of the pipeline rather than the code. My recommendation: one job now, `ubuntu-latest`, running `python3 -m unittest` and `verify_fixtures.py`. It does not contradict the Phase 1 decision — Phase 1 is about the *Rust* CI matrix — but it does add a workflow before Phase 1, so it is the owner's call. |
| **G10** | `fixtures/userdata/media-downloader/settings/settings.ini` is a third Qt-dialect file and is validated with `configparser`, not with the ported Qt reader. Harmless today — I confirmed it has **zero** non-ASCII bytes — but it is outside this round's proof, and `configparser` would accept a value Qt reads differently. | Small, real, and named by the coder rather than hidden. Fold into T13, not urgent. |

**Recorded, not claimed as fixed:** `escape_qt_ini_value` raises on a lone surrogate. No `QString` from a real
settings file can contain one, and the coder says so rather than asserting safety.

### T13 — close the last two loose ends (small, do with T12's follow-up)

1. Validate `fixtures/userdata/media-downloader/settings/settings.ini` with the ported Qt reader when it is
   non-ASCII, and keep `configparser` only for the ASCII case or drop it. If the reader finds a value that is not
   valid Qt dialect, report it; do not repair it silently (G10).
2. Decide and record the lone-surrogate behaviour: either reject it with a clear message, or document it as
   impossible-by-construction in the test. One line either way; the point is that the behaviour is chosen rather
   than accidental.

Acceptance: both covered by the existing test file, `python3 -m unittest` still green, no fixture changes.

### Round 6 — verdict on `f43e61c` (5 November 2026)

Re-checked on a fresh archive. `VERIFY_OK`, `Ran 21 tests … OK`, and without `fixtures/` `OK (skipped=1)` with the
skip named. I reproduced the negative control myself — wrote `DownloadFolder=/tmp/Загрузки` as UTF-8 into the
userdata file and watched both the test and the verifier fail with the mojibake in the message.

**Accepted.** T13 is done, and the coder is right that my condition was wrong.

| Coder's point | Verdict |
| --- | --- |
| T13.1's trigger "when it is non-ASCII" never fires on a file with zero non-ASCII bytes, so the defect G10 is about stays invisible | correct, and sharper than I had it: the right test is **decode with the port, re-encode with the helper, compare bytes**, not "inspect the non-ASCII ones". Verified — the injected UTF-8 file is caught with no special branch. |
| "keep `configparser` for the ASCII case" is not neutral — it was the source of `settings_keys=94`, so keeping two parsers would keep two definitions of a key | correct. I confirmed the count is unchanged at 94 with the Qt reader, and that the two parsers genuinely disagree: on the injected file `configparser` reports a clean `/tmp/Загрузки` (94 keys, no complaint) while the Qt reader reports the mojibake and the mismatch. The old check could not have seen it. |
| T13.2's "document as impossible-by-construction" describes an accident; reject explicitly instead | correct. `ValueError: … lone surrogate (U+d800) …` now, and a valid astral pair still writes as `\xd83e\xddea` — the companion test that stops "reject surrogates" from decaying into "reject non-BMP". |
| §8's `### Reporting` block still asks for four outputs while the round asked for two | correct, and my error: the block is generic. Fixed below. |

**Open — one finding (G11), not caused by this round but exposed by it:**

| # | Finding | Why it matters |
| --- | --- | --- |
| **G11** | **The Qt INI reader port understands values but not keys.** `iniUnescapedKey` escapes a key's non-alphanumerics as `%XX` (Latin-1 range) or `%UXXXX` (above), and writes `/` as `\\`. The port reads `%U417%U430=value` as the literal key `%U417%U430`, and `My\\Group\\Key` keeps its backslashes. Nothing in `fixtures/` has a non-ASCII or nested key — I checked — so no fixture is wrong today. | This is a **Morid** requirement, not only a fixture one: real `settings.ini` files from Windows users will contain `%UXXXX` keys (any extension or engine whose name is non-ASCII lands in a key), and [data-compat](../data-compat.md) §3.1 tells the implementation to read them. Right now the port would pass its own tests and be wrong on exactly those files. Task **T14**, and it belongs to the same helper — one place to be right. |

**Also recorded:** the reporting block below is corrected to ask for what is actually needed, rather than the four
fixed outputs inherited from round 2.

### T14 — teach the reader keys, not only values (G11)

1. Port `iniUnescapedKey`: `%XX` → the Latin-1 code point of that byte, `%UXXXX` → that UTF-16 code unit (case
   insensitive, as Qt is), `\\` → `/`, and for keys Qt also lowercases a key that consists only of lowercase
   characters in its escaped form — read `iniUnescapedKey`'s `lowercaseOnly` handling and match it, or say why the
   fixture set does not need it.
2. Tests, in the existing file: a `%UXXXX` key, a `%XX` key, a nested key with `\\`, a key with a literal `%` that
   is not an escape, and one full `settings.ini` containing all of them re-encoded with the writer and compared
   byte for byte. If the writer does not have a key-escaping helper, add it — the round trip is the point.
3. Confirm nothing in `fixtures/` changes; if something does, stop and report it rather than re-capturing.

Acceptance: `python3 -m unittest` green with the new tests, `VERIFY_OK`, no fixture changes, and the reader refuses
a malformed `%` escape rather than passing it through.

### Reporting

Report only: the files changed, the commands you ran with their output, which acceptance criteria you could not
check in this environment and why, and anything in these instructions that turned out to be wrong. No summary of
the previous delivery, and no fixed list of outputs — report what this round's acceptance actually needs.
