# Task 6 for the coder — Phase 5: network, backend installation, updates

**Status:** ready to issue · **Issued:** 4 November 2026 · **Estimated:** 10–14 person-days
**Requires:** Tasks 1–5 accepted and merged (`main` at `ba3e569` or later).
**Produces:** a backend that is downloaded, verified, unpacked and installed — and refused when any of those three
checks disagrees with what the source published. Plus version reading, so `morid engines` reports what is actually
installed.

---

## 1. Goal

Every phase so far ran a backend somebody else had already installed. This phase installs it, and it is the first
phase that talks to the network, so **the network is the risk, not the feature**.

At the end of this task:

- `morid-net` fetches a release description, downloads an asset, verifies **size and SHA-256**, unpacks it with a
  refusal of path traversal, and installs it atomically — a failed install leaves the previous binary exactly as it
  was.
- `morid engines` gains `--install`, `--update` and `--remove`, and reports a real version for every backend that is
  present (F-32, F-34, F-35).
- Supporting engines (ffmpeg, deno, quickjs-ng, python, bsdtar) are recognised, version-checked and installed by the
  same mechanism (F-38).
- **No request is made to any host that is not a backend's own release page** (D-014), and **no extension
  description is downloaded at all** (D-010).

There is no window in this phase. The Configure → Extensions screen stays in Phase 7; what it will call is built
here.

---

## 2. Context you need

1. [../extensions.md](../extensions.md) §6 — where versions come from, and why integrity is size + SHA-256 from the
   release metadata. §2 — `SupportingEngine` / `UpdatableSupportingEngine`, which is how a helper is told apart from
   a downloader. §7 — the bundled set.
2. [../decisions.md](../decisions.md) — **D-014** (their endpoints, our endpoint, no extension endpoint), **D-010**
   (nothing is downloaded for a description), **D-013** (Morid does not update itself; it only notices), **D-005**
   (no Windows 7 path).
3. [../dependencies.md](../dependencies.md) §2.5–2.7 — the three decisions this phase has to close: `ureq` versus
   `reqwest`, the TLS trust store, and whether unpacking is `tar`/`flate2`/`zip` or a shelled-out `bsdtar`.
4. [../testing.md](../testing.md) §3 — **"no test may touch the network"**, and how the HTTP layer is tested instead.
   §4.1 — the fixture class "HTTP responses: recorded GitHub API bodies".
5. [../cli.md](../cli.md) §2.3 — the three verbs and their exit codes (`0`, `3`, `4`).
6. [../architecture.md](../architecture.md) §7 — `<appdata>/morid/bin`, and §10 A2.

---

## 3. What to produce

### 3.1 `morid-net`, behind a trait

The crate gets one trait that describes the three requests it needs — **release metadata**, **asset download**,
and nothing else — and one implementation that uses the HTTP client. Every test uses a double implementing the
trait. This is not stylistic: [testing](../testing.md) §3 forbids a socket in `cargo test`, and the only way to keep
that rule while proving integrity handling is to make the transport swappable.

The trait returns bytes and status; it does not parse JSON and it does not know what an engine is.

### 3.2 Release metadata → an asset for *this* platform

From the recorded metadata, choose the asset for the current platform and architecture. The rule is where the
original's behaviour is easiest to get wrong silently, so state it in one function with a table of inputs:

- the platform/architecture names used by each project differ, and so does the **format** (`.tar.gz`, `.zip`, a bare
  binary, an `.exe`);
- one project in the bundled set publishes on **Codeberg**, not GitHub (`gallery-dl`), so a parser that assumes one
  JSON shape breaks on the second backend it meets;
- `DownloadUrlMAC` exists because macOS assets differ; `DownloadUrlWin7` is **not used** (D-005).

Where the original's choice differs from the "obvious" one (picking the first asset, matching on a substring,
preferring the newest by date rather than by version), the original wins unless the difference is a defect you can
name. Say which you found.

### 3.3 Integrity: size and SHA-256, verified before anything is moved

1. Download to a temporary file inside the **same directory** as the destination (so the rename is atomic within one
   filesystem).
2. Verify size, then SHA-256, against the metadata — in that order, so a truncated download fails cheaply.
3. Only then rename into place. Unpack first if the asset is an archive, into the same temporary directory.
4. On any failure: delete the temporary file, leave the existing binary untouched, exit `3`, and say which check
   failed and what was expected.

The acceptance pattern is **[testing](../testing.md) I9**: a corrupted body must leave the **old binary intact and
still executable**. "The command failed" is not that assertion — the test has to show the old file byte for byte,
and then run it.

### 3.4 Unpacking, with traversal refused

An archive entry whose path escapes the target directory is **refused, not sanitised**: `..`, an absolute path, a
symlink pointing outside, and a Windows path with a drive or a backslash all abort the install with an error naming
the entry. [SECURITY.md](../../SECURITY.md) lists this as a named risk, and it is the one place where "it worked on
my machine" is not evidence.

Test: one archive per attack, each asserting the refusal **and** that nothing was written outside the target.

### 3.5 Version reading (F-32)

Every extension declares `VersionArgument`, `VersionStringLine` and `VersionStringPosition`. Read the version by
running the backend with that argument and taking that token — the fields are already parsed by `morid-ext`; this
phase runs them.

**The fixtures already contain the answer for seven backends and they contain two traps.** Both are visible in
`fixtures/streams/*/version.txt` without reading any C++:

| Backend | Declared (line, position) | Fixture gives |
| --- | --- | --- |
| yt-dlp | 0, 0 | `2026.10.26` |
| gallery-dl | 0, 0 | `1.32.19` |
| aria2c | 0, 2 | `1.37.0` |
| wget | 0, 2 | `1.21.3` |
| svtplay-dl | 0, 1 | `4.211` |
| you-get | 0, 2 | `0.4.1743,` — **with a trailing comma** |
| get-sauce | 0, 2 | nothing: line 0 is empty, `-v` prints a **leading blank line** |

So: `you-get`'s version string carries punctuation, and `get-sauce`'s declared line is off by one relative to its
own output. Decide what Morid reports in each case — the token as printed, or a normalised version — **and say which
and why**, because a version with a comma in it sorts and compares differently. Do not silently strip characters.

And say what happens for the six extensions with **no captured version output**: `deno`, `lux`, `quickjs`,
`quickjs-ng`, `safaribooks`, `ytdl-patched`. Reporting "unknown" for a backend that printed something the reader
could not parse is a defect; reporting "not captured" is honest. Pick one and make the CLI print it identically
every time.

### 3.6 Install, update, remove (F-34, F-35)

- `--install <name>`: fetch, verify, unpack, place under `<data>/bin`, and make the file executable on Unix.
- `--update <name>`: the same path, with the integrity rule of §3.3 and the old binary kept on failure.
- `--remove <name>`: **refuse to remove a bundled extension's backend** where the original refuses, and say why.
  Removing a file is not the same as removing a description (D-010): the JSON stays.
- Every one of them is a no-op that reports success when the thing is already in the requested state — running
  `--install` twice must not fail the second time.
- Exit codes: `0` success, `1` usage, `3` download/verification/extraction failure, `4` unknown engine.

### 3.7 The endpoint rule, enforced rather than promised

D-014 has three clauses and only one of them is about URLs. Make the rule mechanical: one allow-list built from the
`DownloadUrl`/`DownloadUrlMAC` values of the bundled extensions, and a request to anything outside it is an
**internal error, not a network error** — exit `9` with a message naming the host. A test proves that a fixture
extension with `DownloadUrl: "https://example.invalid/x"` is refused before any socket is opened, using the trait
double's call count as the assertion.

For Morid's own "a newer version exists" notice (D-013): one endpoint, Morid's, and it **only reports** — it never
downloads, and there is no code path that could install it.

### 3.8 Supporting engines (F-38)

ffmpeg, deno, quickjs-ng, python and bsdtar are identified by `SupportingEngine`, are not offered in the downloader
list, and follow their own per-platform rules. The audit fixed their shape at five names
([../audit-original.md](../audit-original.md) §"Engines"); if the code shows a different set, that is a finding.

- They are installed by the same verified path, and reported in their own section of `morid engines`, never mixed
  into the downloader list.
- A supported helper that is already provided by the system is **not** downloaded when the setting says to use the
  system one (`UseSystemSupportingEngine`, imported in Phase 4) — and when it is missing, the message names the
  helper and what needs it, not "engine not found".

---

## 4. Constraints

- **No new dependency without a written decision in `docs/dependencies.md`.** This phase has three open ones (§3
  above); closing them is part of the work, with the measured size delta, not an afterthought.
- **`unsafe` stays out**: `morid-net` and `morid-store` keep `#![forbid(unsafe_code)]`. Unpacking and hashing are
  safe code or an accepted crate.
- **`cargo test` passes with no network and no `PATH` access to a real backend.** A test that needs the internet is
  a test that will be deleted ([testing](../testing.md) §3).
- **The download folder, the data directory and the user's existing binaries are the only places touched.** Nothing
  is written outside `<data>/bin`, `<data>/tmp` and the temporary directory next to the destination.
- If a claim in this task contradicts the code or a fixture, **report it and stop on that point** — that has been
  the cheapest thing that happened in every phase so far.

---

## 5. Acceptance criteria

1. CI green on Linux, macOS and Windows: build, tests, fmt, clippy `-D warnings`, and the existing Phase 0–4 checks.
2. **I9 proved**: a corrupted download leaves the old binary byte-identical **and still runnable** — paste the
   `cmp`/`sha256sum` output and the successful run of the old binary, not a sentence about it.
3. Four traversal attacks refused, each with the entry named; nothing written outside the target.
4. Version output for the seven captured backends pasted next to the fixture, with your decision about `you-get`'s
   trailing comma and `get-sauce`'s leading blank line stated in one sentence each.
5. `install`, `update`, `remove` are idempotent; `--remove` on a bundled engine is refused with the reason.
6. The endpoint allow-list test: a foreign `DownloadUrl` is refused **before** the transport is called, proven by the
   double's call count.
7. The three dependency decisions are written into `docs/dependencies.md` with measured size deltas from CI.
8. Exit codes `3`, `4` and `9` each produced by a test, and `docs/cli.md` §2.3 matches the binary.
9. The existing `--dry-run` goldens still pass unchanged; a network failure in `--install` does not change argv.
10. The size row for `docs/dependencies.md` §6 is added with the CI run, not guessed.

---

## 6. Testing you must report

Paste output, not summaries: the corrupted-download run with hashes and the surviving binary executed; the four
traversal refusals; `morid engines` for the seven captured backends and for the six without a capture; the
allow-list refusal; `install` twice and `remove` twice; the dependency size deltas; and anything in §3 that
surprised you.

Order: (1) files added or changed; (2) what is implemented and what is deliberately not; (3) the outputs above;
(4) known problems, including anything guessed; (5) every place where this task or a document contradicts what the
code does, with the evidence.

---

## 7. What the review will try to break

The review reads **outputs, logs and artefacts, not source code** — that is a standing instruction from the owner.
So every claim below has to be demonstrable in a log or an artefact, and a claim that needs the reviewer to read
Rust to confirm is a claim that will not be confirmed.

- A hash check that passes because the expected hash was taken from the file just written, not from the metadata.
  Show the expected value's origin in the log.
- Verification that happens **after** the file is in place. Show the order in the log or in the failure state.
- Traversal refused for `..` only. The other three attacks are in the criterion.
- Version reading that "works" because the test feeds a fixture straight to a parser. The version must come from
  running the backend, with a fake one standing in.
- An allow-list that is a string comparison on the host only, so `http://` versus `https://`, a port, or a redirect
  slips past. Say what you compare and what a redirect does.
- `--remove` deleting the extension JSON rather than the binary.

Each is a criterion, not an opinion.

---

## 8. Documents that change as a result

Updated **by you**, in the same pull request:

- `docs/dependencies.md` — the three Phase 5 decisions (§2.5–2.7), and the size row in §6.
- `docs/roadmap.md` — the Phase 5 status paragraph and the estimate corrected against the actual.
- `docs/cli.md` — only if a flag, an output field or an exit code differs from §2.3.

Updated by the manager after review: `docs/features.md` (F-32, F-34, F-35, F-38), `docs/testing.md` §2,
`docs/extensions.md` §6 if the asset-selection rule turns out to be under-described.

---

## 9. What comes next, so you can design accordingly

- **Phase 6** adds the remaining backends and wires the engine choice: a URL rule selecting the engine, per-engine
  default options prepended, the `LikeYoutubeDl` family sharing an archive and a parser. Those two Phase 4
  leftovers (`url_rule`, `engine_default`) belong there, and they consume what this phase installs.
- **Phase 7** draws Configure → Extensions: install, update, remove, per-engine defaults. Every button there is one
  of the verbs you build here, so a verb whose failure cannot be described in a sentence becomes a dialog nobody
  can write.
