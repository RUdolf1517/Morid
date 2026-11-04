# `legacy/` — the C++ reference

This directory holds the original **Media Downloader** source tree (C++14 / Qt 5.15 or Qt 6), the program Morid
reimplements. It is here to be **read, built and compared against**. It is not part of the product, it is not
compiled into any Morid binary, and pull requests do not modify it.

Two decisions govern this directory:

- **D-006** — the rewrite happens in this repository, and the C++ original is kept in `legacy/` as the reference.
- **D-015** — `legacy/` stays in the repository permanently. It is not deleted when a phase completes, and no
  "remove the old code" milestone exists.

## What is inside

```
legacy/
├── src/            C++ sources, 37 .cpp / 35 .h / 8 .hpp, 42,910 lines
│   ├── engines/    13 engine implementations (yt-dlp, gallery-dl, you-get, …)
│   ├── utils/      small helpers: qprocess, single_instance, tableWidget, …
│   ├── flatpak/    Flatpak manifest and AppStream metainfo
│   └── *.ui        Qt Designer forms
├── translations/   18 .ts/.qm catalogues, 354 messages each
├── images/         icons and artwork
├── changelog       the original's history, 1.0.0 → 5.6.8 (also at ../CHANGELOG-legacy.md)
├── CMakeLists.txt, src/media-downloader.pro
├── build_linux-qt5.sh, build_arch-qt5.sh, build_fedora-qt5.sh, build_windows.ps1
└── cmake_uninstall.cmake.in
```

Three things that were part of the original tree do **not** move here:

| Item | Where it goes | Why |
| --- | --- | --- |
| `extensions/` (17 JSON files) | repository root | an active contract that Morid loads, not a historical artefact — D-003, D-010 |
| `LICENSE`, `LICENSE.txt`, `GPLv2`, `GPLv3` | repository root | the project's licence and the original author's copyright notice, not an artefact of the C++ program |
| `changelog` | `CHANGELOG-legacy.md` in the root | the original's release history, kept unedited next to Morid's own `CHANGELOG.md` |

The licence files remain in the repository root: they are the project's licence, not an artefact of the C++
program, and the copyright notices in them must not be moved or edited.

## Rules

1. **Read-only.** The only changes accepted in `legacy/` are mechanical ones that a decision requires — moving
   the tree in, or repointing a path that CI and documentation use. Behaviour is never "fixed" here; if the
   original is wrong, the defect is recorded in [../docs/migration.md](../docs/migration.md) §6 and the C++
   file is left as it is.
2. **No formatting sweeps.** A diff in `legacy/` should be small enough to review line by line, and usually it
   should be empty.
3. **No new features.** Fixing a bug in the C++ program means maintaining two programs. The original is a
   reference; when it stops being able to answer a question, say so and record the answer in the new code's
   tests.
4. **Copyright notices stay.** The GPL requires it and the project promised it. See
   [../CONTRIBUTING.md](../CONTRIBUTING.md) §9.

## What it is used for

| Use | Where it is described |
| --- | --- |
| Recording expected argv and backend output as test fixtures | [../docs/migration.md](../docs/migration.md) §3, [../docs/testing.md](../docs/testing.md) §4 |
| Determining the exact behaviour of an extension key | [../docs/extensions.md](../docs/extensions.md) |
| Determining the format of `settings.ini` and the JSON data files | [../docs/data-compat.md](../docs/data-compat.md) |
| Enumerating what the user can do at all | [../docs/features.md](../docs/features.md) |
| Checking a disagreement about behaviour during code review | every review of a Rust change that claims parity |

## Building it

Instructions, including the Qt version choice and the platform quirks: [../docs/building.md](../docs/building.md)
§7. A build is needed only to capture new fixtures or to check a behaviour that reading the code cannot settle.
Producing a C++ binary that users run is not a goal of this repository.

No C++ toolchain is required to build Morid. Qt is never a dependency of the Rust workspace, and CI does not
build `legacy/`.

## What this directory is not

- It is not a fallback: if a Morid phase is late, the answer is not "ship the old program".
- It is not the source of truth for the *product*. The decisions in
  [../docs/decisions.md](../docs/decisions.md) override it wherever they disagree — RTL mirroring (D-002),
  Windows 7 (D-005), self-updating (D-013), network endpoints (D-014) and download profiles (D-011) are all
  places where Morid deliberately differs.
- It is not a licence exemption. Code copied from here into a Rust crate is still GPL-2.0-or-later and still
  carries the original author's copyright.
