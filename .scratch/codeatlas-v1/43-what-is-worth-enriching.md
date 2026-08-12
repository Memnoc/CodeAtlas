# Ticket 43 — what is worth enriching

**Status:** deferred — after V1, and see "The case against this ticket"
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 12 — enrich a map through a real backend
**Blocks:** none
**Blocked by:** 42 — not technically, but doing 42 first may remove most of
the motivation for this, and a change that costs answer quality should not be
made to solve a problem that concurrency already solved
**Filed:** 2026-08-12

## Problem

Enrichment fills a slot for every file, function and class. On this
repository, measured 2026-08-12:

| slot source | count | share |
|---|---|---|
| functions | 891 | 70% |
| files | 280 | 22% |
| classes | 97 | 8% |
| flow and tour slots | 325 | (on top of the above) |

Function summaries are **70% of what an enrichment run costs**, and the cost
is real: money on the `claude` backend, subscription allowance on
`cli:claude`. A reader trying CodeAtlas for the first time on a large
repository meets a 35-minute, several-hundred-call bill before they have seen
what enrichment buys them.

The question this ticket asks: is every slot worth what it costs, and should
the reader get a say?

## The case against this ticket

**Function summaries are not decoration, and the first version of this ticket
said they were.** They feed the question path. `ask::kind_rank` ranks files
before classes before functions, but functions **are** in the slice — up to
`CONTEXT_NODES = 40` nodes accompany every question. A map enriched without
them answers questions from file-level prose and bare function *names*.

So this is not "trim the fat". It is a genuine trade between what a run costs
and how well the map answers, and it should be argued with evidence rather
than assumed. Anyone picking this up should be prepared to close it as
*won't do*.

**Ticket 42 has landed, and it did dissolve most of the problem.** Most of the
pain measured on 2026-08-12 was *wall-clock* and *risk of total loss*, not cost
as such. Four-way concurrency and per-batch checkpointing shipped in `394112f`;
ticket 40's estimate and progress shipped in `311ec74` and `5c7799f`. What is
left of the original complaint is only "it costs 70% more than it needs to",
which is much weaker than "it takes 44 minutes and one hiccup wastes all of
it" — and the reader is now told the figure before agreeing to it, which was
the actual grievance. **Re-read the case against this ticket before starting
it; closing it as won't-do is now the more likely right answer.**

## What to build, if the evidence supports it

Some control over which slot kinds an enrichment run fills, with a default
that is defensible for a first-time reader.

**Do not presuppose a flag.** At least three shapes are worth weighing:

1. **A scope option** — `--enrich-scope files,classes` or similar. Simple,
   explicit, and puts the trade in the reader's hands. Also one more thing to
   understand before the first run.
2. **Lazily, on demand** — enrich a function's summary the first time
   something needs it, from `serve`. Spreads the cost over use and spends
   nothing on code nobody looks at. Much larger change, and it puts a model
   call on an interactive path.
3. **A cheaper model for the cheap slots** — `--model` exists but is global.
   "Function summaries on the small model, everything else on the default"
   keeps every slot filled and cuts most of the cost. Probably the best value
   per unit of change, and it does not degrade the ask slice nearly as much
   as omitting the slots entirely.

Option 3 is the one to cost out first.

## Acceptance criteria

- [ ] The choice is **measured, not asserted**. Enrich this repository both
      ways and compare answers to the same set of questions. A ticket that
      changes what the reader pays for on the strength of a table of slot
      counts has not done the work.
- [ ] Whatever is built, the call count moves by the amount claimed. Count
      provider calls at seam 2 against a fixture with a known slot mix.
- [ ] A map with mixed provenance stays coherent. Unenriched slots already
      keep their mechanical text and their `structural` provenance, and the
      dashboard already badges the difference — assert that a partially
      enriched map renders and that the header tally reports it honestly.
- [ ] The question path is assessed explicitly, not left to be discovered:
      state what a question over a map enriched this way can and cannot
      answer, and put it in the write-up.
- [ ] Closing this as *won't do* with the measurement attached is a
      successful outcome and should be recorded as one.

## Notes

**The default matters more than the flag.** Whatever option is chosen, most
readers will run `scan --enrich` with no arguments, once, and judge the
product by what comes back. A flag nobody sets is not a decision — the default
is the decision.

**Ticket 40 is a cheaper answer to part of this, and it got cheaper.** A good
deal of the frustration behind this ticket was not knowing what the run would
cost before agreeing to it, nor whether it was working once it had started.
Ticket 40 now covers both — an estimate before the first call and progress
during — and neither costs a single point of answer quality. Weigh this ticket
*after* that one has landed: a reader who is told "1593 slots, 64 calls,
~140k–185k input tokens" up front may simply decide the full run is worth it,
which would close this as won't-do without changing any behaviour at all.
