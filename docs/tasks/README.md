# Tasks issued to the coder

Each file here is a complete assignment: goal, context, what to read, what to produce, constraints, acceptance
criteria, what to report back, and which documents change as a result. They are issued one at a time, by the
project manager through the owner, and a task is not started before the previous one is accepted.

| # | File | Phase | Estimate | State |
| --- | --- | --- | --- | --- |
| 1 | [phase-0-fixtures.md](phase-0-fixtures.md) | Phase 0 — reference behaviour and fixtures | 5–8 days | delivered; T1–T5 done in `b344cdc`, T6–T8 done in `f187923`; **round 3: T9–T11 open** |
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

## Rule for the manager: verify before specifying

Every technical claim written into a task must come from the artefact — source, output of a run, or a documented
API contract — and not from the manager's recollection. When a claim cannot be checked, the task says so and asks
the coder to establish it instead of asserting it.

This rule exists because of a record, not a principle. In Phase 0 the manager wrote three wrong technical
statements into tasks, each of which the coder disproved from the code:

| Written | Reality | Cost |
| --- | --- | --- |
| G4 told the coder to check the `reason` field of `log.txt` | the field is `program` | one exchange |
| G6 blamed the capture host's locale for s20 losing its download folder | the cause is `settings.ini` being literal UTF-8 while Qt 5 reads non-escaped bytes as Latin-1 | a wrong task (T7 as first written), withdrawn and rewritten |
| `\xHHHH` notation, and an instruction not to handle quoting | Qt writes lowercase unpadded `\x<hex>`, and does quote values containing `;`, `,`, `=` or edge spaces | two defects (G7, G8) shipped into a delivered helper; tasks T9, T10 |

The pattern is the same in all three: a plausible mechanism was accepted because it explained the symptom, and
the check that would have falsified it took a minute. A fourth case is the reviewer's, not the coder's fault by
construction: the review *missed* G5 and G7 in previous rounds, which the next round then found. So the rule has a
second half — **before accepting a round, try to break it**, and record what was tried.

What the coder should do when a task contradicts the code: report the contradiction with the evidence and stop
on that point. Do not implement a claim you can see is false, and do not silently correct it either.

---

## Standing constraints for every task

- No code, no file and no dependency is added without a task that asks for it.
- `legacy/` is read-only except for a mechanical move ([../../legacy/README.md](../../legacy/README.md) §2).
- No `unsafe`, no `build.rs`, no async runtime, no dependency outside the accepted list in
  [../dependencies.md](../dependencies.md) without asking first.
- Every claim in a hand-back is backed by output. A result that was not observed is reported as not observed.
