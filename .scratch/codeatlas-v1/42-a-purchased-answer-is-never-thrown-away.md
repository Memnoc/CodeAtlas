# Ticket 42 — a purchased answer is never thrown away

**Status:** ready
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 12 — enrich a map through a real backend
**Blocks:** none
**Blocked by:** none
**Filed:** 2026-08-12, from a real 35-minute run on this repository

## Problem

`fill_slots` is sequential and all-or-nothing:

```rust
let mut answers: BTreeMap<String, String> = BTreeMap::new();
for batch in slots.chunks(BATCH_SIZE) {
    answers.extend(provider.enrich(&request)?.answers);
}
```

and `run` writes nothing until it returns:

```rust
let count = fill_slots(graph, provider.as_ref())?;
save_store(root, graph, provider.identity())?;
```

Two problems, and they are one ticket because fixing either alone makes the
other worse.

**Purchased answers are discarded.** Answers accumulate in memory for the
whole run. On this repository that is ~64 provider calls over 20–35 minutes.
A `Ctrl-C` at minute 30 loses all of it. So does **one transient failure on
batch 63**, through that `?`. The reader has paid — in money on the `claude`
backend, in subscription allowance on `cli:claude` — and the failure mode
throws away the exact thing they paid for.

`fill_slots`'s doc comment calls this deliberate: *"Any batch error fails the
whole step: the caller never saves a partially-purchased run."* The intent is
right and the conclusion does not follow. Not shipping a **half-enriched map**
is worth defending. Discarding **sixty-three successful answers** is not the
same thing, and the two are separable: checkpoint the answers, still write the
map once at the end.

**It is sequential when it does not need to be.** Each batch is an independent
process spawn or HTTP call. `SelectedProvider` is already
`Box<dyn EnrichmentProvider + Send + Sync>` and `SharedProvider` is already an
`Arc` of the same, used across threads by `serve` — the design is there and
`fill_slots` simply happens to be a `for`.

**Why one ticket.** Concurrency without checkpointing makes the loss strictly
worse: more calls in flight means more purchased work to throw away when one
of them fails. Checkpointing is what makes concurrency safe to add.

## What to build

Every answered batch is durable as soon as it lands, an interrupted run
resumes without re-buying what it already has, and batches run several at a
time.

Resumption is nearly free already: `AnnotationStore::reattach` restores
annotations by content hash with no provider call, and `collect_slots` only
yields slots that are still structural. A re-run after a checkpointed
interruption should cost only what is left.

## Acceptance criteria

- [ ] A run interrupted partway through, then re-run, makes provider calls
      only for the slots it did not already have. **Count the calls** — a test
      that only checks the final map is satisfied by re-buying everything.
- [ ] A batch that fails does not discard the batches that succeeded before
      it. This needs a backend that fails on the *nth* call; `fail` refuses
      everything and `fake:` refuses nothing, so seam 2 likely needs one more
      test provider. Adding it is part of this ticket.
- [ ] Batches actually run concurrently: assert **calls in flight**, not
      elapsed time and not the final result. A wall-clock assertion is flaky
      and a result assertion passes over a sequential implementation — this is
      the criterion most likely to be ticked against something that cannot
      fail.
- [ ] Concurrency does not change the outcome. The same fixture through the
      same fake backend yields byte-identical output whatever the interleaving;
      answers are addressed by slot key, so this should hold by construction —
      assert it anyway.
- [ ] The map on disk is still written once, whole. A half-enriched
      `knowledge-graph.json` must not be observable, and the existing
      guarantee that a failed enrichment leaves the structural map intact must
      still hold — the egress suite's `the structural map is intact`
      assertion passes unchanged.
- [ ] The concurrency limit is bounded and conservative by default. A reader
      on a subscription has a rate limit, and the failure mode of guessing too
      high is their allowance, not a slow test.

## Notes

**No async runtime, no new dependency.** ADR-0006 bounds the audit surface.
`std::thread::scope` and a semaphore-ish counter, not `tokio` and not `rayon`.
If it cannot be done in std, say so in the write-up rather than reaching for a
crate.

**Rate limits are the real ceiling, not the design.** Four in flight is a
sensible starting point; eight is likely to trip a subscription limit and turn
a slow run into a failed one. Whatever is chosen, `--enrich` should degrade
sanely when the backend starts refusing — which is the other half of why
checkpointing comes first.

**This is filed `ready` rather than deferred**, unlike ticket 43, because the
data-loss half is a defect that costs the reader money rather than an
improvement to something that works. If it is split, split it that way round:
checkpointing first, concurrency second.
