# Ticket 14 — the header tally says "structural" twice

**Status:** done
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 21 — the header tally stops reading "STRUCTURAL · 0 structural", so
the grouping mode and the provenance count no longer share one word
**Blocks:** none
**Blocked by:** none — can start immediately

## Problem

The header prints the active grouping mode beside the provenance tally, and
both use the word "structural" for unrelated things: one names how the canvas
is grouped, the other counts nodes whose prose nobody bought. On a fully
enriched map the header reads "STRUCTURAL · 0 structural", which parses as a
contradiction and is in fact two true facts colliding.

## What to build

Two facts that read as two facts.

## Acceptance criteria

- [x] The header never renders the grouping mode and the provenance count
      using the same word.
- [x] Both facts stay visible and correct across the provenance mixes: all
      mechanical, all enriched, and mixed.
- [x] A test asserts the two labels cannot collide, proven able to fail by
      restoring the old wording.

## Notes

The fix is wording, not information. Neither fact is redundant — a reader
needs to know how the canvas is grouped and how much of the prose was
purchased — so do not solve the collision by deleting one of them.

## Decision (2026-08-14)

The grouping-mode label moved; the provenance count kept its word.
"structural" as a provenance kind is contract vocabulary
(`Provenance::Structural` in the map contract) and the dashboard's
provenance badges render it literally, so the tally's "N structural" stays
consistent with every badge. The grouping label is dashboard-side
presentation, and the glossary already names the grouping's unit: a Layer.
The segment and the tally now both say **Layer** (paired with **Domain** —
each grouping labelled by the unit it groups into), sourced from one
`groupingLabels` map in `Header.tsx` so the header cannot say one word
where the control says another. The walkthrough's grouping step and the
two tests that click the segment by name moved with it. CONTEXT.md's
Region entry ("a layer under the structural grouping, or a domain under
the domain grouping") stays true — it names the grouping concept, not the
segment's text — so the glossary was not touched.

## Guard proven able to fail (2026-08-14)

Old wording restored (`<strong>{grouping}</strong>` in `Header.tsx`), then
`npx vitest run tests/render.test.tsx -t "never says the grouping and the
count with one word"`:

    FAIL  tests/render.test.tsx > the header tally reads as two facts >
    never says the grouping and the count with one word, whatever the mix
    AssertionError: expected [ 'structural' ] to not include 'structural'

Fix re-applied; the same file then passes 16/16 (run at 10:17). The guard
walks every radio the Grouping control offers rather than naming them, so
it survives future relabelling and trips on any recollision.
