<div align="center">

# Morid

**A GUI for command-line media downloaders — rewritten in Rust.**

[![license](https://img.shields.io/badge/license-GPL--2.0--or--later-0b3d91?style=flat-square)](LICENSE)
[![status](https://img.shields.io/badge/status-pre--alpha-orange?style=flat-square)](docs/roadmap.md)

</div>

---

## What this is

Morid is a graphical application for downloading video, audio and other media from the internet. It is a
frontend: the actual downloading is done by well-known command-line tools, primarily
[yt-dlp](https://github.com/yt-dlp/yt-dlp), and by others such as [gallery-dl](https://github.com/mikf/gallery-dl),
[you-get](https://github.com/soimort/you-get), [aria2c](https://aria2.github.io/) and
[wget](https://www.gnu.org/software/wget/).

Morid is a from-scratch **Rust** reimplementation of [Media Downloader](https://github.com/mhogomchungu/media-downloader)
by Francis Banyikwa. The original C++/Qt program is the functional reference; it is kept in
[`legacy/`](legacy/) for comparison and is not the product.

## Project status

**Pre-alpha. The workspace builds, and does nothing.**

Phase 1 is a skeleton: eight crates that compile, an empty library in each, a console binary whose only
feature is `morid --version`, and a Slint window that shows "Morid 0.1.0" and closes. One workflow runs on
Linux, macOS and Windows: `rustfmt --edition 2021 --check` over the tracked files (not `cargo fmt`, whose
wrapper rejects its own flags on the pinned toolchain), `cargo clippy -D warnings`, `cargo test`,
`cargo build --release`, an offline rebuild, a smoke run of both binaries, the size report — measured and
published, never gated — `cargo deny check`, `cargo audit`, and the Phase 0 fixture checks on one platform.
There is no download functionality yet: the crates are shapes, not behaviour, and Phase 2 starts filling them.
What exists today:

| Area | State |
| --- | --- |
| Audit of the original C++ code base | done — [`docs/audit-original.md`](docs/audit-original.md) |
| Product decisions (15 open questions) | decided — [`docs/decisions.md`](docs/decisions.md) |
| Functional map (every feature, priority, acceptance criterion) | done — [`docs/features.md`](docs/features.md) |
| Architecture, roadmap, migration plan | done — [`docs/architecture.md`](docs/architecture.md), [`docs/roadmap.md`](docs/roadmap.md), [`docs/migration.md`](docs/migration.md) |
| Rust workspace, CI, first code | done — Phase 1: eight crates, six checks on three platforms, size reported |
| Repository layout: C++ in `legacy/`, 10 inherited workflows retired | done — see [`legacy/`](legacy/) |

The roadmap explains the order and why it is that order: [docs/roadmap.md](docs/roadmap.md).

## What it will do

Everything the original program does, which is more than "download a link":

- download video and audio, with format and quality selection driven by the backend;
- show the formats a URL offers and let the user pick one;
- support yt-dlp, gallery-dl, you-get, svtplay-dl, get-sauce and other backends through the same JSON
  extension format the original uses;
- batch downloads, including reading links from a file;
- playlists, with ranges and selective download;
- subscriptions to playlists, checked against a download archive so nothing is fetched twice;
- an unlimited number of concurrent downloads, with a configurable limit;
- per-engine default options, reusable presets, and URL rules that select an engine and options
  automatically;
- cookies from a browser or a file, proxy configuration, text encoding handling;
- the same 18 interface languages, dark themes, tray icon and notifications;
- a download library to browse, rename and delete what was downloaded;
- a console interface (`morid`) that does all of the above without a window.

A console interface is a product feature here, not an afterthought: it is what makes the behaviour testable
and it is described in [docs/cli.md](docs/cli.md).

## What it will not do

- **No mirroring for right-to-left languages.** Arabic is translated and renders correctly, but the
  interface layout is not mirrored. Slint, the chosen UI toolkit, does not support interface direction yet;
  see [D-002](docs/decisions.md). The item is on the roadmap with no release attached.
- **No Windows 7, 8 or 8.1.** Windows 10 and 11, x86_64 only; see [D-005](docs/decisions.md).
- **No self-updating.** Morid checks for a new version and tells you; it does not install it; see
  [D-013](docs/decisions.md).
- **No lossy assumptions.** Morid never silently changes what a backend downloads.

## Install

There are no packages yet. When there are, they will be listed in
[docs/releases.md](docs/releases.md).

## Build from source

Once the workspace exists (Phase 1):

```sh
cargo build --release
```

Until then the only buildable code in this repository is the C++ original in [`legacy/`](legacy/), which is
documented in [docs/building.md](docs/building.md) §7 and is not the product. Building it needs Qt and is
optional; CI never does it.

## Documentation

| Document | Contents |
| --- | --- |
| [audit-original.md](docs/audit-original.md) | What the original program is, how it is built, what was found |
| [decisions.md](docs/decisions.md) | Every product decision taken by the owner, with reasons |
| [features.md](docs/features.md) | Functional map: each feature, where it lives, priority, acceptance criterion |
| [architecture.md](docs/architecture.md) | Rust design: crates, boundaries, how the pieces talk |
| [migration.md](docs/migration.md) | How the C++ behaviour becomes Rust behaviour, and how equivalence is proven |
| [roadmap.md](docs/roadmap.md) | Phases, what each phase delivers, what is deliberately deferred |
| [extensions.md](docs/extensions.md) | The extension JSON contract — 34 keys, schema versions 1–4 |
| [data-compat.md](docs/data-compat.md) | The original's settings and data files, and how they are imported |
| [cli.md](docs/cli.md) | Console interface: commands, exit codes, output format |
| [testing.md](docs/testing.md) | Test strategy, fixtures, and what is *not* covered |
| [dependencies.md](docs/dependencies.md) | Every dependency and the argument for it |
| [platforms.md](docs/platforms.md) | Supported platforms and what each one requires |
| [building.md](docs/building.md) | Building, cross-compiling, packaging |
| [workspace-manifest.md](docs/workspace-manifest.md) | Exact target content of `Cargo.toml`, `rust-toolchain.toml`, `deny.toml` |
| [releases.md](docs/releases.md) | Release process, artefacts, checksums |
| [tasks/](docs/tasks/) | Assignments for the coder: Phase 0 (fixtures), Phase 1 (skeleton) |

## Licence

Morid is free software under the **GNU General Public License, version 2 or (at your option) any later
version**. See [LICENSE](LICENSE). It is a derivative work of Media Downloader; the original author's
copyright notices in [LICENSE.txt](LICENSE.txt) are retained and must not be removed.

Third-party components and their licences are listed in [THIRD-PARTY.md](THIRD-PARTY.md).

## Credit

Morid exists because of [Media Downloader](https://github.com/mhogomchungu/media-downloader) by
**Francis Banyikwa** (`mhogomchungu@gmail.com`), Copyright (c) 2021–2026, GPL-2.0-or-later. The functional
specification, the extension format and the user data formats are his work. Any defect in the Rust
reimplementation is ours, not his.

### Phase 1 acceptance evidence

Continues the original skeleton commits `e1192eb…fe0f6a0`, rather than replacing them:
eight crate stubs, the separate `xtask`, inline Slint window and matrix CI were retained.
Repairs include the Slint feature/MSRV selection, licence policy, committed lock file,
offline build, GUI smoke checks, size report and guarded Phase 0 checks.
No downloader functionality is implemented. See `docs/workspace-manifest.md` §7 for deviations.

- [Green three-platform baseline, committed lock](https://github.com/RUdolf1517/Morid/actions/runs/42460111802):
  Linux 7m31s, Windows 7m31s, macOS 7m30s (job wall clocks; all below 15 minutes).
- [Earlier green run with detailed evidence](https://github.com/RUdolf1517/Morid/actions/runs/42459576443):
  Linux size `morid-gui: 5749160 bytes (5.48 MiB)`, `morid: 326040 bytes (0.31 MiB)`;
  CLI tree contains only Morid's skeleton crates, no Slint or GUI;
  Phase 0: `Ran 30 tests`, `OK`, `VERIFY_OK`.
- [Negative licence probe](https://github.com/RUdolf1517/Morid/actions/runs/42460673154)
  at `c3455d39`: temporary workspace crate `licence-probe`, declaring `GPL-3.0-only`,
  caused `cargo deny` to fail (exit 4):

  ```text
  error[rejected]: failed to satisfy license requirements
  crates/licence-probe/Cargo.toml:10:12
  10 | license = "GPL-3.0-only"
     | rejected: license is not explicitly allowed
  ```

  The probe is removed. This tested a disallowed crate against the policy;
  it did **not** literally break `deny.toml`, as criterion 8 requests.
- [No-C-compiler experiment](https://github.com/RUdolf1517/Morid/actions/runs/42460673068):
  `cc=none gcc=none clang=none`. Full GNU build failed with `linker cc not found`
  (exit 101); the host procedural macro build scripts need a native linker driver.
  Instead the console target was built with
  `RUSTFLAGS="-C linker=rust-lld -C link-self-contained=yes"` for
  `x86_64-unknown-linux-musl`: exit 0, 410424 bytes, `morid 0.1.0`.
  This proves only the console alternative, not a no-C-toolchain GUI build.
- The empty-test-pattern check locally reports `Ran 0 tests` and exits successfully;
  applying CI's exact-count guard (`30`) rejects it. The specification's count of 16
  is stale; tests and fixtures were not changed to fit it.

Known limitations: no local Rust toolchain, so Rust checks were executed in Actions,
not locally. Linux/macOS GUI smoke checks pass; Windows lacks `glCreateShader` and
that exact initialization error is a warning, not proof that a window opened.
The `unmaintained = "workspace"` cargo-deny policy is a reviewable deviation:
transitive unmaintained advisories are not gating, while vulnerabilities remain gating
and cargo-audit still reports warnings. Binary size policy D-008 does not authorize
this advisory-policy change. Temporary diagnostic workflows and lock emitters are removed.
