# Changelog

All notable changes to Morid are documented here. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project intends to follow
[Semantic Versioning](https://semver.org/spec/v2.0.0.html).

**Morid has not been released yet.** There is no Rust code in a usable state; the entries below record
decisions and planning, not a program you can run. See [docs/roadmap.md](docs/roadmap.md) for what is being
built and in what order.

The history of the original C++/Qt program that Morid reimplements — 71 releases from 1.0.0 to 5.6.8 — is kept
unmodified in `CHANGELOG-legacy.md` (created in Phase 1 when `changelog` is moved to the repository root; until
then it is [changelog](changelog)). It is the reference for what "same functionality"
means, and it is not part of Morid's own version history.

## [Unreleased]

### Added

- Audit of the original C++/Qt code base (`docs/audit-original.md`): repository structure, architecture,
  the 57-method engine base class, the 34-key extension schema, the download pipeline, the logger-as-progress-
  parser, the settings and data files, localisation, packaging, and the defects found along the way.
- Functional map (`docs/features.md`): every feature of the original with an ID, its location in the original,
  its dependencies, a migration priority and a checkable acceptance criterion.
- Architecture proposal for the Rust implementation (`docs/architecture.md`): crate boundaries, the event
  model, the job scheduler, the engine model that replaces the 57 virtual methods.
- Roadmap (`docs/roadmap.md`) and migration plan (`docs/migration.md`), phased so that behaviour is fixed
  before it is reimplemented.
- Extension contract (`docs/extensions.md`), user-data compatibility notes (`docs/data-compat.md`), console
  interface specification (`docs/cli.md`), test strategy (`docs/testing.md`), dependency policy
  (`docs/dependencies.md`), platform matrix (`docs/platforms.md`), build and release documentation.

### Decided

Fifteen product decisions, recorded with their reasoning in [`docs/decisions.md`](docs/decisions.md):

- **D-001** The interface is built with Slint; Qt is not part of the final architecture.
- **D-002** Right-to-left interface mirroring is deferred; Arabic text renders but the layout is not mirrored.
- **D-003** Full compatibility: all 17 bundled extensions and the original's settings and data files are
  supported, with a one-time import.
- **D-004** A `RequiredMinimumVersionOfMediaDownloader` of the `2.x` family is treated as satisfied, with a
  clear diagnostic when a backend still fails.
- **D-005** Windows 10 and 11, x86_64 only. Windows 7, 8 and 8.1 are not supported.
- **D-006** The project continues in this repository; the C++ code moves to `legacy/`.
- **D-007** Slint is used under its royalty-free licence with attribution; the project licence is unchanged.
- **D-008** Binary size is measured and published on every build, but does not gate the build.
- **D-009** There are two binaries: `morid` (console, documented, with compatibility promises) and `morid-gui`.
- **D-010** Extensions come only from the `extensions/` directory in this repository.
- **D-011** "Download profiles" are not introduced as a separate feature; presets, the URL manager and
  per-engine default options cover the need.
- **D-012** All documentation is in English.
- **D-013** Morid does not update itself; it only reports that a newer version exists.
- **D-014** Backend binaries are fetched from their own projects; application and extension endpoints belong
  to Morid.
- **D-015** `legacy/` stays in the repository permanently.

### Known limitations, stated up front

- No RTL layout mirroring (D-002).
- No Windows 7/8 support (D-005).
- No self-update (D-013).
- Binary size is not gated, so growth is reported rather than blocked (D-008).
