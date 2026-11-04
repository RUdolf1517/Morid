# Third-party notices

Morid is a frontend: it downloads nothing by itself. It runs external command-line programs chosen by the
user and described by JSON files in [`extensions/`](extensions/). Those programs are **separate processes**,
not linked libraries; no code from them is compiled into Morid.

This file lists what ships with Morid, what it links, what it runs, and under which licence — so that a
distribution packager does not have to unpack the build to find out.

Licence identifiers below were taken from each upstream project on 4 November 2026. They are stated for
convenience; the licence file shipped by the project is authoritative.

## 1. Morid itself

**GPL-2.0-or-later.** See [LICENSE](LICENSE).

Morid is a derivative work of [Media Downloader](https://github.com/mhogomchungu/media-downloader),
Copyright (c) 2021–2026 Francis Banyikwa, GPL-2.0-or-later. The original author's notice is retained in
[LICENSE.txt](LICENSE.txt) and must not be removed. The C++ source of that program is kept in
[`legacy/`](legacy/) and is not part of the Morid binary.

`GPLv2` and `GPLv3` in the repository root are the licence texts as the original project shipped them. The
normative statement for Morid is in `LICENSE` and `LICENSE.txt`.

## 2. Rust crates

The dependency set is not final until Phase 1 of the roadmap; the candidate list and the argument for each
crate are in [docs/dependencies.md](docs/dependencies.md). This section is updated when `Cargo.lock` exists,
because until then any list would be a guess.

The single dependency that is not negotiable is the **UI toolkit**:

| Crate | Used for | Licence |
| --- | --- | --- |
| [`slint`](https://crates.io/crates/slint) | the entire interface | Royalty-free licence, or GPLv3, or commercial — Morid uses the royalty-free option ([D-007](docs/decisions.md)) |

**Attribution requirement.** The royalty-free Slint licence requires that the use of Slint be acknowledged in
an appropriate place. Morid acknowledges it in the About window and in this file. If you distribute Morid, keep
that acknowledgement.

Slint's own repository is REUSE-compliant and contains parts under other licences (documentation and examples
are MIT, some assets keep their own terms); none of those parts are copied into Morid.

## 3. Backend tools — run, not linked

These are executed as separate programs. Morid does not distribute them, does not link them, and does not
include their source. Installing and updating them, and complying with their licences, is the responsibility
of whoever distributes them — Morid's own installers download them from their upstream release pages
([D-014](docs/decisions.md)).

| Tool | Used for | Licence | Notes for packagers |
| --- | --- | --- | --- |
| [yt-dlp](https://github.com/yt-dlp/yt-dlp) | the default downloader | Unlicense (public domain) | no obligations; the Unlicense text is in the project |
| [gallery-dl](https://github.com/mikf/gallery-dl) | image galleries | GPL-2.0 | separate program; its GPL obligations apply to it, not to Morid |
| [you-get](https://github.com/soimort/you-get) | alternative downloader | MIT | — |
| [svtplay-dl](https://github.com/spaam/svtplay-dl) | Swedish broadcasters | MIT | — |
| [get-sauce](https://github.com/gan-of-culture/get-sauce) | image/video sites | MIT | — |
| [lux](https://github.com/iawia002/lux) | alternative downloader | MIT | extension present, not in the default menu |
| [aria2](https://aria2.github.io/) | accelerated downloading as a helper for yt-dlp | GPL-2.0 | separate program |
| [wget](https://www.gnu.org/software/wget/) (and wget2) | plain HTTP downloads | GPL-3.0 | **GPL-3.0**: a GPL-3.0 binary distributed *alongside* a GPL-2.0-or-later program is fine — they are separate programs communicating over a command line. Do not link them. |
| [ffmpeg](https://ffmpeg.org/) | muxing, post-processing invoked by yt-dlp | LGPL-2.1-or-later by default; GPL-2.0-or-later for builds with GPL components | check which build you redistribute; the "GPL" builds are GPL-2.0-or-later |
| [deno](https://github.com/denoland/deno) | JavaScript runtime for yt-dlp | MIT | — |
| [quickjs-ng](https://github.com/quickjs-ng/quickjs) | JavaScript runtime, alternative to deno | MIT | — |
| [Python](https://www.python.org/) | interpreter for Python-based backends | PSF-2.0 | — |
| bsdtar / unzip / tar | extracting downloaded backend archives | BSD-2-Clause (libarchive) / Info-ZIP / GPL-3.0 | used only via the system, never bundled; GNU tar is GPL-3.0 |

Two points that matter legally and are easy to get wrong:

1. **Morid does not link GPL-3.0 code.** It starts wget and tar as separate processes. If a future change
   linked a GPL-3.0 library into the binary, the result would have to be GPL-3.0, which conflicts with
   GPL-2.0-**only** distributions of the combined work. That is a decision, not a refactor.
2. **Some ffmpeg builds are GPL.** If an installer downloads a GPL ffmpeg build and ships it in the same
   archive as Morid, the packager is distributing a GPL-2.0-or-later binary next to a GPL-2.0-or-later
   program, which is compatible; but the ffmpeg build's own terms (including its source-offer obligations)
   still apply to it.

## 4. Icons and artwork

The icons in `legacy/src/icons/` (black and white variants) and the application icons ship with the original
project under GPL-2.0-or-later. They are reused in Morid with the copyright notice retained. If an icon turns
out to have a different provenance, it must be replaced before release.

## 5. Build-time tooling

Not distributed, listed for completeness: Rust toolchain (MIT OR Apache-2.0), Cargo (MIT OR Apache-2.0),
`slint-build` (same licence as Slint), CI runners provided by GitHub.

## 6. Maintaining this file

A pull request that adds a dependency, a bundled tool, an icon or a font updates this file in the same commit
([CONTRIBUTING.md](CONTRIBUTING.md) §3). A release is not made while a distributed component is missing from
this list.
