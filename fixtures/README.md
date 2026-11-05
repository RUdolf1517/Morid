# Phase 0 reference fixtures

These fixtures record the behaviour of the unmodified C++ application in `legacy/`. No Rust or
product code is involved. The reference is media-downloader **5.6.8**, at commit
`17c0104b8824bdad7620b2e5127484ca42dde98e` (`origin/main` when captured), running on **Linux x86_64**
with **Qt 5.15.16 LTS (LGPL)**. Raw streams were captured on 2026-11-04; scenario argv and userdata
were captured on 2026-11-05. Each scenario's `meta.json` repeats the commit and platform provenance.

## Inventory

- `scenarios/`: all 40 required cases, indexed by `manifest.json`; 36 are `captured` and four are
  `not-captured` for the observed reasons below.
- `streams/`: raw output from all seven required backends. Every capture has a URL class, command,
  transport, exit status, byte count and SHA-256 in the backend's `meta.json`.
- `userdata/media-downloader/`: an anonymized data directory written by the original application,
  including 94 settings keys, 20 download-history objects, both generated themes and a real yt-dlp
  archive entry. `userdata/meta.json` records a successful reload by the original application.

No downloaded media, backend executables, cookies, tokens, personal data or real credentials are
fixtures. The only credential-shaped values are the task-specified dummy proxy `user:pass@host:8080`.
The `.bin` files under `streams/yt-dlp/` are small raw process-output byte sequences, not media or
executables.

## Backend versions

| Backend | Version used for raw streams |
| --- | --- |
| yt-dlp | 2026.10.26 |
| gallery-dl | 1.32.19 |
| wget | 1.21.3 |
| aria2c | 1.37.0 (PyPI `aria2==0.0.1b0` wheel) |
| you-get | 0.4.1743 |
| svtplay-dl | 4.211 |
| get-sauce | 1.10.0, built from source commit `994bc525b764f6f5d681143562f96c9457b5bcc7` |

The argv wrapper reports these versions only to satisfy the original application's startup probes.
It is not the source of any raw stream fixture.

## How argv capture works

`tools/capture/capture.py` creates an isolated appdata directory for each scenario, installs the
shipped extension JSON files, writes scenario-specific QSettings and drives the original widgets.
`tools/capture/qt_driver.cpp` is built as an external `LD_PRELOAD` helper; it interposes only
`QCoreApplication::exec()` to schedule UI actions. Nothing in `legacy/` is changed or linked into the
helper.

Backend names in the isolated `bin/` directory resolve to `backend_wrapper.py`. At the actual process
boundary the wrapper records `sys.argv[1:]`, the invoked program, working directory and selected proxy
variables as JSONL. Argument boundaries therefore come from `QProcess`, not from reparsing a debug
line. Startup version probes go to a disposable separate transcript. The scenario transcript becomes
`log.txt`; its first record is copied mechanically to `argv.json`. Multi-process cases also have
`invocations.json` and `argv-2.json`. The post-download command in s38 has `post-log.txt` and
`post-argv.json`.

s10, s16, s18 and s19 disable batch metadata previews so their golden transcripts contain the requested
download/subtitle/comments operations rather than unrelated metadata probes. s19 enables the original
`AutoSetDefaultEngineAndOptions` option so the user-supplied chapter output template is attached to the
batch row. s26 does not manually choose an engine: its wrapper records the original's actual fallback
when only a different architecture has a `Cmd` branch.

## How stream and userdata capture works

`capture_streams.py` runs the real backends against a local deterministic HTTP server. The response
bodies and downloaded files stay in a disposable cache. Each fixture file is the backend's merged
stdout/stderr, written directly without editing. Transport is a pipe except gallery-dl progress, which
uses a pty to obtain genuine progress updates. yt-dlp interruption is triggered by observed progress,
not solely by elapsed time. Local URLs are represented as `<local-url>` in metadata; only URL-class
descriptions are committed.

The yt-dlp set additionally includes CR progress, a final partial line, interrupted output and a real
Latin-1 process-output capture. wget has no JSON listing fixture because its tested interface has no
corresponding listing mode. Optional lux and safaribooks stream sets were not produced; their absence
does not affect the seven required backends.

`capture_userdata.py` seeds disposable settings, drives the original Configure, Subscription, Batch and
Basic tabs, and exits through the original Configure Quit path so `autoSavedList.json` is persisted.
Successful wrapper downloads generate 20 history entries. A real yt-dlp invocation against the local
server creates the archive entry. Only disposable workspace paths are replaced with `/home/user` in
the resulting copy; URLs were created as `https://example.com/...` from the outset. The fixture is then
loaded from a disposable copy by the original application as a reload check.

## Re-capture and verification

The helpers use the capture cache `/home/user/.cache/morid-capture`. Install the versions above in its
`venv/`, provide Qt 5.15.16 with Core/Gui/Widgets/Network and the `minimal` QPA plugin in `qt5/`, then
run:

```sh
cd /home/user/Morid
export PATH=/home/user/.cache/morid-capture/venv/bin:$PATH
export QT_PREFIX=/home/user/.cache/morid-capture/qt5
export CAPTURE_DRIVER=/home/user/.cache/morid-capture/qt-driver.so
export PYTHONDONTWRITEBYTECODE=1

tools/capture/build-reference.sh
python3 tools/capture/capture_streams.py
python3 tools/capture/capture_userdata.py
python3 tools/capture/capture.py
python3 tools/capture/verify_fixtures.py
```

Qt is run with `QT_QPA_PLATFORM=minimal`; no display server is required. QtNetwork must be built with
SSL declarations available (the capture build used OpenSSL 3.0.18 in runtime mode), because the legacy
source refers to `QSslSocket` even though these scenarios do not use real remote URLs.

The normal stream and userdata commands keep existing snapshots and rerun verification. Scenario argv
is genuinely recaptured on every full run. A second normal full run with the same reference and
versions must produce no fixture diff. `--force` on the stream or userdata helper explicitly replaces
those snapshots; real progress timing and history timestamps can then differ and are not part of the
clean-tree idempotency check.

`verify_fixtures.py` is read-only. It checks all 40 manifest entries, transcript/argv equality and
backend identity, raw-stream hashes and awkward bytes, userdata structure and anonymization, comparator
positive/negative behaviour, and fixture size/artifact constraints.

## Comparator

Golden and candidate vectors are compared element by element; order is significant and shell quoting
is not. Candidate input can be JSON, wrapper JSONL, one-argument-per-line text, or a future dry-run
command's output.

```sh
tools/compare-argv s01-plain-yt-dlp \
  --candidate fixtures/scenarios/s01-plain-yt-dlp/argv.json

# Phase 3 can use the same comparator without changing its interface:
tools/compare-argv s01-plain-yt-dlp -- morid --dry-run 'https://example.com/v/fixture-01'
```

A match exits 0; a moved, missing, added or changed argument exits 1 and prints indexed golden/candidate
columns. This helper may be folded into `xtask` in Phase 1 if it remains useful; it is not a product or
Cargo dependency.

## Known gaps and observed reference behaviour

The following commands can be used to reproduce individual gaps:

```sh
python3 tools/capture/capture.py --only s28-auto-update-false
python3 tools/capture/capture.py --only s33-you-get
python3 tools/capture/capture.py --only s35-unknown-extension
python3 tools/capture/capture.py --only s36-missing-required-key
```

- **s28, not captured:** `AutoUpdate=false` is handled by the original's network/version-update logic;
  there is no backend update process and therefore no external update argv for the wrapper to record.
- **s33, not captured:** on Linux x86_64 the shipped Generic branch invokes `you-get` directly. The
  interpreter invocation is in the Windows `Cmd` branch, unavailable on this capture host. The observed
  direct invocation is preserved as `observed-argv.json`, not presented as the requested interpreter
  golden.
- **s35 and s36, not captured:** the original does not produce the task's expected empty argv for an
  unknown extension or an extension missing `Name`. It rejects/skips that extension and falls back to
  another available engine. The real fallback argv and driver log are preserved. This is a reference
  behaviour discrepancy to account for under `docs/migration.md` §6, not something these fixtures
  correct or hide.
- **s26, captured fallback:** with `Cmd` only for aarch64 on an x86_64 host, the original likewise
  falls back to yt-dlp. This case is captured because fallback behaviour is precisely what the task
  requests for s26; `meta.json` records the observation.

No scenario array was hand-written to fill these gaps. Optional lux/safaribooks raw streams remain
unproduced, as noted above.
