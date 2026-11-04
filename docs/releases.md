# Releases

Related: [building](building.md) · [platforms](platforms.md) · [CHANGELOG](../CHANGELOG.md) ·
[decisions](decisions.md) D-008, D-013

**No release has been made.** This document describes how one will be made, so that the first one is not invented
under pressure.

---

## 1. Versioning

The project intends to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html), with one addition that
matters more than the version number: **the console interface and the extension format are compatibility
surfaces**, and neither is allowed to break in a minor release.

| Change | Version |
| --- | --- |
| A new feature, a new extension key, a new console command | minor |
| Bug fix, translation update, documentation | patch |
| Required argument change, removed field in `--json`, removed extension key, changed exit code | **major** |
| Anything that changes the argv generated for an existing input | reviewed as a compatibility break; major unless shown to fix a defect |

Morid's own numbering restarts at `0.1.0`; it does not continue the original's `5.x`. The original's history is
kept in `CHANGELOG-legacy.md` (today still the file `[changelog](../changelog)` in the repository root) and is
not Morid's history. Confusing the two would make
"Morid 6.0 is older than Media Downloader 5.6.8" a conversation somebody has to have.

---

## 2. Release gates

A release is made only when all of the following hold. Each is a check somebody performs, not a wish.

| # | Gate | Evidence |
| --- | --- | --- |
| 1 | Every P0 feature from [features](features.md) is `done` | the status column; no exceptions |
| 2 | CI green on Linux, macOS and Windows | the run attached to the tag |
| 3 | `cargo deny check` and `cargo audit` clean, or every advisory has a recorded decision | CI output |
| 4 | Size measured and recorded in [dependencies](dependencies.md) §6, growth explained | the release pull request |
| 5 | The round-trip data test passes: original data → import → export → the original program reads it | [data-compat](data-compat.md) §6 |
| 6 | Manual procedures M1–M7 performed and recorded | [testing](testing.md) §6 |
| 7 | `CHANGELOG.md` written by hand, in categories, with every decision that changed behaviour | the file |
| 8 | `THIRD-PARTY.md` matches what actually ships | the file |
| 9 | Artefacts checksummed, checksums published, install instructions verified against a clean machine | `SHA256SUMS` |
| 10 | No P3 row silently implemented; every documented gap still documented | [features](features.md) §8 |

Gate 5 and gate 10 exist because both are the kind of thing that quietly stops being true.

---

## 3. Artefacts

| File | Platform | Contents |
| --- | --- | --- |
| `morid_<ver>_linux_x86_64.tar.gz` | Linux x86_64 | `morid`, `morid-gui`, `extensions/`, `docs/`, licences |
| `morid_<ver>_linux_aarch64.tar.gz` | Linux aarch64 | same |
| `morid_<ver>_amd64.deb` | Debian/Ubuntu | same, installed under `/usr` |
| `io.github.rudolf1517.Morid.flatpak` | Linux | sandboxed build |
| `Morid-<ver>-arm64.dmg` | macOS arm64 | `.app` bundle |
| `Morid-<ver>-x86_64.dmg` | macOS x86_64 | `.app` bundle |
| `Morid-<ver>-setup.exe` | Windows 10/11 x86_64 | Inno Setup installer, per-user |
| `Morid-<ver>-portable.zip` | Windows 10/11 x86_64 | data beside the executable |
| `SHA256SUMS` | all | checksums for every file above |

No 32-bit artefact and no Windows 7 build, on purpose ([D-005](decisions.md)). Their absence is a decision, and
the release notes say so rather than leaving users to infer it.

A `.tar.gz` release also contains a `morid` directory with a `README` explaining that the backends are not
included and that yt-dlp will be offered for download on first run.

---

## 4. The release procedure

```
1. freeze: no feature merges; only fixes
2. write CHANGELOG.md for the release; move [Unreleased] content
3. bump the version in Cargo.toml (workspace) and in the packaging metadata
4. run the gates of §2 on a clean checkout
5. tag: git tag -a v<ver> -m "Morid <ver>"; push the tag
6. CI builds the artefacts on all supported runners and attaches them to the release
7. compute SHA256SUMS, publish them in the release body and as a file
8. write the release notes: what changed, what broke, what is still missing, what to do if you
   came from Media Downloader (import instructions)
9. announce: repository release, and the packaging channels (Flatpak, AUR, distributions)
```

Step 8 is not optional and is not "see the changelog". The release notes must state, in plain language:

- what to run to import an existing Media Downloader installation;
- which limitations are new (RTL layout, Windows 7, no self-update) so that a user does not read them as
  regressions;
- whether the binaries are signed or notarised, and what to do if the operating system objects.

---

## 5. What a user coming from Media Downloader experiences

This is the most important paragraph in the document, because the first release will be judged on it.

1. Both programs can coexist. Morid uses its own data directory and never writes into the original's.
2. On first run, Morid offers to import: `morid import <original-data-dir>` or Configure → Import. The report
   lists what was imported, what was skipped, and which settings were dropped because the feature is not carried
   over (self-update settings, per [D-013](decisions.md)).
3. The download folder, presets, URL rules, per-engine defaults, subscriptions, archive and history come across.
   The archive is what prevents everything being downloaded again — it is the single most important item in the
   import.
4. Backend binaries are already on disk in the original's data directory and are reused, not re-downloaded.
5. The original program keeps working if the user goes back: nothing was moved or altered.

If any of these five steps is false in practice, the release is not ready, regardless of what the gates say.

---

## 6. Open release decisions

| # | Decision | Blocks | Default if nobody decides |
| --- | --- | --- | --- |
| R1 | Code signing for Windows (cost, certificate) | a clean Windows experience; SmartScreen warnings otherwise | ship unsigned and say so plainly in the release notes |
| R2 | macOS notarisation (Apple developer account, cost) | Gatekeeper refusing the bundle | ship unsigned and document the workaround, as the original does |
| R3 | Distribution channels beyond GitHub releases (Flatpak on Flathub, AUR, Fedora) | reach | GitHub releases only for the first version |
| R4 | Whether the installer fetches backends on first run or only offers instructions | first-run experience | offer instructions, plus a button that downloads with verification |
| R5 | Licence file set: whether `GPLv2`/`GPLv3` texts stay alongside `LICENSE` | packaging clarity (audit defect D7) | keep all four files; add a `LICENSES.md` note explaining which is normative |

---

## 7. Rollback

If a release turns out to be defective:

- the release is marked as pre-release or `yank`ed from package channels, not deleted;
- the reason is written into `CHANGELOG.md` under the version, so the next reader knows;
- a fixed version is released; **downgrade instructions are published** for the console binary, because a user
  whose import produced bad data needs to know what state they are in;
- if the defect touched user data (an import that lost entries, an archive that was rewritten), it is treated as
  a critical bug: a reproduction is required, and the fix ships with a test that fails on the defective version.
