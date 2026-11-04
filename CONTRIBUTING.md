# Contributing

Thank you for considering a contribution. One rule outranks everything else here:

> **Morid must behave like the program it replaces, or say clearly why it does not.**

That is not a slogan. The whole reason this project exists is that a working C++/Qt application is being
reimplemented in Rust without losing what it does. A change that makes the Rust version nicer but different
is a regression until a decision says otherwise.

---

## 1. Before you write code

For anything larger than a typo:

1. Read [`docs/decisions.md`](docs/decisions.md). Fifteen product decisions are recorded there with reasons.
   They are not up for renegotiation in a pull request.
2. Read the functional map entry you are implementing in [`docs/features.md`](docs/features.md). Every feature
   has an ID (`F-01`, `F-31`, …), a priority and an acceptance criterion. Work that does not map to an ID is
   work that has not been agreed.
3. Check the phase in [`docs/roadmap.md`](docs/roadmap.md). Do not implement a later phase early, even if it
   looks easy: it usually depends on a decision that has not been made.
4. For anything touching the extension format, the data files or the console interface, the contract in
   [`docs/extensions.md`](docs/extensions.md), [`docs/data-compat.md`](docs/data-compat.md) or
   [`docs/cli.md`](docs/cli.md) changes in the same pull request.

No feature may be advertised anywhere — README, release notes, `--help` — before it works and its acceptance
criterion is covered by a test.

---

## 2. Development setup

```sh
# Rust 1.85 or newer (docs/dependencies.md §1)
cargo build
cargo test
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check
```

Optional tooling:

```sh
cargo install cargo-deny cargo-audit cargo-bloat --locked
cargo deny check        # licences and duplicate crates, see deny.toml
cargo audit             # known vulnerabilities
cargo bloat --release   # what the binary is made of
```

The build must work offline (`cargo build --offline`) and must not require a C compiler. If a dependency
breaks that rule, it is a decision to make with the project owner, not a workaround to commit.

To build the C++ reference program — needed only to compare behaviour — see
[`docs/building.md`](docs/building.md#7-building-the-c-reference).

---

## 3. What a pull request must contain

| Change | Required in the same pull request |
| --- | --- |
| Bug fix | A test that fails before the fix |
| New feature or migrated feature | The `F-` ID from `docs/features.md`, its acceptance criterion as a test, README/roadmap status update |
| Extension-format behaviour | `docs/extensions.md` update, a fixture for every schema version affected |
| Change to a data file format | `docs/data-compat.md` update, an import test against a real original file |
| Console output change | `docs/cli.md` update; a change in the output format is a breaking change and needs a version bump |
| New dependency | `docs/dependencies.md` entry answering the six questions, licence check, measured binary delta |
| Behaviour that differs from the original | A decision record in `docs/decisions.md`, agreed before the code is written |
| Anything user-visible | `CHANGELOG.md` entry |

Additionally:

- `cargo fmt` and `cargo clippy -D warnings` must be clean;
- CI must be green on Linux, macOS and Windows;
- the binary size is measured and reported, and a growth of more than 10 % is justified in the description
  ([D-008](docs/decisions.md));
- no new `unsafe` (the workspace forbids it);
- no dependency added to save lines of code.

---

## 4. Code style

- Idiomatic Rust, `rustfmt` defaults, edition 2021.
- Plain functions and small enums over trait objects, generics and abstraction layers. If a new module needs
  a diagram to explain, it is probably too complex for this project.
- Errors are one `enum Error` per crate with a `Display` implementation. No `anyhow` in library crates; the
  console binary may use it at the top level.
- No panics on input data. `unwrap`/`expect` are acceptable only for invariants that cannot fail, and must be
  commented as such.
- No `unsafe`. Ever. If a dependency needs it, that is a discussion, not a decision.
- Comments explain *why*; the code explains *what*.
- Keep the boundary between "domain" and "I/O" visible: a crate that parses bytes must not spawn processes,
  and a crate that spawns processes must not know about widgets. See
  [`docs/architecture.md`](docs/architecture.md).

### The lesson from the original

The C++ program has one base class with 57 virtual methods and a `Context` object that carries fifteen
references to everything. That is why it has no unit tests: no module can be constructed without the whole
application. Do not reproduce it. When you are tempted to add "just one more" virtual method or to pass the
whole application state into a function, that is the moment to split the type instead.

---

## 5. Testing expectations

Every behavioural change needs a test. Specifically:

- an acceptance criterion from `docs/features.md` maps to a named test, and the row names the test
  ([`docs/testing.md`](docs/testing.md) §2);
- the argv generated for a scenario is tested against a golden fixture captured from the C++ program, not
  against what seems reasonable;
- corrupted, hostile and refusal cases are as important as success cases: an unparsable backend output, a
  malformed extension file, a URL that runs to 64 KiB, a filename with a newline in it;
- tests must not touch the network. Backend output is replayed from fixtures; a test that needs the internet
  is a manual procedure documented in `docs/testing.md`, not a unit test;
- fixtures are committed only when they are anonymised captures. Anything that contains a real cookie,
  token or private URL is rejected in review.

If you find a case where Morid produced a different result from the original, **that is the highest-priority
bug class in the project**. Include the fixture, the expected result from `legacy/`, and the result Morid
produced.

---

## 6. Reporting bugs

Use the issue templates for ordinary bugs. For a suspected vulnerability follow [SECURITY.md](SECURITY.md)
and do not disclose it publicly.

For a suspected compatibility bug — Morid downloads something different from Media Downloader for the same
input — report:

1. the exact inputs (URL class, options, backend and version);
2. the command line Morid generated and the one the original generated (both are logged verbatim);
3. what was downloaded in each case.

---

## 7. Commit messages

Conventional Commits:

```
feat(engine): parse yt-dlp destination lines
fix(ext): reject schema version above 4 with a clear message
docs: specify the CLI exit codes
test(store): cover import of a settings.ini with unknown keys
```

Keep the subject under 72 characters. Explain *why* in the body when the change is not obvious. Reference the
`F-` ID or the decision record (`D-00N`) when there is one.

---

## 8. Licence and provenance

Morid is **GPL-2.0-or-later**. By contributing you agree that your contribution is licensed under those terms.
There is no CLA.

Two rules that are not negotiable:

- **Do not remove or alter the copyright notices** of the original author. They are in `LICENSE.txt` and in
  the headers of `legacy/`. The project is a derivative work of GPL-2.0-or-later code, and the notices stay.
- **Do not copy code from projects with incompatible licences.** Backend tools have different licences —
  yt-dlp is Unlicense, gallery-dl is GPL-2.0, wget is GPL-3.0, aria2 is GPL-2.0 — and none of their code is
  linked or copied here. Morid *runs* them as separate programs. See [THIRD-PARTY.md](THIRD-PARTY.md).

Changing the project licence requires an analysis of code provenance and the owner's decision; it is not a
pull request.

---

## 9. `legacy/` is read-only

`legacy/` contains the original C++ program and stays in the repository permanently
([D-015](docs/decisions.md)). It is a reference, not a product: it receives no features, no cleanup and no
security fixes.

Changes inside `legacy/` are allowed only when they are strictly necessary to compare behaviour — for example
a build fix that lets it compile on a current system. Such a change must say in the commit message which
comparison it enables.

---

## 10. Review culture

Reviews here are about evidence, not preference. Expect questions like:

- Which `F-` ID is this, and which test proves the acceptance criterion?
- What argv does this produce, and how do you know the original produces the same one?
- What happens with a 64 KiB URL, a non-UTF-8 filename, a backend that dies mid-line, a download directory that
  does not exist?
- What does this cost in binary size?
- If it disagrees with `legacy/`, why is the new behaviour right and where is the decision that says so?

Those questions are the mechanism by which a reimplementation earns trust. They are not hostility.
