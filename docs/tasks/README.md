# Tasks issued to the coder

Each file here is a complete assignment: goal, context, what to read, what to produce, constraints, acceptance
criteria, what to report back, and which documents change as a result. They are issued one at a time, by the
project manager through the owner, and a task is not started before the previous one is accepted.

| # | File | Phase | Estimate | State |
| --- | --- | --- | --- | --- |
| 1 | [phase-0-fixtures.md](phase-0-fixtures.md) | Phase 0 — reference behaviour and fixtures | 5–8 days | **accepted 7 Nov 2026** in `cc04893`; CI of its checks deferred to Phase 1 by D-017 |
| 2 | [phase-1-skeleton.md](phase-1-skeleton.md) | Phase 1 — workspace skeleton and CI | 5–7 days | **accepted** in PR #4 (`b6c2a58`), merged |
| 3 | [phase-2-domain.md](phase-2-domain.md) | Phase 2 — domain types, extension schema, command resolution | 10–14 days | **ready to issue** |

Task 1 produces no Rust code and must not be skipped: without it there is no definition of "same behaviour" to
test against. Task 2 produces no functionality: it produces the pipeline that later phases are checked by.

## How results are reviewed

A task is not accepted because it "looks done". The review checks the acceptance criteria by running the checks,
not by reading the report, and compares the claimed results against the actual output. The coder's hand-back must
contain: files changed, a description of the implementation, the actual test and build output, known problems, and
anything not done.

A discrepancy between a document and reality is reported, not silently resolved. Prose documents in `docs/` are
updated by the manager after review; the coder updates only the documents a task names explicitly.

## Branch situation, 7 November 2026

PR #2 was merged (`6407823`), which closed `arena/17dce945-morid` for writing. The Phase 1 work that had started on
that branch — eight commits, `e1192eb` … `fe0f6a0` — is still on the remote and is **not** in `main`, and its tip is
red: the job stops on a manifest error before anything is built. The remote said so plainly, and the report says so
too; neither is a reason to redo the work.

To continue: the coder gets a fresh branch from `main`, takes the existing commits from the old branch rather than
rewriting them, and finishes there. See [phase-1-skeleton.md](phase-1-skeleton.md) §3.1a for what those commits
prove and what they do not.

**Ordering decision (owner, 7 November):** the documentation branch `arena/8b9410cd-morid` (PR #3) is merged **after**
Phase 1, not before. Its four spec corrections are needed by Phase 1 while it is being written, so the coder must
read them from that branch (`git show origin/arena/8b9410cd-morid:docs/...`) and make the corresponding corrections
in their own branch. Expect a conflict in `docs/roadmap.md` and `docs/workspace-manifest.md` when PR #3 lands; the
manager resolves it, and the correction is the same text on both sides, so it is mechanical.

---

## Rule for the manager: specify outcomes, let the coder read the source

The manager's job in a task is to say **what the result must be** — behaviour, interface, invariants, acceptance
criteria — and to say **where the answer lives**, naming the file and the section or function. It is not the
manager's job to read the old C++ and translate it into instructions.

That division is a correction from the owner (10 November 2026), and it is a reaction to the record below, which
has one shape: the manager read a symptom, inferred a mechanism, and wrote the inference into a task as fact. Four
times. Each time the artefact was a minute away and would have said something else. Reading the source more
carefully would have made the manager a slower, more confident version of the same mistake: the tasks would have
grown into paraphrases of C++, and a paraphrase of 40 000 lines is not a specification, it is a place to hide
errors.

So the division of labour is:

- **Manager:** the contract. What the behaviour must be from the user's side, which interface it appears through,
  which invariant it must not break, what is checked and how. "`morid-ext` resolves the command for this platform;
  the three thin files behave as the original behaves; here is the criterion that proves it."
- **Coder:** the mechanism. Read `legacy/`, follow the exact dispatch, and implement what the code does — not what
  a plausible description of the code says. When the result of reading is surprising, report it and it becomes a
  correction to the document.
- **Both:** the criterion must be falsifiable from outside the implementation. "Same as the original" is not a
  criterion unless there is a test that would fail if it were not.

A task may still contain a technical claim. When it does, it carries its source inline and says what to do if the
source disagrees — as Phase 2 §3.2 does with the three transformations. That is not the manager asserting a
mechanism; it is the manager naming one and handing the check to the person who is going to read the file anyway.

This rule exists because of a record, not a principle. In Phase 0 the manager wrote three wrong technical
statements into tasks, each of which the coder disproved from the code:

| Written | Reality | Cost |
| --- | --- | --- |
| G4 told the coder to check the `reason` field of `log.txt` | the field is `program` | one exchange |
| G6 blamed the capture host's locale for s20 losing its download folder | the cause is `settings.ini` being literal UTF-8 while Qt 5 reads non-escaped bytes as Latin-1 | a wrong task (T7 as first written), withdrawn and rewritten |
| `\xHHHH` notation, and an instruction not to handle quoting | Qt writes lowercase unpadded `\x<hex>`, and does quote values containing `;`, `,`, `=` or edge spaces | two defects (G7, G8) shipped into a delivered helper; tasks T9, T10 |
| "thin extension files inherit the rest from their family by `Name`" (Phase 2 task) | `legacy/src/engines.cpp:244-258` dispatches on the **exact file name** and applies one hand-written transformation per file in `engines/yt-dlp.cpp`. There is no family lookup | caught by the coder before writing code, because the task told them to stop if the mechanism differed; corrected in `extensions.md` §1 and the task |
| `\|\|` is "not implemented by the original", and `PlaylistRange` is a range grammar with rejection rules (Phase 2 task and spec) | `engines.cpp:1574` implements `\|\|` and `gallery-dl.json` ships it; the range field is free text split by `splitPreserveQuotes` and never validated (`playlistdownloader.cpp:1061-1069`) | caught by the coder; both corrected in `extensions.md` §5.1 / §5.4 and the task; the validator requirement was **withdrawn** rather than specified |

The pattern is the same in all five: a plausible mechanism was accepted because it explained the symptom, and the
check that would have falsified it took a minute.

The fourth and fifth are worth separating from the first three, because they are the ones that **cost nothing** — and
the fifth is the last of its kind, because the owner has since changed how tasks are written (above). The fifth is
also the clearest illustration of why that change was needed: the wrong claim was not a detail I had mis-remembered
from a file I had read, it was a **requirement I had invented** — a range validator the original does not have. Had
the coder implemented what the task asked, Morid would have rejected playlist ranges that the original accepts, and
no test would have caught it, because the test would have been written from the same invented rule. That is the
asymmetry worth remembering: an invented mechanism costs a round, an invented requirement can cost the compatibility
promise (D-003) without anyone noticing.

Neither cost a round, a commit or a rebuild: the task said "if the mechanism differs, stop and report", the coder
checked `legacy/` before writing a line, and the correction landed in the specification instead of in code that would
have been rewritten. That sentence — name the claim, say where it came from, and tell the coder to stop if it is wrong —
is cheaper than being right, and it is now standard in every task that describes a mechanism I have not read the
source for.

There is also a case that is the reviewer's rather than the manager's: in Phase 0 the review *missed* G5 and G7, which
the next round then found. So the rule has a second half — **before accepting a round, try to break it**, and record
what was tried.

What the coder should do when a task contradicts the code: report the contradiction with the evidence and stop
on that point. Do not implement a claim you can see is false, and do not silently correct it either.

---

## Standing constraints for every task

- No code, no file and no dependency is added without a task that asks for it.
- `legacy/` is read-only except for a mechanical move ([../../legacy/README.md](../../legacy/README.md) §2).
- No `unsafe`, no `build.rs`, no async runtime, no dependency outside the accepted list in
  [../dependencies.md](../dependencies.md) without asking first.
- Every claim in a hand-back is backed by output. A result that was not observed is reported as not observed.
