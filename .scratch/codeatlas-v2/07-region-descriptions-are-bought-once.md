# Ticket 07 — region descriptions are bought once

**Status:** ready
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 8 — region descriptions are bought once and carried over by content
hash, so a re-scan never re-buys prose that is still valid; 7 — the purchased
half of the region card
**Blocks:** none
**Blocked by:** 06 — there is nowhere to put the prose until the field exists

## What to build

Enrichment gains one slot kind — a layer's description — stored in the
annotation store and carried over by content hash exactly as enriched layer
names are.

## Acceptance criteria

- [ ] A new slot kind offers each structural layer's description, and a
      provider's answer lands in the description — never in the name.
      Addressing stays collision-proof across kinds through the existing
      key-prefix rule.
- [ ] The slot carries mechanically summarised topology only: the layer ID
      and its file count, matching the layer-name slot's discipline. Never
      the member list, never edges.
- [ ] The answer is stored and carried over on the same derivation-input hash
      the layer name uses, so a re-scan that changes nothing re-buys nothing,
      and a layer whose membership changed expires its prose.
- [ ] Name and description coexist in the store for the same layer without
      either claiming the other's key.
- [ ] A store written before this ticket keeps re-attaching. Whether the
      store version bumps is decided explicitly and recorded: a bump is a
      bill, and is only worth sending when the data would otherwise be wrong.
- [ ] The cost estimate counts the new slots, so `--dry-run` still says what
      a run will buy.
- [ ] A blank or refused answer never replaces the mechanical text — the
      existing rule, extended to this slot.
- [ ] Tests use a scripted provider double. No live provider, no API key, no
      subscription spend — ever, in any test in this repository.

## Notes

No new carry-over mechanism. This is
[ADR-0005](../../docs/adr/0005-full-rescan-with-content-hash-enrichment-carry-over.md)
discipline applied to one more slot; if the work seems to need a new hashing
rule, that is a signal the slot is being modelled wrong.

The prose is bought for a *layer*, and layers are stable — this is one of the
cheapest slot kinds in the system. Eight layers here against 1100+ node
summaries; do not add batching machinery for it.
