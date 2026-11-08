# Workspace manifest — target content

**Status: not created.** This file records the exact content the workspace manifests must have when Phase 1 is
done ([roadmap.md](roadmap.md) §"Phase 1"). It exists so that the coder copies a reviewed definition instead of
inventing one, and so that review of the Phase 1 pull request is a comparison against a written spec.

Nothing in this file is built or tested yet. If a later decision changes a value here, the change is made here
first, in the same pull request that changes `Cargo.toml`.

---

## 1. `Cargo.toml` (workspace root)

```toml
[workspace]
resolver = "2"
members = [
    "crates/morid-core",
    "crates/morid-ext",
    "crates/morid-engine",
    "crates/morid-store",
    "crates/morid-net",
    "crates/morid",
    "crates/morid-cli",
    "crates/morid-gui",
]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.85"
license = "GPL-2.0-or-later"
authors = ["Rudolf Zinovev"]
repository = "https://github.com/RUdolf1517/Morid"

[workspace.dependencies]
morid-core = { path = "crates/morid-core" }
morid-ext = { path = "crates/morid-ext" }
morid-engine = { path = "crates/morid-engine" }
morid-store = { path = "crates/morid-store" }
morid-net = { path = "crates/morid-net" }
# `morid` is the application crate both binaries depend on. It was missing from this list,
# so `morid = { workspace = true }` in morid-cli and morid-gui could not resolve — the first
# thing the Phase 1 branch proved, by failing to load the manifest. `morid-gui` is
# deliberately absent: nothing may depend on it (§1, "Must not contain").
morid = { path = "crates/morid" }
serde = { version = "1", default-features = false, features = ["derive", "std"] }
serde_json = { version = "1", default-features = false, features = ["std"] }
sha2 = { version = "0.10", default-features = false }
ureq = { version = "3", default-features = false, features = ["rustls", "gzip"] }
tar = { version = "0.4", default-features = false }
flate2 = { version = "1", default-features = false, features = ["rust_backend"] }
zip = { version = "4", default-features = false, features = ["deflate"] }
slint = { version = "1", default-features = false, features = [
    "std",
    "compat-1-2",
    "renderer-femtovg",
    "backend-winit",
    "std-widgets",
] }

[profile.release]
opt-level = "s"
lto = "fat"
codegen-units = 1
panic = "abort"
strip = "symbols"

[profile.release.package."*"]
opt-level = "s"
```

Notes, each with its reason:

- **`resolver = "2"`** — edition 2021 default, stated explicitly so it cannot drift.
- **`rust-version = "1.85"`** — the minimum forced by the current Slint release, not a preference; recorded in
  [dependencies.md](dependencies.md) §1.
- **`version = "0.1.0"`** — Morid does not continue the original's `5.x` ([releases.md](releases.md) §1).
- **`license = "GPL-2.0-or-later"`** — matches `LICENSE`; the Cargo field is a statement about the crate, the
  licence file is normative ([LICENSES.md](../LICENSES.md)).
- **Two binaries** — `morid-cli` (console, [cli.md](cli.md)) and `morid-gui` (Slint window) are separate
  artefacts so the console binary does not link Slint (D-009).
- **Every dependency `default-features = false`** — the rule from [dependencies.md](dependencies.md) §3; a crate
  that turns on a default feature is reviewed again.
- **`opt-level = "s"` not `"z"`** — the measured compromise; `"z"` is tried only if the size report shows the
  release is far above the expectation, and the result is recorded in the size table.
- **`panic = "abort"`** — stated cost in [building.md](building.md) §3: no unwinding, no recovery from a panic.
- **`strip = "symbols"`** — symbols are shipped separately if a crash report ever needs them; they are not in the
  binary.
- **No `[workspace.dependencies]` entry for `libc` / `windows-sys`** — those are target-specific and stay in the
  `morid-engine` manifest with a `[target.'cfg(...)'.dependencies]` section.

---

## 2. Per-crate manifest shape

Every crate manifest is minimal and inherits from the workspace:

```toml
[package]
name = "morid-core"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true
authors.workspace = true
repository.workspace = true
description = "Morid domain types; no I/O"
```

Rules:

- no `build.rs` anywhere unless a decision records why (currently none is planned);
- no crate depends on `morid-gui` or on `slint`;
- `morid-core` depends on no other workspace crate — that is what makes it testable in isolation.

---

## 3. `rust-toolchain.toml`

```toml
[toolchain]
channel = "1.85"
components = ["rustfmt", "clippy"]
profile = "minimal"
```

Pinning the minimum, not the newest: a build with the newest stable is also allowed and is what CI uses for the
"latest" leg ([testing.md](testing.md) §7). Pinning the minimum here means an accidental use of a newer feature
fails locally rather than in a user's build.

---

## 4. `deny.toml`

```toml
[licenses]
version = 2
confidence-threshold = 0.9
allow = [
    "MIT",
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "ISC",
    "Zlib",
    "Unlicense",
    "MPL-2.0",
]

[[licenses.exceptions]]
name = "slint"
allow = ["GPL-3.0-only", "LicenseRef-Slint-Royalty-free-2.0", "LicenseRef-Slint-Software-3.0"]

[bans]
multiple-versions = "warn"
wildcards = "deny"

[sources]
unknown-registry = "deny"
unknown-git = "deny"
```

Notes:

- the allow-list is the one in [dependencies.md](dependencies.md) §5; additions are decisions, not pull-request
  edits;
- Slint is the single exception and it is explicit: the crate is triple-licensed, and Morid uses it under the
  royalty-free option with attribution ([D-001](decisions.md), [D-007](decisions.md)). The exception lists the
  identifiers `cargo deny` actually reports; it does not grant anything the decision did not;
- `unknown-git = "deny"` means no dependency may come from a git URL. A pin to a branch is how a project loses
  reproducibility;
- `multiple-versions = "warn"` rather than `deny`: two versions of a transitive crate are a size and audit
  question, not a build failure.

---

## 5. `.gitignore` additions

The C++ tree in `legacy/` builds in place, so the root `.gitignore` must cover:

```
/target
/legacy/build/
/legacy/BUILD/
/legacy/src/build/
*.qm
```

`legacy/BUILD/` is where `legacy/build_windows.ps1` writes; `legacy/build/` is where the CMake instructions in
[building.md](building.md) §7 write. `*.qm` was previously committed; it is now generated from the `.ts` sources
at build time, so a stale binary translation cannot be shipped silently. `Cargo.lock` is **committed**, deliberately
([dependencies.md](dependencies.md) §7).

---

## 6. What Phase 1 must not do

- Do not add a `[patch]` section, a `[replace]` section or a `paths` override.
- Do not enable `std`-less builds: `no_std` is not a goal and both binaries need the filesystem and a process
  spawner.
- Do not add `[features]` that gate whole subsystems. Optional backends are data
  ([architecture.md](architecture.md) §3), not build features. The only planned feature flags are
  `flatpak` (path overrides) and `portable` (data directory location).
- Do not copy the profile from another project without re-measuring; the size table in
  [dependencies.md](dependencies.md) §6 is the evidence.

---

## 7. Deviations recorded while Phase 1 was built

Each of these was forced by something the specification could not know, and each is in the same pull request as
the code that needed it. The values above have been edited to match, so this section is the record of *why*.

1. **`slint` is pinned to the 1.13 line and its feature list is different** (§1). Three separate facts, each from
   crates.io rather than from taste:
   - `version = "1"` resolves to 1.18.1, which is `edition = "2024"` and needs a newer rustc than the 1.85 the
     workspace pins. `~1.13` is the newest line whose `rust_version` is exactly `1.85`.
   - `std-widgets` is not a feature of `slint` 1.13.1 or 1.18.1 at all (`/api/v1/crates/slint/1.13.1`: 33
     features, no `std-widgets`); widgets are built in, and `import … from "std-widgets.slint"` compiles without
     it.
   - `compat-1-2` cannot be dropped: `slint` 1.13.1 refuses to compile without it
     (`error: The feature \`compat-1-2\` must be enabled`, `slint-1.13.1/lib.rs:207`).
   `backend-winit` and `renderer-femtovg` stay as written.
2. **`.cargo/config.toml` exists and is not mentioned above.** It sets
   `resolver.incompatible-rust-versions = "fallback"`. With `resolver = "2"` and the default `allow`, cargo resolved
   transitive crates needing 1.87–1.90 (`image 0.25.10`, `zbus 5.19`, `uuid 1.29`) and then refused to build with
   the pinned toolchain, which makes `rust-version = "1.85"` a lie. With `fallback`, resolution keeps the promise.
3. **`deny.toml` gained five things the specification did not have**, all recorded in the file itself:
   - `[advisories] unmaintained = "workspace"`. Without the section, `cargo deny check` denies every advisory,
     unmaintained ones included, and Slint's tree carries a growing set of them — `zvariant`, `zbus_names`,
     `zvariant_derive`, `ttf-parser`, `rustybuzz`, `pin-utils`, each marked "no safe upgrade" inside Slint 1.13.
     The policy states that unmaintained advisories are reported for workspace crates only; vulnerabilities still
     fail, and `cargo audit` in the next step still prints every unmaintained crate it finds, as a warning.
   - `GPL-2.0-or-later` in the allow list — Morid's own licence, which the list forgot: `cargo deny` rejected
     `morid 0.1.0`, `morid-cli 0.1.0` and the other six.
   - `BSL-1.0` (`clipboard-win`, through `copypasta`) and `Unicode-3.0` (the `icu_*` crates) — licences that only
     appear in Slint's transitive tree, both permissive and GPL-compatible.
   - an exception entry for **every** Slint crate rather than only `slint`: they all declare the same triple
     licence and an exception is matched by crate name, so the single `slint` entry left nine violations.
   - `[bans] allow-wildcard-paths = true` — every workspace dependency is written `{ path = "crates/…" }` with no
     version, which `wildcards = "deny"` rejects ("found 10 wildcard dependencies for crate \'morid\'"). The flag
     exempts path dependencies only, and only for crates that are not publishable (see the next item).
4. **`xtask` is its own workspace.** `xtask/Cargo.toml` carries an empty `[workspace]` table, so it is not one of
   the eight members and `cargo build --workspace` does not build it. Including it would have made nine members and
   added a second dependency tree; the workflow formats and lints it explicitly instead.
5. **`morid-gui/src/main.rs` uses `#![deny(unsafe_code)]`, not `forbid`.** `slint!` expands to Slint's generated
   code, which carries `#[allow(unsafe_code)]`, and an inner `allow` under a crate-level `forbid` is error E0453.
   The library crates keep `forbid`; Morid's own code in the binary contains no `unsafe`.
6. **`publish = false`** is set in `[workspace.package]` and inherited by every crate. Morid is an application;
   nothing here goes to a registry, and saying so is what makes `allow-wildcard-paths` apply — `cargo deny` refuses
   the exemption for publishable crates because "crates.io disallows path dependencies".
7. **`morid-gui` has one test hook.** If `MORID_GUI_SMOKE_MS` is set, the window runs the Close button's own code
   path after that many milliseconds, which is how CI proves that a window opens and closes with a real exit code
   on three platforms. Unset, nothing changes.
