# Supported platforms

Related: [building](building.md) · [releases](releases.md) · [decisions](decisions.md) D-005 ·
[audit](audit-original.md) §2.12

## 1. The matrix

| Platform | Architecture | Status | Minimum version | Artefact |
| --- | --- | --- | --- | --- |
| Linux | x86_64 | **supported** | glibc 2.31 (Ubuntu 20.04 class) | `.tar.gz`, `.deb`, Flatpak |
| Linux | aarch64 | **supported** | glibc 2.31 | `.tar.gz` |
| Linux | x86 (32-bit) | not supported | — | — |
| macOS | arm64 (Apple silicon) | **supported** | macOS 12 | `.dmg`, `.app` bundle |
| macOS | x86_64 (Intel) | **supported** | macOS 12 | `.dmg`, `.app` bundle |
| Windows | x86_64 | **supported** | Windows 10 1809 | `.exe` installer, portable `.zip` |
| Windows | x86 (32-bit) | not supported | — | — |
| Windows | 7, 8, 8.1 | **not supported** | — | — |
| *BSD | any | not supported, not tested | — | — |

Two of these are decisions rather than observations and should be read as such:

- **Windows 7/8 are not supported** ([D-005](decisions.md)). Supporting them means freezing the toolchain,
  keeping a 32-bit build, avoiding crates that dropped old Windows, and testing on an unpatched system. That is
  a permanent tax on every later choice, not a one-off task. Users on those systems can stay on the C++ original,
  which continues to exist as their program.
- **32-bit builds are not produced.** Every user-facing reason to have one (old hardware, old Windows) is
  already outside the matrix. A 32-bit build that nobody tests is a support burden pretending to be a feature.

## 2. What each platform requires

### 2.1 Linux

- **Display:** X11 or Wayland. Slint's default backend uses whichever is available; there is no separate build.
- **System libraries:** `libc`, `libdl`, `libpthread` (usually implicit), and for the window: the usual X11/
  Wayland client libraries that Slint pulls in through `winit`. No Qt, no GTK.
- **No C compiler needed** to build, and no system OpenSSL: TLS is pure Rust
  ([dependencies](dependencies.md) §2.6).
- **Runtime dependencies:** none beyond libc, except a shell for the "run command after download" feature and an
  external player if the user configures one.
- **Flatpak:** supported ([D-005](decisions.md) is unrelated; the original's sandbox workarounds for deno leaking
  into every code path are *not* reproduced — see `docs/data-compat.md` and audit D9 for what was wrong with
  that).

### 2.2 macOS

- **Bundle:** `Morid.app` with `Info.plist`, `.icns`, and the binaries in `Contents/MacOS`.
- **Architectures:** separate arm64 and x86_64 builds. A universal binary is not produced initially: it doubles
  the size of the largest artefact for a benefit measured in a shrinking number of users. Revisit if ARM and
  Intel users both report it matters.
- **Notarisation:** unsigned bundles are reported as damaged by Gatekeeper on recent macOS. The original ships
  unsigned bundles and says so in its README. Morid should either notarise or document the same workaround
  honestly; this is a release-time decision listed in [releases](releases.md) §6.
- **File dialogs and the "open with" behaviour** go through the platform, not through a custom picker.

### 2.3 Windows

- **Installer:** Inno Setup, per-user install by default (`%LOCALAPPDATA%`), no administrator prompt for the
  portable variant.
- **Portable mode:** data beside the executable when `--portable` is passed or a marker file exists
  ([data-compat](data-compat.md) §1).
- **Process control:** cancellation uses Job Objects with `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`. The original's
  shelling out to `taskkill` is not reproduced ([architecture](architecture.md) §5).
- **Antivirus:** the original's README documents false positives from Windows Defender and others. A freshly
  compiled, unsigned Rust binary will attract the same attention. Mitigation is code signing, which is a cost
  and a decision for the owner; until then, the release notes must say plainly that the binary is unsigned.
- **Console:** `morid.exe` is a console subsystem binary; `morid-gui.exe` is a window subsystem binary, so
  starting the GUI does not open a console window.

## 3. Development platforms

CI runs on GitHub-hosted runners: `ubuntu-latest`, `macos-14`, `windows-2022`. Every pull request is built and
tested on all three; a change that only compiles on the author's machine is not merged.

| Check | Linux | macOS | Windows |
| --- | --- | --- | --- |
| `cargo fmt --check` | ×1 | — | — |
| `cargo clippy -D warnings` | × | × | × |
| `cargo test` | × | × | × |
| release build + size report | × | × | × |
| packaging smoke test | × | — | — |

Tests that are platform-specific (process-tree cancellation, job objects, path handling with non-UTF-8 names) are
written with `#[cfg]` and must run on their platform. A test that is skipped everywhere is not a test.

## 4. Feature differences that are not bugs

These are real platform differences, documented so they are not reported as defects:

| Behaviour | Linux | macOS | Windows |
| --- | --- | --- | --- |
| Tray icon | depends on the desktop environment; absent under some Wayland composers without a status notifier | menu bar item | notification area; may be hidden by default in Windows 11 |
| Notifications | via the desktop's notification daemon | via the notification centre | via toast notifications |
| Default player lookup | `xdg-open` | `open` | shell open; VLC if configured |
| Data directory | `$XDG_DATA_HOME` or `~/.local/share` | `~/Library/Application Support` | `%LOCALAPPDATA%` |
| "Run command after download" | `sh -c` | `sh -c` | `cmd /c` |

## 5. Build environment requirements

| Requirement | Why | What happens if missing |
| --- | --- | --- |
| Rust 1.85+ | `docs/dependencies.md` §1 | the build stops with the toolchain error |
| No C compiler | pure-Rust TLS and compression | — (this is a goal, not a dependency) |
| No network at build time | reproducible, offline builds | the build is considered broken |
| No system Qt, GTK, OpenSSL | Slint and rustls draw their own | — |

Cross-compilation is documented in [building](building.md) §5. The only cross-compilation target that is
actually supported is Linux x86_64 → Linux aarch64; macOS and Windows builds are made on their own runners.

## 6. Platforms that are explicitly out of scope

| Target | Why not |
| --- | --- |
| Windows 7 / 8 / 8.1 | D-005 |
| Windows on ARM | no test hardware, no user demand recorded, and every dependency would need an ARM build |
| Linux 32-bit | see §1 |
| Linux musl (static) | attractive for size, but the GUI needs system libraries; a static *console* build may be offered later, and would be documented here first |
| *BSD | no maintainer, no CI runner |
| Android / iOS | the tool runs other desktop programs; not a mobile application |
| Web (WASM) | the program spawns processes; a browser cannot |

A request to add a platform starts with a maintainer and a CI runner, not with a port.
