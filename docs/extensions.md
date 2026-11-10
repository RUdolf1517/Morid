# The extension contract

Related: [features](features.md) · [architecture](architecture.md) · [data-compat](data-compat.md) ·
[decisions](decisions.md) · [audit](audit-original.md) §2.5

An extension is a JSON file that tells Morid how to talk to a command-line downloader: where its executable
lives, how it is invoked, how its version is read, how to tell progress from noise, and what to do with its
output.

**This is a public contract.** Third parties write these files, and the 17 files in [`extensions/`](../extensions/)
must keep working ([D-003](decisions.md)). This document is therefore normative: it says what each key means,
what happens when it is missing, and what Morid does differently from the original — because the original does
not document several of these keys at all, and in two cases does not behave the way its own shipped files
suggest.

Sources for what follows: the 17 files in `extensions/` and the C++ reader
(`legacy/src/engines.cpp:1000-1120`, `1494-1580`), cross-checked key by key.

---

## 1. Schema versioning

| Field | Meaning |
| --- | --- |
| `Version` | Version of the *schema this file is written against*, as a string integer: `"1"`…`"4"` |
| `RequiredMinimumVersionOfMediaDownloader` | Minimum version of the *application* the file needs |

Rules:

- A file **without** `Version` is treated as version `1`. The original does the same implicitly.
- A file with a `Version` **higher** than the reader supports is refused with a message naming both versions.
  The original silently ignores unknown versions; silently accepting a file written for a newer schema is how
  an engine ends up invoked with half its options.
- Unknown keys are **preserved and reported as a warning**, never fatal. An extension written for a newer
  schema may add keys that an older Morid does not need.
- `RequiredMinimumVersionOfMediaDownloader` of the `2.x` family is treated as **satisfied**
  ([D-004](decisions.md)); a higher requirement refuses the extension with a message that names the required
  version and the running version. This is a deliberate difference from the original, which compares against
  its own compiled version and would reject every existing extension once Morid renumbers.

**Thin files — and the mechanism is not inheritance.** Three shipped files carry only `Name` and `Version`:

```json
{ "Name": "yt-dlp-nightly", "Version": "4" }
```

The reader must **not** infer that a thin file inherits "its family" — that is what this document said until it was
checked against `legacy/src/engines.cpp:244-258`, and it is wrong. The original dispatches on the **exact file name**,
loads `yt-dlp.json` as the base, and then applies one hard-coded transformation from
`legacy/src/engines/yt-dlp.cpp`:

| File | Transformation of the `yt-dlp.json` base |
| --- | --- |
| `yt-dlp-nightly.json` | `Name`; `DownloadUrl` → the nightly-builds GitHub endpoint; `Cmd` replaced on all three platforms with nightly binary names (`yt-dlp-nightly`, `yt-dlp_x86-nightly.exe`, `yt-dlp-nightly_macos`, plus the Win7 names); the thin file's own `Version`, `AutoUpdate` and `BackendPath` override the base when present |
| `yt-dlp-ffmpeg.json` | `Name`; `--downloader ffmpeg` appended to `DefaultDownLoadCmdOptions` |
| `yt-dlp-aria2c.json` | `Name`; `--downloader aria2c` appended to `DefaultDownLoadCmdOptions`; the Generic command becomes `stdbuf -o L yt-dlp` |

Consequences a reader must respect:

- **A table entry keyed by `Name` is not equivalent to this.** The dispatch is by file name, and the three
  transformations differ in *what* they change — one replaces the whole `Cmd`, two append two arguments.
- **A missing `Cmd` in a thin file is not an error**, but "resolve the family" is not a rule either: only these three
  names have a base, and anything else without `Cmd` is invalid.
- **Treating the transformations as configuration is the honest implementation.** They cannot be expressed in the
  shipped JSON, which is exactly why the original hard-codes them. Morid does the same, keyed by file name, and the
  table above is the specification of the difference.
- **`yt-dlp-aria2c` barely changes `Cmd` at all, and that is not obvious from a summary.** It calls
  `json.add(...)` for Generic, Windows and MacOS but — unlike `cmdNightly` at `yt-dlp.cpp:374` — never calls
  `json.done()`, and `addJsonCmd` publishes nothing without `done()` (`utility.cpp:1218-1240`). The nightly's
  arguments are `Args: [<same executable>]` (`utility.h:1145-1150`); the aria2c overrides carry
  `Args: ["stdbuf","-o","L","yt-dlp"]`. Net effect: the base `Cmd` survives untouched, including the plain
  `yt-dlp` name on Windows and macOS. An implementation that applies the three overrides — the obvious reading of
  the `json.add` calls, and the reading this document carried on its first pass — produces a different command line
  than the original on every platform. The observable difference is the **absence** of an argument, which no test
  that only checks for presence will catch.

There is one more name-based special case, in the engine list rather than in a transformation: `youtube-dl.json` is
removed from the list the original enumerates (`legacy/src/engines.cpp:820`). No shipped file of that name exists, so
it changes nothing today; it is recorded so that a future `youtube-dl.json` is not silently offered as an engine.

**How many keys there are.** The documentation elsewhere says "34 keys"; the 17 shipped files use **40** distinct
keys. The 34 is a count from an early draft and was never right. Do not plan against a number — the reader must
accept any key it knows, warn about the ones it does not (§8 rule 7), and the test iterates the files, not a list.

Schema history, as observed in the shipped files:

| Version | Files | What changed |
| --- | --- | --- |
| 1 | `getsauce.json`, `lux.json` | first structured form |
| 3 | `yt-dlp-aria2c.json`, `yt-dlp-ffmpeg.json` and the rest of the yt-dlp family | engine variants as thin files that `Name`-reference the family |
| 4 | `yt-dlp.json`, `yt-dlp-nightly.json`, `yt-dlp-aria2c-native.json` | `DownloadUrlWin7`, `SkipLineWithText` usage stabilised |

The differences between 1 and 3/4 are not documented anywhere in the original; the table above is what the
files show. Nothing in the reader branches on `Version`, which is itself a defect: it means the field is
decorative. Morid will branch on it where it matters and say so here.

---

## 2. Identity and packaging

| Key | Type | Required | Meaning |
| --- | --- | --- | --- |
| `Name` | string | **yes** | Engine name, unique. Shown in the UI, used in settings keys (`CookieBrowser_<Name>`), archive file names and the URL manager. |
| `Version` | string int | no | Schema version, see §1 |
| `RequiredMinimumVersionOfMediaDownloader` | string | no | Minimum application version, see §1 |
| `LikeYoutubeDl` | bool | no | This engine behaves like yt-dlp: it shares yt-dlp's archive file and its parser. The original's family mechanism. |
| `CanDownloadPlaylist` | bool | no | The Playlist Downloader tab accepts this engine |
| `AutoUpdate` | bool | no | The application may download a new binary for this engine from `DownloadUrl` |
| `UsePrivateExecutable` | bool | no | Prefer a private copy in the application data directory over a system executable |
| `SupportingEngine` / `UpdatableSupportingEngine` | bool | no | This is a helper (ffmpeg, deno, python, bsdtar), not a downloader the user picks |
| `ArchiveContainsFolder` | bool | no | The downloaded archive contains a directory; extraction must be followed by a rename |
| `BackendPath` | string | no | Where the executable lives. `${default}` / `${BackendPath}` mean "the application's bin directory". |
| `UserName` | string | no | Credential for a backend that takes one on the command line. **Never logged** (I7); see §4.2. |
| `Password` | string | no | As `UserName`. Stored in the extension file, which is why §4.2 exists. |

Notes that are contractual because user data depends on them:

- `Name` is part of three file names on disk (`archiveFile-<Name>.txt`, cookie settings keys, per-engine
  defaults). Renaming an engine in a future version is a data migration, not a cosmetic change.
- `LikeYoutubeDl: true` inherits the **archive file** of yt-dlp, deliberately: a user who already downloaded a
  video with yt-dlp must not get it again because they switched to the aria2c variant.

---

## 3. Command selection

```jsonc
"Cmd": {
  "Generic": { "amd64": { "Name": "yt-dlp", "Args": ["yt-dlp"] },
               "x86":   { "Name": "yt-dlp", "Args": ["yt-dlp"] } },
  "MacOS":   { "amd64": { "Name": "yt-dlp_macos", "Args": ["yt-dlp_macos"] } },
  "Windows": { "amd64": { "Name": "yt-dlp.exe", "Args": ["yt-dlp.exe"] } }
}
```

`Cmd` is the structure above: **platform → architecture → `{Name, Args}`**. Lookup order: the platform-specific
block (`Windows` / `MacOS`), then `Generic`. Within a block the
architecture key, then a fallback. Architecture keys observed in the shipped files:
`amd64`, `x86`, `aarch64`, `win7amd64`, `win7x86`.

**Change from the original.** Windows 7/8 architecture keys (`win7*`) are **not used**
([D-005](decisions.md)). A file that only defines `win7*` keys for Windows is reported as unsupported on the
supported Windows platforms rather than silently falling back to an x86 build that may not run.

`Args[0]` is the executable name; the rest are its fixed arguments (`Args` and `Name` inside `Cmd` are the only
keys at that level; both are required). A name is resolved as: an absolute path,
then the application's bin directory, then `PATH`. Note the `stdbuf -o L yt-dlp` form used by
`yt-dlp-aria2c-native.json`: the executable name and the program name differ, and the version reader must use
`Name`, not `Args[0]`.

Placeholders available in `Args`:

| Placeholder | Replaced with |
| --- | --- |
| `${default}` | the application's private binary directory (the original's spelling is part of the contract) |
| `${BackendPath}` | the same directory |
| `${CommandName}` | `Name` without a path, used by Python-based extensions (`safaribooks.json`) |

---

## 4. Arguments and options

| Key | Type | Meaning |
| --- | --- | --- |
| `DefaultDownLoadCmdOptions` | string[] | Arguments always added when downloading |
| `DefaultListCmdOptions` | string[] | Arguments added when fetching a format list |
| `DumptJsonArguments` | string[] | Arguments for a metadata-only dump (yt-dlp family) |
| `DefaultCommentsCmdOptions` | string[] | Arguments for retrieving comments |
| `DefaultSubstitlesCmdOptions` | string[] | Arguments for retrieving the subtitle list. **The misspelling is part of the contract** and appears in five files |
| `DefaultSubtitleDownloadOptions` | string[] | Arguments for downloading/embedding subtitles |
| `OptionsArgument` | string | The flag that introduces a format, e.g. `-f` for yt-dlp, `-s` for get-sauce. It makes a bare `mp4` in the options field mean `-f mp4` |
| `PlaylistItemsArgument` | string | The flag that introduces an item range, e.g. `--playlist-items`. **Read by the original but never used**: it is stored in `engines.cpp:1033` and the only accessor (`engines.h:1596`) has no callers. The range reaches argv through the playlist tab's own options instead (§5.4). Keep it as data, do not build argv from it. |
| `PlayListIdArguments` | string[] | **Distinct from `PlaylistItemsArgument`**: fixed arguments that make a backend return playlist ids only. Shipped only by `aria2c.json`, and empty (`[]`). Note the honest limit: the original writes the key (`aria2c.cpp:76`, `wget.cpp:164`) but **never reads it** — `grep -rn PlayListIdArguments legacy/src` finds no reader. Morid keeps it as passive data and does not act on it; a non-empty value would be an unimplemented behaviour, so it is preserved and reported rather than interpreted. |
| `BatchFileArgument` | string | The flag that reads URLs from a file, e.g. `-a` |
| `VersionArgument` | string | The flag that prints the backend version |
| `CookieArgument` | string | Flag to read cookies from a browser, e.g. `--cookies-from-browser` |
| `CookieArgumentTextFile` | string | Flag to read cookies from a file, e.g. `--cookies` |
| `EncodingArgument` | string | Flag to set output encoding; empty means the backend has no such flag |
| `DownloadOptionsAsLast` | bool | Where user options go in the final argv: last (true) or before engine defaults |
| `RemoveText` | string[] | Substrings stripped from output before parsing (e.g. ANSI escapes in `safaribooks.json`) |
| `SkipLineWithText` | string[] | Lines containing any of these are dropped entirely |
| `SplitLinesBy` | string[] | Line terminators for this backend: `["\n"]` or `["\n","\r"]` |
| `ReplaceOutputWithProgressReport` | bool | Do not parse output for progress; show elapsed time instead (get-sauce, lux) |
| `ShowListTableBoundary` | object | **Not a bool.** Both the shipped file and the code that writes it use an empty object: `aria2c.json` has `{}`, and `aria2c.cpp:34` and `wget.cpp:131` insert `QJsonObject()`. Nothing in the original reads the key. Treat it as an unknown-shape placeholder: accept any value, act on none, and do not describe it as a boolean. |
| `ExtraOptions` / `ExtraOptionsFlatpakAppImage` / `ExtraOptionsWin7` | string[] | Arguments added only on that packaging/platform |
| `ExtraArguments` / `ExtraArgumentsFlatpak` / `ExtraArgumentsWin7` | string[] | **Not recognised** — a mis-spelling in the shipped set, see §4.1. Warning, never silently accepted. |

### 4.2 Credentials inside extension files

Three shipped files carry `UserName` / `Password` as plain strings. That is the original's design and it is a
security problem we inherit rather than introduce:

- the values must never reach a log line, the history file or the console stream (invariant I7), and a test exists
  for it in Phase 3, not later;
- the reader treats them as opaque strings — no trimming, no shell escaping beyond what argv needs;
- they are **not** migrated out of the extension file into a keyring in this project: that would break the contract
  that an extension file written for the original keeps working. Recorded as O9 in
  [migration](migration.md) §6.1.

### 4.1 A defect in the shipped set that Morid must decide about

Three files, including `ytdl-patched.json`, use **different key names**:

```jsonc
"ExtraArguments": [], "ExtraArgumentsFlatpak": [...], "ExtraArgumentsWin7": [...]
```

The reader only looks for `ExtraOptions`, `ExtraOptionsFlatpakAppImage` and `ExtraOptionsWin7`
(`legacy/src/engines.cpp:1052-1058`). The consequence is that in the original, `ytdl-patched` runs **without**
its intended `--no-js-runtimes --js-runtimes quickjs` on Flatpak and Windows 7.

Morid's behaviour, decided here because leaving it undocumented is worse than either choice:

1. `ExtraOptions*` is the only recognised spelling. `ExtraArguments*` is **not** silently accepted.
2. A file containing `ExtraArguments*` produces a **warning** naming the ignored keys and the file, so the
   discrepancy is visible instead of silent.
3. `ytdl-patched.json` is corrected in our copy to use the recognised keys, and the change is recorded in
   `CHANGELOG.md` together with the note that this changes behaviour for that extension.

This is a behaviour change relative to the original. It is deliberate and listed here rather than being
smuggled in as a "fix".

---

## 5. Output interpretation

### 5.1 Progress rules

`ControlJsonStructure` describes how to recognise a progress line without regular expressions:

```jsonc
"ControlJsonStructure": {
  "Connector": "&&",
  "lhs": { "startsWith": "[download]" },
  "rhs": { "contains": "ETA" }
}
```

Predicates available on each side: `startsWith`, `endsWith`, `contains`, `containsAny` (array of strings) and
`containsAll` (array of strings). **One predicate per side is used**, and if a side carries more than one they are
tested in exactly that order — `startsWith`, `endsWith`, `contains`, `containsAny`, `containsAll` — and the first
present one decides. They are not combined. `containsAny` is true when any element is in the line; `containsAll`
when every element is.

`Connector` selects between two forms (`legacy/src/engines.cpp:1551-1582`):

| Form | Shape | Result |
| --- | --- | --- |
| no connector | `Connector` absent or empty, `lhs` present | the line matches when `lhs` holds; `rhs` is ignored |
| `&&` | `Connector`, `lhs` and `rhs` all present | both must hold |
| `||` | same | either may hold |

Both connectors are part of the contract. `gallery-dl.json` ships `"Connector": "||"`; `svtplay-dl.json` and
`wget.json` ship the no-connector form with `lhs` only. A connector other than `&&` or `||`, a connector without
both sides, or a structure that is not an object at all, is a **validation failure naming the file** — not a rule
that silently never fires.

An empty side object matches nothing, so a rule written that way never fires. An empty `ControlJsonStructure` (`{}`,
as `getsauce.json` and `lux.json` ship) means no progress detection, and the backend's output is shown verbatim.

The values are then read from the line by the generic progress reader (percent, size, speed, ETA, file name)
or, for the six engines with a named `BackendKind` ([architecture](architecture.md) §3.2), by that variant's
parser.

### 5.2 Version rules

| Key | Meaning |
| --- | --- |
| `VersionStringLine` | Which line of the version output to read (0-based) |
| `VersionStringPosition` | Which whitespace-separated token of that line is the version |

### 5.3 What is *not* data

Four behaviours are code, not schema, and pretending otherwise would be dishonest
([architecture](architecture.md) §3.2): yt-dlp `-o` template rewriting, gallery-dl's JSON stream,
svtplay-dl's file-name tracking, get-sauce's banner handling. These are selected by engine name through a
closed enum with a budget of seven variants.


### 5.4 Playlist item ranges are not validated by the original

The range field in the playlist tab is **free text that is split into argv tokens, not parsed as a range**
(`legacy/src/playlistdownloader.cpp:1061-1069`): the text is handed to `util::splitPreserveQuotes`, appended to the
command line after the engine's list options, and saved to the range history as typed. On Qt ≥ 5.15
`splitPreserveQuotes` is `QProcess::splitCommand` (`legacy/src/util.hpp:92-147`), so the field takes quotes and
spaces; on older Qt it is a hand-written splitter with different quote handling.

Consequences, stated plainly because they look like an omission:

- there is **no** range grammar in the original — no `1-3` versus `3-1` check, no "open-ended" form, no rejection
  with a message. `1-3`, `1,3,5`, `abc`, `--yes-playlist` and an empty string all come out the same way: split, or
  absent;
- therefore Morid does **not** introduce a range validator. A user who types rubbish gets the backend's own error,
  exactly as before. A helpful pre-check would reject input the original passes through, which is a compatibility
  break dressed as a feature (D-003, D-016 in spirit);
- what Morid does own is the **token split**, and that is `DownloadOptions` parsing, not a range type: quoting rules
  platform-independent, the result compared byte for byte against the Phase 0 fixture `s07-playlist-item-range`
  (`--playlist-items` `2-3` as two tokens);
- `F-21` keeps its "invalid ranges rejected with a message" wording only as the backend's message. If a later phase
  wants a friendly message, it belongs in the interface layer as a hint, not in `morid-core` as a rule.

---

## 6. Update sources

| Key | Meaning |
| --- | --- |
| `DownloadUrl` | GitHub releases API URL to query for a new binary |
| `DownloadUrlWin7` | Historic Windows 7 alternative; **not used** ([D-005](decisions.md)) |
| `DownloadUrlMAC` | macOS-specific URL where the platform builds differ |

These URLs point at **third-party projects** ([D-014](decisions.md)). Morid does not host backend binaries and
does not replace these addresses with its own. Integrity is checked against the size and SHA-256 (`digest`)
published by the GitHub API; a mismatch aborts the update and leaves the existing binary untouched.

`extensions/` in this repository is the only source of extension *descriptions* ([D-010](decisions.md)). The
original fetched them from a raw GitHub URL pinned to a commit hash (`legacy/src/configure.cpp:876`); that
mechanism is not carried over.

---

## 7. The bundled set

17 files ship in `extensions/`. The original's UI menu offers 10 of them plus `custom`
(`legacy/src/configure.cpp:1231-1252`); the mismatch is a defect (audit D5).

| File | In the original's menu | Morid |
| --- | --- | --- |
| `yt-dlp.json` | yes | yes |
| `yt-dlp-nightly.json` | yes | yes |
| `yt-dlp-aria2c.json` | yes | yes |
| `yt-dlp-ffmpeg.json` | yes | yes |
| `yt-dlp-aria2c-native.json` | **no** | yes — it is a usable variant that was simply unreachable |
| `gallery-dl.json` | yes | yes |
| `you-get.json` | yes | yes |
| `getsauce.json` | yes | yes |
| `svtplay-dl.json` | yes | yes |
| `aria2c.json` | yes (non-Windows) | yes (non-Windows) |
| `wget.json` | yes (non-Windows) | yes (non-Windows) |
| `lux.json` | **no** | yes |
| `safaribooks.json` | **no** | yes |
| `ytdl-patched.json` | **no** | yes, with the `ExtraArguments` correction of §4.1 |
| `deno.json`, `quickjs.json`, `quickjs_ng.json` | **no** (supporting engines, not downloaders) | listed as supporting engines, not in the user-facing downloader menu |

---

## 8. Validation rules for a reader

In order, and the message a user sees must name which one failed:

1. File is valid JSON.
2. `Name` present and non-empty.
3. `Version` ≤ supported schema version.
4. `RequiredMinimumVersionOfMediaDownloader` satisfied (D-004 rules).
5. At least one command resolves for the current platform and architecture — **after** applying the file-name
   transformation of §1 if this is one of the three named thin files. A file with no `Cmd` of its own and no
   transformation is invalid; it is not "resolved through the family".
6. The resolved executable exists, or the extension is marked "needs installation" rather than broken.
7. Unknown keys → warning, not failure.
8. `ExtraArguments*` present → warning pointing at §4.1.

A reader that fails 1–5 keeps the extension out of the list and reports why. A reader that fails 6 keeps it in
the list with an "install" action, because "not downloaded yet" is not the same as "invalid".

---

## 9. Compatibility promise

- Every key documented here works as documented for schema versions 1–4.
- Keys are added, not repurposed. A key with a changed meaning is a new key plus a schema version.
- An unknown key never makes a file fail to load.
- The 17 bundled files, plus any file a user already uses, load without manual edits.
- `DefaultSubstitlesCmdOptions`, `${default}`, `${BackendPath}`, `${CommandName}` and the `2.x` compatibility
  rule are permanent spellings, misspellings included. Fixing them would break files we do not own.
