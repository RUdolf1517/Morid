# Dependencies

Related: [architecture](architecture.md) · [platforms](platforms.md) · [testing](testing.md) ·
[THIRD-PARTY.md](../THIRD-PARTY.md)

The default answer to "should we add a crate?" is **no**. Each dependency is a permanent cost: binary size,
compile time, supply-chain surface, licence obligations, and the day it stops being maintained. This document
records the argument for every crate that survives that answer, and the argument against the ones that were
considered and rejected.

It is written before the code exists. Entries below are therefore **proposals with a decision attached**, and a
pull request that implements them must confirm the measured size rather than the estimate.

**Language and toolchain:** Rust 1.85 or newer, edition 2021. The floor is set by Slint's current minimum; it
will be raised only when a dependency forces it.

---

## 1. The procedure

Before a dependency is added, answer these six questions in the pull request:

1. Does it implement something **mandatory**, or merely convenient?
2. Can the standard library do it in a reasonable amount of safe code?
3. Can a crate already in the tree do it?
4. What does it cost in binary size, compile time and supply-chain surface?
5. Is it maintained, and is its licence compatible with GPL-2.0-or-later?
6. Does it duplicate something we already have?

Additional rules:

- default features disabled unless a default feature is genuinely required;
- no `build.rs` that downloads anything, generates code from the network, or compiles C — the build must work
  offline (`cargo build --offline`);
- no `unsafe` in our code; `unsafe` in a dependency must be reviewed and justified;
- the measured size delta goes into this document.

A dependency that fails question 1 is rejected even if it is small and popular.

---

## 2. Accepted dependencies

### 2.1 `slint` — the user interface

- **Why mandatory:** the owner chose Slint ([D-001](decisions.md)). It is the only Rust toolkit that matches
  the original's declarative UI model, and it produces compact binaries.
- **Licence:** royalty-free with attribution, or GPLv3, or commercial. Morid uses the royalty-free option
  ([D-007](decisions.md)); attribution lives in the About window and in `THIRD-PARTY.md`.
- **Configuration:** `slint` with the default renderer; `slint-build` as a build dependency to compile `.slint`
  files. The `gettext` feature is enabled in Phase 7 for translations.
- **Cost:** the largest single contributor to the GUI binary. Measured per release and reported by CI
  ([D-008](decisions.md)).
- **Rejected alternatives:** `iced` (no RTL support at all, weak accessibility), `egui` (immediate mode is a bad
  fit for a form-heavy desktop application, weak accessibility), Qt through a C++ bridge (contradicts the
  purpose of the project).

### 2.2 `slint-build` — compiling `.slint` files

- Build dependency of `morid-gui` only. Same licence as Slint. Does not ship in the binary.
- **Constraint:** it must not require network access at build time. If it does, the build is broken by
  definition.

### 2.3 `serde` + `serde_json` — the extension and data formats

- **Why mandatory:** the extension schema, the presets, the URL rules, the subscriptions and the history are
  JSON, and they are a public contract ([extensions](extensions.md)). A hand-written JSON parser would be a
  correctness risk and would not save meaningful size — `serde_json` is the reference implementation of parse
  behaviour users already depend on.
- **Licence:** MIT OR Apache-2.0.
- **Cost:** moderate; `serde` derive expands code, so `serde_json` is used with `preserve_order` off and
  `arbitrary_precision` off (both default) to keep the tree small.
- **Note:** unknown keys are preserved ([extensions](extensions.md) §1), which needs `serde_json::Value` for
  passthrough rather than a strict struct for the whole file. The struct covers the known keys; the raw value is
  kept alongside.

### 2.4 `sha2` — integrity of downloaded backend binaries

- **Why mandatory:** a downloaded executable must be verified against the SHA-256 published by its source
  ([extensions](extensions.md) §6). Shipping an unverified binary is a security defect, not a feature gap.
- **Licence:** MIT OR Apache-2.0.
- **Configuration:** `default-features = false` — only `Sha256` is needed.
- **Rejected alternative:** shelling out to `sha256sum`/`certutil`. Not available everywhere, locale-dependent
  output, and it makes the security property depend on `PATH`.

### 2.5 HTTP client — `ureq` (proposed)

- **Why mandatory:** release metadata and binary downloads need HTTPS.
- **Proposal:** `ureq` with `rustls`, `default-features = false`. Synchronous, small tree, no async runtime
  dragged in for four request types.
- **Alternative:** `reqwest` with `rustls-tls` — better known, larger, pulls `tokio`. Rejected on size unless a
  requirement appears that `ureq` cannot meet (HTTP/2 is not one).
- **Decision point:** Phase 5 ([architecture](architecture.md) §10, A2).
- **Licence:** both MIT OR Apache-2.0.

### 2.6 `rustls` + `webpki-roots` — TLS without a C toolchain

- **Why mandatory:** TLS is not optional for HTTPS, and the build must not require a C compiler or system
  OpenSSL (`docs/platforms.md` §5). `rustls` is pure Rust with `ring`/`aws-lc` behind a feature.
- **Licence:** MIT OR Apache-2.0 (rustls); ISC/BSD-style for `ring`, which contains `unsafe` and must be
  reviewed once, in writing, before it is accepted.
- **Open question:** `ring` versus `aws-lc-rs` versus the platform verifier. The last option would use the
  operating system's trust store and certificate validation, which is smaller and matches user expectations on
  corporate networks, at the cost of three platform-specific implementations. To be settled in Phase 5.

### 2.7 Archive unpacking — `tar` + `flate2` + `zip` (proposed)

- **Why:** installing a backend means unpacking what its project publishes: `.tar.gz`, `.zip`, and on Windows
  `.zip`. The original shells out to `bsdtar`/`unzip`, which means a missing system tool breaks extension
  installation, and extraction is where path traversal bugs live
  ([SECURITY.md](../SECURITY.md)).
- **Proposal:** `tar` + `flate2` (pure-Rust `miniz_oxide` backend, no C) + `zip`.
- **Alternative kept open:** keep shelling out, as the original does. Cheaper in size, but adds a runtime
  dependency on a system tool and delegates the security property to it.
- **Licence:** MIT OR Apache-2.0 (`tar`, `flate2`), MIT (`zip`).
- **Decision point:** Phase 5. The agent of the decision may well be "which is measurably smaller", since both
  are defensible.

### 2.8 `libc` (Unix only) — process groups and signals

- **Why mandatory:** cancellation must kill a process **tree** ([architecture](architecture.md) §5).
  `setsid`/`killpg` are not exposed by the standard library.
- **Configuration:** `[target.'cfg(unix)'.dependencies]`, so it is not in the Windows build at all.
- **Licence:** MIT OR Apache-2.0.
- **Alternative rejected:** `nix`, which wraps the same calls with more surface than needed.

### 2.9 `windows-sys` (Windows only) — Job Objects

- **Why mandatory:** the same requirement on Windows. The original uses `taskkill`, which is a quoting and
  localisation hazard; Job Objects with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` are the correct mechanism.
- **Configuration:** `[target.'cfg(windows)'.dependencies]`, selected features only.
- **Licence:** MIT OR Apache-2.0.

### 2.10 `thiserror` — error boilerplate (candidate, not settled)

- **Why considered:** every crate has one `enum Error` with a `Display` implementation; deriving it is a
  hundred lines saved per crate.
- **Counter-argument:** the project's convention ([CONTRIBUTING](../CONTRIBUTING.md) §4) wants one error enum per
  crate, and that is three enums in total. Hand-writing `Display` for three types is a day's work, once. A
  proc-macro dependency for that is questionable.
- **Decision:** hand-write the three implementations in Phase 2. Revisit only if the count grows.

### 2.11 `directories` / `dirs` — application data path (candidate, likely rejected)

- The three platforms need `%APPDATA%`-equivalent, `~/Library/Application Support`, and XDG. That is three
  environment lookups and a documented fallback ([data-compat](data-compat.md) §1).
- The original also has to support the `MediaDownloaderAppDataPath` override, which the crate does not know
  about, so the call site grows anyway.
- **Decision:** implement in `morid-store`, ~80 lines, no dependency. Revisit if the platform rules turn out to
  be subtler than expected.

### 2.12 Time formatting / ETA (`chrono`, `time`) — rejected

Progress output needs seconds and `HH:MM:SS`, and history needs a timestamp. That is arithmetic on integers and
one `SystemTime` call. A time-zone database in a downloader is not defensible.

### 2.13 Naming, paths, globbing — rejected

`same-file`, `glob`, `pathdiff`, `sanitize-filename`: each solves a few lines of a problem whose correct answer
depends on a policy we must define ourselves anyway (what *is* a safe file name for a title with a slash in it
on Windows?). Policy in `morid-core`, code to implement it next to the policy.

### 2.14 Async runtime — not adopted

`tokio` is not in the dependency set. The program's work is dominated by child processes and line parsing; an
async runtime buys little and costs a great deal of binary (see [architecture](architecture.md) §10, A1).
Reconsider only if the scheduler demonstrably needs it.

---

## 2.1 Rejected: an INI parsing crate

`configparser`, `ini` and similar crates are refused, and this is not a size judgement — they cannot do the job.

The file is `settings.ini`, written by the reference program through `QSettings` on **Qt 5 with no
`setIniCodec` call**. Qt documents the result precisely: "no codec is used, and non-ASCII characters are encoded
using standard INI escape sequences", generating "pure ASCII files". A `DownloadFolder` of `/x/Загрузки` is
therefore on disk as `/x/\x417\x430...` — one `\x<hex>` per UTF-16 code unit, inside an INI dialect that no
general-purpose parser implements.

Reading that file with a byte-oriented INI crate yields a path with literal backslashes in it; writing it back as
UTF-8 produces a file the original cannot read. Either way the user's download folder is lost. This is why
D-016 made the encoding a compatibility requirement rather than a preference.

**What is used instead:** a reader and writer for Qt's INI dialect, inside `morid-store`, with the `\x<hex>` rule,
the `%XX`/`%UXXXX` key-escaping rule, `[Section]` grouping, `;`/`#` comments and Qt's writer quirks. It is
perhaps 200 lines plus tests — smaller than the argument needed to justify a dependency that still would not
work. The test fixture is a real `settings.ini` produced by the reference program containing a non-ASCII path
([data-compat](data-compat.md) §3.0).

---

## 3. Logging

No logging crate in the dependency set for now. What is needed is: a ring buffer of lines per job, a file for
the log window, and history. That is a struct and a `VecDeque`. `tracing` and `log` are valuable in services
with distributed context; here they would add a facade, a subscriber and a filter language to write lines into a
window that already exists.

Revisit if a user-visible diagnostic mode ever needs structured, levelled output across crates; the facade would
then be `log` with a hand-written sink.

---

## 4. Localisation

**gettext `.po`**, with Slint's `gettext` feature ([architecture](architecture.md) §10, A4).

- **Why:** Slint supports gettext natively for `@tr()`; the existing 18 Qt `.ts` files convert mechanically
  (same message text, same context); translators already know the format, and tools exist.
- **Cost:** the `gettext` feature pulls a small dependency tree. Measured in Phase 7.
- **Constraint from [D-002](decisions.md):** the RTL gap is a toolkit limitation, not a catalogue limitation —
  the Arabic catalogue works, the layout does not mirror.

---

## 5. Licence policy

- The project is **GPL-2.0-or-later**. A dependency must be compatible with it under at least one of its
  options. Permissive licences (MIT, Apache-2.0, BSD, ISC, Zlib, Unlicense) are compatible.
- **GPL-3.0-only code cannot be linked** into a GPL-2.0-**only** distribution of the combined work, and our
  stated licence is "or later", which permits the combination — but it would effectively force every
  distributed binary to GPL-3.0. That is a decision for the owner, not for a pull request. No GPL-3.0-only crate
  is currently proposed.
- A dependency with **no** licence, or with a non-commercial clause, is refused.
- `cargo deny` enforces an allow-list in CI: `MIT`, `Apache-2.0`, `Apache-2.0 WITH LLVM-exception`,
  `BSD-2-Clause`, `BSD-3-Clause`, `ISC`, `Zlib`, `Unlicense`, `MPL-2.0` (file-level copyleft, acceptable for
  unmodified use, to be reviewed per crate).
- Attribution obligations (Slint's, and any Apache-2.0 `NOTICE`) are recorded in
  [THIRD-PARTY.md](../THIRD-PARTY.md) in the same pull request that adds the crate.

---

## 6. Size history

Required by [D-008](decisions.md): the size is measured and published, not gated. Every release appends a row.

| Date | Commit | `morid-gui` (Linux x86_64, release) | `morid` (Linux x86_64, release) | Note |
| --- | --- | --- | --- | --- |
| CI run 42459576443 | `4b3ae528` | 5,749,160 bytes (5.48 MiB) | 326,040 bytes (0.31 MiB) | Skeleton only; [three-platform CI](https://github.com/RUdolf1517/Morid/actions/runs/42459576443) |
| CI run 42881799249 (Linux job) | `564284b4` | 5,749,160 bytes (5.48 MiB) | 514,472 bytes (0.49 MiB) | Phase 2 complete: domain, extension reader, `engines` CLI. The GUI binary is unchanged because Phase 2 adds no window code; the console binary grew by 188,432 bytes over the skeleton, which is the `serde`/`serde_json` reader and the resolver. [measurement](https://github.com/RUdolf1517/Morid/actions/runs/42881799249) |

For reference, the program being replaced ships as a Qt bundle: a Qt6 Windows installer is in the tens of
megabytes before backends are counted. The Rust target is expected to be in single-digit megabytes for the
console binary and in the low tens for the GUI, most of it Slint and TLS. Those are expectations, not promises;
the table above is the only claim that counts.

---

## 7. Supply chain

- `Cargo.lock` is committed. The build is reproducible from the lock file.
- `cargo deny check` and `cargo audit` run in CI on every pull request.
- New direct dependencies are reviewed by the project owner in the pull request that adds them; a dependency
  bump that crosses a major version is a review item, not a chore.
- No dependency may execute code at build time that fetches from the network.
- Vendoring is not used. If a dependency disappears, the replacement is a decision, not a fork.
