# Ticket 40 — what it will cost, and how it is going

**Status:** in-progress — 2026-08-12
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 12 — enrich a map through a real backend; this is the operator's
side of it, not the enrichment itself
**Blocks:** none
**Blocked by:** none
**Filed:** 2026-08-12, from a real run on this repository. **Amended the same
day** to fold in the cost estimate, which was nearly a separate ticket: it is
the same function, the same stream, and the same reader question asked a
moment earlier, and two tickets editing the same three lines is worse than
one.

## Problem

`codeatlas scan --enrich` tells the reader nothing before it starts and
nothing while it runs. It prints `mapped 280 files`, goes silent for 20–35
minutes, and prints a count at the end.

Both halves were hit for real on 2026-08-12, in this order:

**Before.** The reader agreed to a run on an estimate of "about a dozen
provider calls" — which was wrong, because it counted only file nodes. The
real figure is 64. Nothing in the tool would have corrected that, and by the
time it was known the run was already several minutes in and had already spent
the difference.

**During.** *"It seems it is stuck at mapped 280 files?"* Answering that
required looking at the process table for a live `claude --print` child and
comparing its age against the parent's. A reader cannot do that, and "is this
hung or is this working" is the one question a long silent process guarantees
they will ask.

This is the longest operation in the product and the only one that spends the
reader's money or subscription allowance. It is the last place silence is
affordable at either end.

## Measured on this repository, 2026-08-12

| | |
|---|---|
| summary slots (file + function + class) | 1268 |
| flow and tour slots | 325 |
| `BATCH_SIZE` | 25 |
| provider calls, sequential | 64 |
| prompt characters | ~550,000 |
| input tokens | ~143k – 191k |
| output tokens | ~40k – 73k |

The last three were approximate when this was filed — hand-computed at
~138k–184k. The figures above are what the built feature now measures off the
real prompts, and they are ~4% higher: the hand version undercounted the
system prompt and the schema, which are sent on all 65 calls. That the ticket
about not quoting loose numbers was itself filed with a loose number is the
whole argument for measuring from the real code path.

## What to build

**One line before the work starts, printed unconditionally**, saying what the
run is about to do: slots, calls, and an estimated token range. No flag to
remember — a reader who has to know to ask is a reader who finds out
afterwards.

**Progress while it runs**, so a working run is distinguishable from a wedged
one. The total is known before the first call — `slots` is a fully collected
`Vec` and the loop is `slots.chunks(BATCH_SIZE)` — so this is `batch 7 of 64`,
not a spinner. Do not settle for a spinner.

**`--dry-run`**, which prints the estimate and exits having made zero provider
calls, for deciding whether to spend at all.

### The estimate is a computation, not a guess — mostly

The input side is nearly exact and should be built that way: the slots are
collected and batched before anything is sent, so the real requests can be
constructed, measured, and discarded.

**Compute it from the same code path the real run uses.** Any separate
estimator drifts from reality the first time batching or the prompt changes,
and a confident wrong number is worse than no number. This is also what makes
it testable in a way that can fail.

Three things it must not do:

- **Not a single number.** There is no local Anthropic tokenizer and adding a
  crate for one is what ADR-0006's audit-surface bound exists to prevent
  (`tiktoken` is a different tokenizer and would be wrong anyway). So it is
  characters over a stated divisor: a **range**. `~140k–185k input tokens` is
  honest; `152,431 tokens` is a false number waiting to be quoted back.
- **Not money.** Prices change, so a rate compiled into the binary is a future
  false claim of exactly the species this project has already been bitten by
  three times. And on `cli:claude` there is no monetary cost at all — it is
  subscription allowance. Report calls and tokens; let the reader price them.
- **Not one total.** Input is computed, output is estimated from "one sentence
  per slot". Adding them yields a confident figure that half deserves and half
  does not. Report them separately and say which is which.

## Acceptance criteria

- [ ] Every `--enrich` run prints, before its first provider call: the slot
      count, the number of calls it will make, and an input-token **range**.
- [x] The predicted call count **equals** the calls actually made. Assert it
      against a fixture at seam 2 by counting provider invocations — this is
      the criterion that keeps the estimate from drifting into fiction, and it
      is the reason the estimate must come from the real code path.
- [ ] `--dry-run` prints the same estimate and makes **zero** provider calls.
      Count them; asserting on the output alone passes over a dry run that
      quietly enriches.
- [x] The token figure is rendered as a range and never as an exact count, and
      no output anywhere states a price.
- [ ] Progress **advances** during the run, and the final report accounts for
      every batch. Assert the sequence, not that output is non-empty: one line
      printed once satisfies "it printed something" and leaves the reader
      exactly as stuck.
- [x] Nothing is printed per *slot*. 1593 lines is not progress.
- [ ] All of it goes to **stderr**, where `mapped N files` and every other
      line `scan` writes already goes. `scan` has no stdout contract today and
      this must not invent one.
- [x] A failed batch is still visible as a failure, and the existing guarantee
      holds unchanged: the structural map survives and the run says so. The
      egress suite's `the structural map is intact` assertion must still pass.
- [x] Whatever the run prints when its output is a pipe rather than a terminal
      is what the test drives. A `\r`-updating line is reasonable on a TTY and
      unreadable in a log; if the two differ, the test asserts the one it can
      capture and the TTY form is named as unverified.

## Notes

**No confirmation prompt, for now.** It is the obvious next thought and it
breaks every non-interactive use, so it would need TTY detection plus a
`--yes`. The unconditional estimate line already answers *should I be worried*
before anything has been spent, which is most of the value for none of the
cost. Revisit only if a reader still gets surprised.

**Think about what a reader needs, not what is easy to print.** Batches done
against batches total is the minimum. An elapsed time, or a rough estimate of
time remaining, answers the actual question — *should I wait or kill it* — in
one line. Consider it; do not let it delay the counter.

**Do not add a dependency for any of this.** ADR-0006 bounds the audit
surface, and a progress-bar crate is exactly the kind of thing that looks
free. Two `eprintln!`s and a counter.

**Do not make it chatty by default and quiet behind a flag.** The default run
is the one that goes silent for half an hour.

**Related.** Ticket 42 makes an interrupted run cheap to resume, which changes
what the progress line should probably say — "batch 7 of 64, 39 already
enriched" is a different and better sentence. Neither ticket blocks the other,
but whichever lands second should revisit the wording.
