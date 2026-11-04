# Task 1 for the coder — Phase 0: reference behaviour and fixtures

**Status:** not started · **Issued:** 4 November 2026 · **Estimated:** 5–8 person-days
**Requires:** nothing to exist first. **Must finish before** any Rust code is written.

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
5. The code: `legacy/src/utility.h` (`context`, ~1873–2135), `legacy/src/engines.cpp` (command assembly,
   proxy injection), `legacy/src/engines/*.cpp` (`updateDownLoadCmdOptions` in each), `legacy/src/settings.cpp`.

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
| 01 | plain yt-dlp download, default options | F-01 | engine defaults, output template, `--newline`-class flags |
| 02 | download with a preset (highest quality) | F-03 | preset option string placement |
| 03 | download with user-supplied `-f 22` | F-02 | user options versus engine defaults order |
| 04 | download with two conflicting user options | F-02 | order preserved |
| 05 | format listing | F-01 | list command, `DumptJsonArguments` if any |
| 06 | playlist URL, whole playlist | F-20 | playlist vs single-video flags |
| 07 | playlist URL, item range | F-21 | range syntax as the engine expects it |
| 08 | playlist URL with `--no-playlist` forced by the user | F-21 | last-wins order |
| 09 | download with download-archive | F-24 | `--download-archive` path form |
| 10 | batch download of two URLs | F-10 | batch argument mechanism (`BatchFileArgument`) |
| 11 | cookies from a browser | F-57 | cookie argument per engine |
| 12 | cookies from a file | F-57 | path form, quoting |
| 13 | proxy: mode `None` | F-59 | nothing added |
| 14 | proxy: mode `Manual`, `http://user:pass@host:8080` | F-59 | how credentials are passed, argument vs environment |
| 15 | proxy: mode `Env` | F-59 | which variables are set |
| 16 | subtitle download enabled | F-06 | `DefaultSubtitleDownloadOptions` |
| 17 | subtitle languages specified | F-06 | token splitting |
| 18 | comments download | F-07 | engine-specific flag |
| 19 | chapters split | F-16 | naming template with chapter fields |
| 20 | output directory with a space and a non-ASCII name | F-50 | path passing, no shell |
| 21 | filename template containing `%(title)s` from user text | F-50 | template is passed through, not expanded by us |
| 22 | URL with `&`, `?`, `#`, `%` | F-50 | no URL decoding, no splitting |
| 23 | URL that is not HTTP (a bare filesystem path) | F-69 | rejection or pass-through, whichever the original does |
| 24 | custom extension, template text with `${...}` substitution | F-30 | substitution result |
| 25 | extension with `Cmd` for the current OS only | F-30 | correct branch selected |
| 26 | extension with `Cmd` for a different arch | F-30 | fallback behaviour, whatever it is |
| 27 | extension with `ExtraOptions` | F-30 | where they land in the order |
| 28 | extension whose `AutoUpdate` is false | F-33 | update command not produced |
| 29 | gallery-dl, default options | F-28 | its own defaults |
| 30 | gallery-dl with a per-engine option | F-28 | order |
| 31 | wget | F-28 | `ReplaceOutputWithProgressReport` effect on argv |
| 32 | aria2c | F-28 | CR-based output engine's argv |
| 33 | you-get | F-28 | interpreter invocation |
| 34 | svtplay-dl | F-28 | its own flag set |
| 35 | unknown extension name | F-30 | error path, argv is empty and that is the point |
| 36 | extension with a missing required key | F-30 | load failure, argv empty |
| 37 | yt-dlp with `-x` and audio format | F-08 | conversion flags, ffmpeg reference |
| 38 | `--run-command-after-download` configured | F-70 | the second command's argv |
| 39 | non-UTF-8 encoding setting | F-56 | `EncodingArgument` |
| 40 | Flatpak/AppImage packaging flag | F-30 | `ExtraOptionsFlatpakAppImage` inclusion |

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

- **Do not modify `legacy/`.** Its behaviour is the reference. If it is wrong, record the defect in
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
