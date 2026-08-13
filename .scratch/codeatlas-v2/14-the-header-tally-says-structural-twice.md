# Ticket 14 — the header tally says "structural" twice

**Status:** ready
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

- [ ] The header never renders the grouping mode and the provenance count
      using the same word.
- [ ] Both facts stay visible and correct across the provenance mixes: all
      mechanical, all enriched, and mixed.
- [ ] A test asserts the two labels cannot collide, proven able to fail by
      restoring the old wording.

## Notes

The fix is wording, not information. Neither fact is redundant — a reader
needs to know how the canvas is grouped and how much of the prose was
purchased — so do not solve the collision by deleting one of them.
