# Console interface

Related: [features](features.md) C-01…C-06 · [decisions](decisions.md) D-009 · [architecture](architecture.md) §6

Morid ships two binaries: `morid` (this document) and `morid-gui` (the window). The console interface is a
product feature, not a debugging leftover: it is what makes the behaviour testable and what makes Morid usable
from a script, a scheduler or a server.

**Nothing in this document is implemented yet.** It is the specification to build against. Two rules keep it
honest: every command here exists in `docs/features.md` under a `C-` ID, and every promised exit code has a
test ([testing](testing.md)).

---

## 1. Global behaviour

```
morid [GLOBAL OPTIONS] <COMMAND> [COMMAND OPTIONS]
```

| Global option | Meaning |
| --- | --- |
| `--config <dir>` | Use this data directory instead of the default (honours `MediaDownloaderAppDataPath` too) |
| `--json` | Machine-readable output: one JSON object per line (see §5) |
| `--quiet` | Suppress progress output; errors still go to stderr |
| `--verbose` | Include backend output lines in the stream |
| `--no-color` | Disable colour; also implied when stdout is not a terminal |
| `--lang <tag>` | Interface language for messages, e.g. `ru`, `de` |
| `-h`, `--help`; `-V`, `--version` | Usage and version |

Rules:

- Without `--json`, output is for humans and **is not a compatibility surface**. Scripts use `--json`.
- With `--json`, every line on stdout is one complete JSON object. Diagnostics go to stderr, never into the
  JSON stream.
- Colour is never emitted into a pipe, and never in `--json` output.
- No command prompts interactively. Anything that would need a question is an option with a default.

---

## 2. Commands

### 2.1 `morid list <url>`

Fetch what the URL offers: title, duration, thumbnail URL and the format table.

```
morid list https://example.test/v/abc
morid list https://example.test/v/abc --json
morid list https://example.test/v/abc --engine yt-dlp
```

| Option | Meaning |
| --- | --- |
| `--engine <name>` | Use this backend instead of the resolved default |
| `--playlist` | Treat the URL as a playlist and list entries (requires an engine with playlist support) |
| `--options <string>` | Extra options, parsed by the same rules as the GUI ([features](features.md) F-02) |

Exit codes: `0` success, `2` backend refused the URL, `3` backend failed, `4` unsupported by the engine.

### 2.2 `morid download <url>`

```
morid download https://example.test/v/abc
morid download https://example.test/v/abc -f 22 --to ~/Videos
morid download --batch urls.txt --from-file
morid download https://example.test/v/abc --dry-run
```

| Option | Meaning |
| --- | --- |
| `-f`, `--format <spec>` | Format selection; the same string the GUI would put in "Download Options" |
| `--options <string>` | Raw option string, parsed like the GUI's field |
| `--to <dir>` | Destination folder; overrides the configured one |
| `--engine <name>` | Backend |
| `--batch <file>` | Read URLs from a file (F-11) |
| `--archive <file>` | Use this archive file (F-23) |
| `--no-archive` | Ignore the configured archive |
| `--kind <audio\|video\|best>` | Shorthand that maps to a preset of the configured engine |
| `--dry-run` | Print the argv that would run, then exit 0. **Never spawns a process.** |
| `--wait` | Do not exit until the download finishes (default for a single job; implied) |

`--dry-run` is the debugging tool for compatibility: its output is the exact argument vector, and the test suite
compares it with fixtures captured from the original program.

Exit codes: `0` downloaded, `1` usage error, `3` backend failure, `5` cancelled (SIGINT), `6` destination not
writable.

Interrupting with `Ctrl-C` / `SIGINT` cancels the job **tree** and exits `5`; partial files are kept unless
`DeleteFilesOnCanceledDownload` is set.

### 2.3 `morid engines`

```
morid engines
morid engines --json
morid engines --update <name>
morid engines --install <name>
morid engines --remove <name>
```

Lists installed backends with version and path, in the same order the GUI shows them. `--update` downloads a
newer binary using the extension's `DownloadUrl` and verifies size and SHA-256
([extensions](extensions.md) §6). A mismatch is a hard failure: the old binary stays.

Exit codes: `0` success, `3` download or verification failed, `4` unknown engine name.

**As delivered in Phase 2** the command is read-only and covers only the resolution half: `morid engines` prints one
line per bundled extension (file, name, schema version, platform/arch and the architecture actually selected, the
resolved argv, and the verdict `ready` or `needs installation`), and `morid engines --json` prints the same as one
JSON object per line with `--json` accepting no other argument. `--update`, `--install` and `--remove` arrive in a
later phase ([roadmap](roadmap.md) Phase 5); until then the exit codes are `0` when every file resolved and the
backend is present, `4` when at least one is unavailable or invalid, and `1` for a usage error. The distinction that
matters: **a missing backend is not a failure of the extension** — it is reported as `needs installation` and keeps
the file in the list ([extensions](extensions.md) §8 rule 6).

### 2.4 `morid import <dir>` / `morid export <dir>`

Import a Media Downloader data directory, or produce one
([data-compat](data-compat.md) §4 and §6).

| Option | Meaning |
| --- | --- |
| `--force` | Overwrite existing Morid files, after backing them up |
| `--report <file>` | Write the import report as JSON |

Exit codes: `0` success, `7` source directory not recognised, `8` partially imported (report says what).

`8` exists because "some of it worked" is a real outcome and must not be reported as success.

### 2.5 `morid gui`

Starts the window. Present so that a single launcher can decide, and so that `morid` can be installed as the
only entry point on a headless machine.

---

## 3. Exit codes

One table, used by every command. A test exists for each code.

| Code | Meaning |
| --- | --- |
| `0` | Success |
| `1` | Usage error: unknown option, missing argument, mutually exclusive flags |
| `2` | The URL is not supported by the chosen backend |
| `3` | The backend failed (non-zero exit, crash, or a verification failure) |
| `4` | Unknown or unavailable engine |
| `5` | Cancelled by the user |
| `6` | Destination not writable or does not exist |
| `7` | Source directory not recognised (import) |
| `8` | Partial success (import) |
| `9` | Internal error — a bug; please report it |

Codes are stable. Adding a code is a minor version change and a `docs/cli.md` update; renumbering is never
allowed.

---

## 4. Human-readable output

Deliberately plain, one line per event, no cursor tricks when stdout is not a terminal:

```
$ morid download https://example.test/v/abc -f 22
resolving   yt-dlp 2026.10.31
title       Example clip
destination ~/Downloads/Example clip-abc.mp4
progress    42.3%  18.4 MiB / 43.5 MiB  2.1 MiB/s  eta 00:12
done        ~/Downloads/Example clip-abc.mp4  00:21
```

When stdout is a terminal, the `progress` line may be redrawn in place. In a pipe it is emitted as separate
lines so logs stay readable. This is presentation only and carries no compatibility promise.

---

## 5. JSON event stream

With `--json`, stdout is a stream of newline-delimited JSON objects. Each object has `type` and `v` (stream
version, starting at `1`).

```jsonc
{"type":"started","v":1,"job":1,"engine":"yt-dlp","engine_version":"2026.10.31","argv":["yt-dlp","--newline","-f","22","https://..."]}
{"type":"metadata","v":1,"job":1,"title":"Example clip","duration":243,"thumbnail":"https://...","formats":[{"id":"22","ext":"mp4","resolution":"1280x720","filesize":45613056}]}
{"type":"progress","v":1,"job":1,"percent":42.3,"downloaded":19293796,"total":45613056,"speed":2202009,"eta":12}
{"type":"output","v":1,"job":1,"stream":"stdout","line":"[download] Destination: ..."}
{"type":"finished","v":1,"job":1,"state":"success","files":["/home/u/Downloads/Example clip-abc.mp4"],"duration_ms":21034}
{"type":"error","v":1,"job":1,"code":3,"message":"backend exited with 1","backend_exit":1}
```

Rules:

- one object per line; no pretty printing, no trailing commas, UTF-8;
- field names are stable within a stream version; new fields may be added, fields are never repurposed;
- `argv` is the exact vector passed to the operating system — this is what makes `--json` useful for
  compatibility testing;
- `message` is already localised according to `--lang`; `code` is not, and is what scripts should branch on.

A stream version bump is required for: removing a field, changing a field's type or meaning, or changing the
event set. Consumers must ignore unknown fields.

---

## 6. What is deliberately not offered

- **No daemon mode.** The single-instance IPC of the GUI (F-62) is a convenience for the desktop, not a service
  API; running Morid as a long-lived server is not supported and not promised.
- **No config editing from the command line.** Writing `settings.ini` by hand or through the GUI is supported;
  inventing a `morid config set` command would create a second, undocumented settings surface.
- **No plugin subcommands.** Engines are extensions ([extensions](extensions.md)); the console does not grow a
  parallel extension mechanism.
