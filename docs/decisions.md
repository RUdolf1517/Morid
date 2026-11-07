# Decisions

The log of product decisions taken by the project owner, Rudolf Zinovev. Each entry records the decision, the
date, the reasoning and its consequences.

**This file is the source of truth for the project.** When a task, a document or a line of code disagrees with
an entry here, the entry wins. Entries are never deleted; a decision that is superseded gets a new entry that
names the old one.

A Russian-language record of the same decisions, with the full argumentation written for a non-technical reader,
is kept in [`archive/ru/`](../archive/ru/) — see [D-012](#d-012-documentation-in-english).

---

## D-001. User interface toolkit: Slint

- **Date:** 4 November 2026 · **Question:** Q1 of the audit
- **Decision:** the interface is built with **Slint** — declared in `.slint` files, driven by Rust.
- **Reasoning:** Slint is the only Rust toolkit that matches the original's declarative UI model rather than
  requiring the interface to be drawn imperatively; it has a live preview, and its binaries are compact, which
  serves the minimalism principle. Qt through a C++ bridge was rejected because the point of the project is to
  leave C++ behind.
- **Consequences:** Qt is not part of the final architecture. `morid-gui` depends on `slint` and `slint-build`;
  `.slint` files live in `ui/`. Slint does not support interface direction, which forces D-002. The licence
  question is settled in D-007.

## D-002. Right-to-left interface mirroring is deferred

- **Date:** 4 November 2026 · **Question:** Q2
- **Decision:** the first version does **not** mirror the interface for right-to-left languages. Arabic stays a
  supported language: strings are translated and render correctly, but the layout stays left-to-right.
- **Reasoning:** Slint has an open upstream request for interface direction with no date. Implementing mirroring
  ourselves is estimated at 20–40 days and must be maintained for every new screen. One language out of eighteen
  must not hold back the program for the other seventeen.
- **Consequences:** the limitation is stated **publicly** — in the README, in `CHANGELOG.md`, and in the About
  text — so a user does not read it as a translation defect. It is on the roadmap with no release attached. The
  test criterion is that Arabic text is correct and unclipped, not that the layout mirrors. Revisit if Slint adds
  the feature or if users ask.

## D-003. Full compatibility with extensions and user data

- **Date:** 4 November 2026 · **Question:** Q5
- **Decision:** Morid is a **replacement** for Media Downloader, not a new program with a similar function.
  - All 17 bundled extension files load and work.
  - User data is read and imported once: `settings.ini`, `presetOptions.json`, `downloadDefaultOptions.json`,
    `downloadEngineDefaultOptions.json`, `subscriptions.json`, `autoSavedList.json`, `downloadHistory.json`,
    `archiveFile-*`, `themes/`.
  - The extension schema becomes a **public contract with a version number** and is covered by tests.
- **Reasoning:** preserving the original's functionality is the purpose of the project; without compatibility
  no existing user can move.
- **Consequences:** two documents that did not exist in the plan — [extensions](extensions.md) and
  [data-compat](data-compat.md). Discrepancies found in the audit are **not fixed silently**: each is decided in
  `docs/extensions.md` (the `ExtraArguments*` case) or `docs/migration.md` §6. Import becomes its own phase with
  a test against a real data directory. The upstream extension download by pinned commit
  (`legacy/src/configure.cpp:876`) is not carried over — see D-010.

## D-004. Extension version requirements: soft check

- **Date:** 4 November 2026 · **Question:** Q6 (decided as a consequence of D-003)
- **Decision:** a `RequiredMinimumVersionOfMediaDownloader` in the `2.x` family is treated as **satisfied**. A
  higher requirement refuses the extension with a message naming the required and the running version.
- **Reasoning:** every existing extension asks for `2.x`. Once Morid has its own numbering, a strict comparison
  would reject all of them, including ones that work.
- **Consequences:** the mapping table lives in [extensions](extensions.md) §1. Nothing is silently ignored: the
  extension loads, and a real failure is reported with a clear diagnosis.

## D-005. Windows: 10 and 11 only

- **Date:** 4 November 2026 · **Question:** Q7
- **Decision:** support is limited to **Windows 10 and 11, x86_64**. Windows 7, 8 and 8.1 are not supported. No
  32-bit Windows build is produced.
- **Reasoning:** old-Windows support freezes the toolchain, forbids crates that dropped it, requires a 32-bit
  build and testing on an unpatched system — a permanent tax on every later choice. Windows 7 has had no security
  updates since 2020.
- **Consequences:** [platforms](platforms.md) records the matrix and the reason. The roadmap records that support
  is not planned, so it does not resurface as forgotten debt. Not carried over: `win7amd64`/`win7x86` command
  selection, `yt-dlp_win7.exe` variants, the quickjs-ng pinning done for Windows 7 (changelog 5.6.2, 5.6.8).
  CI matrix: Linux x86_64 + aarch64, macOS arm64 + x86_64, Windows x86_64.

## D-006. Repository: this fork, C++ in `legacy/`

- **Date:** 4 November 2026 · **Question:** Q11
- **Decision:** the project continues in the current repository. The existing C++ code moves to `legacy/` and is
  kept as the behavioural reference.
- **Reasoning:** one link to the project, the whole transition in one history, and an easy "before and after"
  story.
- **Consequences:** `legacy/` contains `src/`, `CMakeLists.txt`, the build scripts, `src/flatpak/`,
  `translations/`. **Not** moved: `extensions/` (an active contract, D-003), the licence and copyright files,
  `images/`. `changelog` becomes `CHANGELOG-legacy.md` without edits; the new `CHANGELOG.md` starts with Morid.
  The root builds with Cargo only. The 10 inherited CI workflows are removed — they need secrets and repositories
  from the upstream project and none of them runs a test; new CI is Rust. GitHub still labels the repository a
  fork; that is accepted.

## D-007. Slint licence: royalty-free with attribution

- **Date:** 4 November 2026 · **Question:** Q3
- **Decision:** Slint is used under its **free licence with attribution** — not the GPLv3 option, not the
  commercial one.
- **Reasoning:** the project licence does not change (GPL-2.0-or-later) and the obligation is a line of
  attribution.
- **Consequences:** the About window names Slint; `THIRD-PARTY.md` records version, licence and the required
  attribution; `docs/dependencies.md` §2.1 records the reasoning. Changing this later requires a licence review
  and a new entry.

## D-008. Binary size: measured, not gated

- **Date:** 4 November 2026 · **Question:** Q12
- **Decision:** the size of every build is **measured and published automatically**, but a large size does not
  fail the build. Targets, not gates: GUI ≤ 20 MiB, console ≤ 4 MiB.
- **Reasoning:** the owner chose the softer mode so that early phases, where the dependency set is still moving,
  are not blocked.
- **Consequences and a stated risk:**
  - required: the size appears in every task report and in `CHANGELOG.md` when it moves by more than 5 %; the
    history is kept in [dependencies](dependencies.md) §6;
  - a growth of more than 10 % over the previous measurement requires a written justification in the pull
    request;
  - **the risk is accepted knowingly:** without a gate, minimalism stays a declaration. The technical lead
    proposes revisiting gates at version 1.0, when the dependency set has settled — either to enable them or to
    record the refusal;
  - size measurement is part of Phase 1 CI, so this decision blocks nothing.

## D-009. Console interface is a product feature

- **Date:** 4 November 2026 · **Question:** Q13
- **Decision:** Morid ships **two binaries**: `morid` (console, documented, with compatibility promises for
  commands, exit codes and the JSON stream) and `morid-gui` (window).
- **Reasoning:** without a console interface the behaviour cannot be tested — the tests would be tied to the
  window, which is exactly why the original has none. It is also a second audience: scripts, schedulers,
  servers.
- **Consequences:** [cli](cli.md) is a specification with stable exit codes and a versioned JSON stream. The core
  is designed as a library and the window is one consumer. Commands: `list`, `download`, `engines`, `import`,
  `export`, `gui`, plus `--dry-run`, which prints the argument vector and never spawns a process — that is the
  main compatibility-testing tool.

## D-010. Extension source: the `extensions/` directory in this repository

- **Date:** 4 November 2026 · **Question:** Q14
- **Decision:** the only source of extension descriptions is `extensions/` in this repository. Nothing is
  downloaded from a URL.
- **Reasoning:** works without a network, does not depend on somebody else's repository or a pinned commit, and
  keeps the contract of D-003 under our control.
- **Consequences:** `configure::setUrl` with the pinned upstream hash is not carried over. Updating a
  description requires a Morid release; accepted, because schema changes are rare. The mismatch between 17 files
  on disk and 10 in the menu (audit defect D5) is resolved in [extensions](extensions.md) §7. Automatic updating
  of the backend **binaries** stays — that is about the tools, not about descriptions (see D-014).

## D-011. "Download profiles" are not introduced

- **Date:** 4 November 2026 · **Question:** Q9
- **Decision:** there is no separate "download profile" entity. The need is covered by three existing
  mechanisms: presets, the URL manager, and per-engine default options.
- **Reasoning:** the original never had the feature, so there is nothing to migrate; inventing it now delays
  parity.
- **Consequences:** [features](features.md) F-53 is marked as covered rather than as a feature, so the map does
  not promise something that does not exist. Named profiles appear in the roadmap as a possible improvement
  after parity, with no release attached.

## D-012. Documentation in English

- **Date:** 4 November 2026 · **Question:** Q10
- **Decision:** **all documentation is written in English.** The technical lead's recommendation to split public
  English from internal Russian was not adopted.
- **Consequences:** README, SECURITY, CONTRIBUTING, CHANGELOG, THIRD-PARTY and everything in `docs/` is English.
  The three working documents written in Russian before this decision — the audit, the owner-facing explanation
  and this log — are available in English here (`docs/audit-original.md`, this file) and their Russian versions
  are kept in [`archive/ru/`](../archive/ru/) for the owner's reference, marked as archival. They are not
  maintained and must not be cited as current. Communication with the owner in chat may be in Russian; that is
  conversation, not documentation.

## D-013. Self-update replaced by a version notice

- **Date:** 4 November 2026 · **Question:** Q8
- **Decision:** Morid does **not** update itself. It checks for a newer version and says so, with a link.
- **Reasoning:** self-update produces a class of failure where the program stops starting for every user at once,
  and it widens the attack surface of the delivery channel, in exchange for convenience.
- **Consequences:** `startedUpdatedVersion` (`legacy/src/utility.cpp:1358`), which renames `update`/`update_new`
  directories at startup, is not carried over. The `WindowsUpdateChannel` and `CheckForUpdates` settings are
  imported but dropped, with a report ([data-compat](data-compat.md) §3). Updates arrive through package managers
  and the releases page. Automatic updating of **backend** binaries is unaffected (D-014).

## D-014. Network endpoints: theirs and ours

- **Date:** 4 November 2026 · **Question:** Q15
- **Decision:**
  - release endpoints of **third-party backends** (yt-dlp, gallery-dl, you-get, svtplay-dl, get-sauce, wget,
    aria2, ffmpeg, deno, quickjs-ng) stay the projects' own — the same ones the original uses;
  - endpoints of **Morid itself** (the "a newer version exists" check) are Morid's own;
  - extension **descriptions** have no endpoint at all (D-010).
- **Reasoning:** the backends are not ours to mirror, and the application update check is not theirs to serve.
  Leaving the original's application URL in place would make Morid offer to update itself into somebody else's
  program.
- **Consequences:** `settings::gitHubDownloadUrl` (`legacy/src/settings.cpp:353-372`), which points at Media
  Downloader releases, is replaced. Integrity checking of downloaded binaries — size and SHA-256 from the GitHub
  API — is preserved. The endpoint list is recorded in [extensions](extensions.md) §6 and
  [THIRD-PARTY](../THIRD-PARTY.md) §3 with the note that external sources are not under our control.

## D-015. `legacy/` stays permanently

- **Date:** 4 November 2026 · **Question:** Q4
- **Decision:** the `legacy/` directory is kept **indefinitely**. Removal is not planned.
- **Reasoning:** the source of truth for the original's behaviour should be permanently at hand, not only in the
  form of captured fixtures.
- **Consequences:** `legacy/README.md` states that the code is not a product, receives no features and no
  security fixes, and exists for comparison. [CONTRIBUTING](../CONTRIBUTING.md) §9 restricts changes inside it to
  what is strictly necessary to compare behaviour. The root builds with Cargo only; the C++ build is described
  separately in [building](building.md) §7. While `legacy/` exists, a behaviour change in Rust is checked against
  it before the change is accepted.

---

## Open questions

### D-016 — `settings.ini` is read and written the way the original does it

- **Date:** 5 November 2026 · **Question:** raised by the Phase 0 review, not by the audit
- **Decision:** **compatibility wins over correctness of encoding.** Morid reads the Qt-escaped INI dialect the
  reference program writes, and writes that dialect back. It does not "improve" the file to UTF-8.
- **Reasoning:** the owner's position is compatibility first, and here it is also the only choice that keeps the
  promise in D-003. A `settings.ini` containing a non-ASCII value is written by the original as ASCII with
  `\x<hex>` escapes (Qt 5 with no `setIniCodec`); a UTF-8 file written by Morid would be misread by the original
  as a different path, and the original would then reset the user's download folder. Writing UTF-8 would be a
  deliberate break dressed up as an improvement.
- **What it does not mean:** Morid does not reproduce *defects*. The Qt 5 → Qt 6 divergence is recorded so that a
  user who switches builds understands what happened, not imitated.
- **Consequences:**
  - the INI reader and writer are specified in [data-compat](data-compat.md) §3.0, including the `\x<hex>` rule,
    the digit-defence rule and the quoting rule — all three taken from Qt's own source, and all three silent when
    got wrong;
  - this is an *architecture* consequence, not a local one: `morid-store` cannot use a general-purpose INI
    crate for this file. It needs a reader/writer for Qt's dialect, written and tested in this repository
    ([dependencies](dependencies.md) §2 gains a rejection entry with this reason);
  - the round trip "Morid writes → the original reads → value intact" is an acceptance test, not a manual check;
  - a non-ASCII value must appear in the fixture set, so the rule is exercised rather than assumed.


**None.** All fifteen questions of the audit were decided on 4 November 2026. D-016 was raised afterwards, by
the Phase 0 review, and is decided.

| Question | Decision | Question | Decision |
| --- | --- | --- | --- |
| Q1 UI toolkit | D-001 Slint | Q9 download profiles | D-011 not introduced |
| Q2 RTL | D-002 deferred | Q10 documentation language | D-012 English |
| Q3 Slint licence | D-007 royalty-free + attribution | Q11 repository | D-006 this fork, `legacy/` |
| Q4 `legacy/` lifetime | D-015 permanent | Q12 binary size | D-008 measured, not gated |
| Q5 compatibility | D-003 full | Q13 console interface | D-009 product feature |
| Q6 extension versions | D-004 soft check | Q14 extension source | D-010 repository files |
| Q7 Windows | D-005 10/11 only | Q15 network endpoints | D-014 split |
| Q8 self-update | D-013 notice only | | |

Engineering choices that are not product decisions — thread model, HTTP client, translation format — are listed
in [architecture](architecture.md) §10 and [roadmap](roadmap.md) §5, and are settled in the phase that needs
them. **INI parsing is no longer in that list**: D-016 settled it, because the answer is dictated by
compatibility rather than by preference.

New questions are added to the table above with what they block and by when an answer is needed. An open
question stops only the work that depends on it, never the whole project.
