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

Lookup order: the platform-specific block (`Windows` / `MacOS`), then `Generic`. Within a block the
architecture key, then a fallback. Architecture keys observed in the shipped files:
`amd64`, `x86`, `aarch64`, `win7amd64`, `win7x86`.

**Change from the original.** Windows 7/8 architecture keys (`win7*`) are **not used**
([D-005](decisions.md)). A file that only defines `win7*` keys for Windows is reported as unsupported on the
supported Windows platforms rather than silently falling back to an x86 build that may not run.

`Args[0]` is the executable name; the rest are its fixed arguments. A name is resolved as: an absolute path,
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
| `PlaylistItemsArgument` | string | The flag for an item range, e.g. `--playlist-items` |
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
| `ExtraOptions` / `ExtraOptionsFlatpakAppImage` / `ExtraOptionsWin7` | string[] | Arguments added only on that packaging/platform |

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

Operators available on each side: `startsWith`, `endsWith`, `contains`, `containsAny` (array),
`containsAll` (array). `Connector` is `&&` in every shipped file; `||` is not implemented by the original and
is **not** part of the contract.

Evaluation: a line is a progress line when `lhs` and `rhs` both hold. An empty object is "no condition". An
empty `ControlJsonStructure` means "no progress detection" and the backend's output is shown verbatim.

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
5. At least one command resolves for the current platform and architecture.
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
