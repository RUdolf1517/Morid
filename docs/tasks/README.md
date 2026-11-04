# Tasks issued to the coder

Each file here is a complete assignment: goal, context, what to read, what to produce, constraints, acceptance
criteria, what to report back, and which documents change as a result. They are issued one at a time, by the
project manager through the owner, and a task is not started before the previous one is accepted.

| # | File | Phase | Estimate | State |
| --- | --- | --- | --- | --- |
| 1 | [phase-0-fixtures.md](phase-0-fixtures.md) | Phase 0 — reference behaviour and fixtures | 5–8 days | not started |
| 2 | [phase-1-skeleton.md](phase-1-skeleton.md) | Phase 1 — workspace skeleton and CI | 5–7 days | not started |

Task 1 produces no Rust code and must not be skipped: without it there is no definition of "same behaviour" to
test against. Task 2 produces no functionality: it produces the pipeline that later phases are checked by.

## How results are reviewed

A task is not accepted because it "looks done". The review checks the acceptance criteria by running the checks,
not by reading the report, and compares the claimed results against the actual output. The coder's hand-back must
contain: files changed, a description of the implementation, the actual test and build output, known problems, and
anything not done.

A discrepancy between a document and reality is reported, not silently resolved. Prose documents in `docs/` are
updated by the manager after review; the coder updates only the documents a task names explicitly.

## Standing constraints for every task

- No code, no file and no dependency is added without a task that asks for it.
- `legacy/` is read-only except for a mechanical move ([../../legacy/README.md](../../legacy/README.md) §2).
- No `unsafe`, no `build.rs`, no async runtime, no dependency outside the accepted list in
  [../dependencies.md](../dependencies.md) without asking first.
- Every claim in a hand-back is backed by output. A result that was not observed is reported as not observed.
