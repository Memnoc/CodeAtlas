# Ticket 02 — significance is published in the map

**Status:** ready
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 6 — the scanner publishes each file's significance so that the
tour, the default drill view and the rankings agree by construction; 9 — new
fields are optional, so every map that validated yesterday validates today
**Blocks:** 03 (the drill view selects on this number)
**Blocked by:** none — can start immediately

## Problem

The number that answers "which files matter" exists, but only inside the tour
builder, as a local computed during `build_tour`. Nothing else can read it, so
every other consumer that wants the same answer re-derives it — and the
dashboard's rankings already derive a different one. V2's drill view needs
this number too, and a third private derivation would guarantee that the
tour, the drill view and the rankings disagree about the same repository.

[ADR-0010](../../docs/adr/0010-file-significance-is-published-once-in-the-map-contract.md)
decided it is published once, in the contract.

## What to build

A fresh scan publishes each file node's significance, computed once, and the
tour selects on the published number instead of a private one.

## Acceptance criteria

- [ ] Every file node in a fresh scan carries `significance` = import fan-in
      + import fan-out + 1 if the file hosts an entry point.
- [ ] The formula exists in exactly one place. Tour *selection* reads the
      published number; tour *ordering* stays the tour's own and is unchanged.
- [ ] Symbol nodes carry no significance — it is a file-level number.
- [ ] A fresh scan publishes it for every file, including files whose
      significance is zero. Optionality exists for old maps, not for omitting
      zeros; a zero is a fact.
- [ ] The field is optional in the contract: a map written before this ticket
      validates unchanged, asserted against a stored fixture map that lacks
      it.
- [ ] Schema and generated TypeScript regenerate together and the drift gate
      passes (ADR-0003 ceremony).
- [ ] Fixture-tree contract tests cover: fan-in only, fan-out only, the
      entry-point bonus, and the zero case.
- [ ] The existing tour tests stay green without being edited — this refactor
      is behaviour-preserving, and an edited assertion would hide that it
      wasn't.

## Notes

The formula is now public API. Changing it later is a contract event with the
ADR-0003 ceremony attached, which is exactly why ADR-0010 pinned `build_tour`'s
existing arithmetic rather than inventing a better score on the way past.

The dashboard's per-region complexity rating is deliberately *not* unified
with this — it answers a different question about a region, not a file. Leave
it alone.
