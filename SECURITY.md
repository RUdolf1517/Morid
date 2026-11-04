# Security Policy

This document explains how to report a security problem in Morid. It is not a claim that the software is
free of vulnerabilities, and it does not replace the documents linked at the end.

## Supported versions

Security reports are investigated against the **latest published release** and the current **`main` branch**.
Fixes are developed on `main` and delivered in a new release. Backports to older releases are not
guaranteed; upgrade to a release that contains the fix.

**There is no released version yet** — see [docs/roadmap.md](docs/roadmap.md). Until the first
release, only `main` is investigated. The C++ code in `legacy/` is a reference copy of
[Media Downloader](https://github.com/mhogomchungu/media-downloader); it is not our product and its defects
should be reported to the upstream project. If a defect in `legacy/` also affects Morid's design, we want to
hear about it.

Include the release version or source commit in your report so the code can be identified unambiguously.

## Report a vulnerability

**Do not disclose an unpatched vulnerability in a public issue, pull request or discussion.**

Use either of these private channels:

1. **Preferred:** open the repository's [Security tab](https://github.com/RUdolf1517/Morid/security) and
   choose **Report a vulnerability** to submit a private report through GitHub.
2. **Fallback:** email [rudolfzinovev@protonmail.com](mailto:rudolfzinovev@protonmail.com).

You do not need to use both channels. Do not include sensitive personal data, cookies, session tokens or
private media unless they are essential to the investigation; a minimal, non-sensitive reproducer is
preferred.

## Scope

Reports about any feature of Morid are welcome. Morid is a frontend that runs other programs, so its
distinctive risks are about **what it runs and what it writes**, not about parsing media itself.

Security-relevant examples:

- **command injection or argument injection** — a URL, title, playlist name, cookie value, proxy string or
  option taken from the user (or from a remote response) reaching a backend as something other than one
  argument; a filename from a backend escaping the download directory;
- **arbitrary file write or overwrite** — writing outside the download folder, following a symlink out of it,
  or overwriting an unrelated file;
- **unsafe handling of downloaded binaries** — installing or updating a backend without checking the size
  and SHA-256 published by the source, or executing something from a path the user did not choose;
- **path traversal in archive extraction** — an extension archive unpacking outside its target directory;
- **credential leakage** — cookies, passwords or proxy credentials appearing in logs, the download history,
  crash reports or the console output that `morid` prints;
- **IPC trust** — the single-instance socket accepting events (for example a URL to download automatically)
  from a process that is not the same user;
- **resource exhaustion** — a crafted remote response causing memory or disk use out of proportion to the
  documented limits;
- **dependency compromise** — a build input that is fetched without integrity verification.

Ordinary compatibility bugs, a backend failing to download from a site, and documented limitations are not
vulnerabilities. A way to *bypass* a documented safeguard may be one.

## What to include

As much of the following as is available:

- the Morid version or commit, operating system, architecture and how it was installed;
- the exact command or UI steps, including options and the backend in use;
- what you expected, what happened, and what the security impact is;
- verbatim output or logs with sensitive values removed;
- a minimal reproducer. If the issue involves a crafted URL or a crafted extension JSON, include it as
  text rather than as a downloaded binary.

Do not run a suspected destructive reproducer against valuable data. Use a disposable download directory.

## Research and coordinated disclosure

Good-faith research that respects these boundaries is welcome. Test only systems and accounts you own or are
authorised to test. Do not access or modify third-party data, disrupt services, or use more exploitation than
is necessary to demonstrate the issue.

Reports are handled through validation, remediation and coordinated disclosure with the reporter. Response
and resolution times depend on the problem; this policy promises no fixed response or fix time. We aim to
keep the reporter informed and to agree disclosure timing. Please keep details and proof-of-concept material
private until that coordinated disclosure.

## Related documentation

- [decisions.md](docs/decisions.md) — why Morid does not self-update (D-013), and why backends are
  refreshed only from their own projects (D-014).
- [extensions.md](docs/extensions.md) — the extension format is an input; what is trusted and what is
  validated.
- [architecture.md](docs/architecture.md) — where process spawning, path handling and IPC live.
- [testing.md](docs/testing.md) — which failure modes are covered by tests, and which are not.
