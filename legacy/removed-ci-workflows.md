# Inherited CI workflows

The ten workflows removed from `.github/workflows/` in Phase 1 are listed here with what each of them did, so that
nothing about the original project's release machinery is lost when the YAML is deleted.

Removed on 4 November 2026. None of them ran a test. All of them built artefacts and published them by
`repository-dispatch` into repositories that do not exist for this fork (`mhogomchungu/media-downloader-git` and
the upstream release page), using secrets this fork does not have. They could not run here, and ten permanently
red jobs would have made the real CI signal unreadable.

The replacement is a single CI workflow covering three platforms — see
[../docs/testing.md](../docs/testing.md) §7 and [../docs/roadmap.md](../docs/roadmap.md) Phase 1. The build and
packaging knowledge that is still useful lives in [../docs/building.md](../docs/building.md) and
[../docs/releases.md](../docs/releases.md).

| Workflow | Platform | What it did |
| --- | --- | --- |
| `LinuxQt6Git.yml` | Linux x64, Qt6 | built the program and uploaded the artefact |
| `PublishHereMacOsArmRelease.yml` | macOS arm64 | built and published a release in this repository |
| `PublishHereMacOsIntelRelease.yml` | macOS x86_64 | built and published a release in this repository |
| `PublishHereWindowsRelease.yml` | Windows | built and published a release in this repository |
| `PublishThereMacOsArmGit.yml` | macOS arm64 | built in `media-downloader-git` |
| `PublishThereMacOsArmRelease.yml` | macOS arm64 | built and published a release in `media-downloader-git` |
| `PublishThereMacOsIntelGit.yml` | macOS x86_64 | built in `media-downloader-git` |
| `PublishThereMacOsIntelRelease.yml` | macOS x86_64 | built and published a release in `media-downloader-git` |
| `PublishThereWindowsGit.yml` | Windows | built in `media-downloader-git` |
| `PublishThereWindowsRelease.yml` | Windows | built and published a release in `media-downloader-git` |

## Building the C++ reference from `legacy/`

The scripts and the CMake project moved with the sources. Path references that the move broke, and how to work
around them without editing the original:

- `legacy/CMakeLists.txt` uses `${CMAKE_CURRENT_SOURCE_DIR}` throughout, so it works unchanged from the new
  location.
- `legacy/CMakeLists.txt:446` copies `changelog` into the macOS bundle. That file is now `../CHANGELOG-legacy.md`.
  A bundle build therefore fails at the copy step unless the file is placed beside the build, or the line is
  pointed at `../CHANGELOG-legacy.md` for a local build. **Do not commit such a change**; it is a local
  workaround for a reference build only.
- `legacy/build_windows.ps1` derives `$SRC_LOCATION` from `$PSScriptRoot` and writes into `$SRC_LOCATION/BUILD`,
  so it produces its output inside `legacy/` — which `.gitignore` does not cover. Expected output paths are
  `legacy/BUILD/`. Add nothing to `.gitignore` for it: the artefacts must not be committed, and an ignore rule
  for a build directory of a reference tree is clutter.
- `legacy/build_linux-qt5.sh` and the two distribution scripts `cd` to their own directory, so they run from
  `legacy/` without change.

None of this is part of the product build. The Rust workspace has no dependency on Qt and CI never builds this
tree.
