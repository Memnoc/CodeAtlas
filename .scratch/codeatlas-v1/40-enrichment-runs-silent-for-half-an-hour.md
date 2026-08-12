# Ticket 40 — enrichment runs silent for half an hour

**Status:** ready
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 12 — enrich a map through a real backend; this is the operator's
side of it, not the enrichment itself
**Blocks:** none
**Blocked by:** none
**Filed:** 2026-08-12, from a real run on this repository

## Problem

`codeatlas scan --enrich` prints `mapped 280 files` and then says nothing
until it is finished. On this repository that silence lasts **20–35 minutes**.

Measured on a real run, 2026-08-12:

| | |
|---|---|
| summary slots (file + function + class) | 1268 |
| flow and tour slots | 325 |
| `BATCH_SIZE` | 25 |
| provider calls, sequential | **~64** |

The user asked *"it seems it is stuck at mapped 280 files?"* — and the only
way to answer was to look at the process table for a live `claude --print`
child and compare its age against the parent's. That is not an answer a
reader can get for themselves, and "is this hung or is this working" is the
one question a long silent process guarantees they will ask.

This is the longest operation in the product and the only one that costs the
reader money or subscription allowance. It is the *last* place silence is
affordable.

## What to build

`scan --enrich` reports its progress as it goes: how many batches are done,
how many there are, and enough to tell a working run from a wedged one.

The total is available before the first call — `enrich`'s `slots` is a fully
collected `Vec` and the loop is `slots.chunks(BATCH_SIZE)` — so this is
`batch 7 of 64`, not a spinner. Do not settle for a spinner.

## Acceptance criteria

- [ ] A run against a multi-batch map reports progress that **advances**, and
      the final report accounts for every batch. Assert the sequence, not that
      output is non-empty: a single line printed once satisfies "it printed
      something" and leaves the reader exactly as stuck.
- [ ] The count of batches reported matches the count of provider calls
      actually made. Drive it at seam 2 with the `fake:` backend over a
      fixture large enough to need several batches, and count the calls.
- [ ] Nothing is printed per *slot*. 1593 lines is not progress.
- [ ] Progress goes to **stderr**, where `mapped N files` and every other
      line `scan` writes already goes. `scan` has no stdout contract today and
      this must not invent one.
- [ ] A failed batch is still visible as a failure, and the existing
      guarantee holds unchanged: the structural map survives, and the run says
      so. The egress suite's `the structural map is intact` assertion must
      still pass.
- [ ] Whatever the run prints when its output is a pipe rather than a
      terminal is what the test drives. A `\r`-updating line is reasonable on
      a TTY and unreadable in a log; if the two differ, the test asserts the
      one it can actually capture, and the TTY form is named as unverified.

## Notes

**Think about what a reader needs, not what is easy to print.** Batches
completed against batches total is the minimum. An elapsed time or a rough
estimate of remaining time would have answered the actual question — *should
I wait or kill it* — in one line. Consider it; do not let it delay the count.

**Do not add a dependency for this.** ADR-0006 bounds the audit surface, and
a progress-bar crate is exactly the kind of thing that looks free. Two
`eprintln!`s and a counter.

**Do not make it chatty by default and quiet behind a flag.** The default run
is the one that goes silent for half an hour.
