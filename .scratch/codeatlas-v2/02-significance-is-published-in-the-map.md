# Ticket 02 — significance is published in the map

**Status:** done
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

- [x] Every file node in a fresh scan carries `significance` = import fan-in
      + import fan-out + 1 if the file hosts an entry point.
- [x] The formula exists in exactly one place. Tour *selection* reads the
      published number; tour *ordering* stays the tour's own and is unchanged.
- [x] Symbol nodes carry no significance — it is a file-level number.
- [x] A fresh scan publishes it for every file, including files whose
      significance is zero. Optionality exists for old maps, not for omitting
      zeros; a zero is a fact.
- [x] The field is optional in the contract: a map written before this ticket
      validates unchanged, asserted against a stored fixture map that lacks
      it.
- [x] Schema and generated TypeScript regenerate together and the drift gate
      passes (ADR-0003 ceremony).
- [x] Fixture-tree contract tests cover: fan-in only, fan-out only, the
      entry-point bonus, and the zero case.
- [x] The existing tour tests stay green without being edited — this refactor
      is behaviour-preserving, and an edited assertion would hide that it
      wasn't.

## Notes

The formula is now public API. Changing it later is a contract event with the
ADR-0003 ceremony attached, which is exactly why ADR-0010 pinned `build_tour`'s
existing arithmetic rather than inventing a better score on the way past.

The dashboard's per-region complexity rating is deliberately *not* unified
with this — it answers a different question about a region, not a file. Leave
it alone.

## Built

`semantics::publish_significance` holds the only copy of the formula and runs
between layer assignment and the tour, so `build_tour` selects on
`node.significance` and computes nothing of its own beyond reading order. The
two topology walks the tour also needs — `import_degree` and
`entry_point_files` — moved out of `build_tour` unchanged, which is why the
tour tests never had to be touched.

A new optional field is a minor bump under `contract/README.md`'s policy, so
the contract is **0.4.0** and the `$id`, both READMEs, the schema and
`dashboard/src/map.generated.ts` moved together. `Node.significance` is
classified `ShareSafe` in the redaction table: it is arithmetic over the
import graph the artifact already ships.

Guards proven able to fail by mutating the implementation and watching the
new assertions trip, each restored afterwards: dropping the fan-in term, the
fan-out term or the entry-point bonus; omitting zeros instead of publishing
them; publishing on symbol nodes; counting the bonus once per entry-point
function rather than once per file (`src/namespace.ts` hosts two, and the
assertion read 3 against 2); making the field required, which drew
`"significance" is a required property` against the stored pre-significance
fixture. Publishing zero for every file was the proof that tour *selection*
reads the published number: both unedited tour tests failed, and the tour
emptied.
