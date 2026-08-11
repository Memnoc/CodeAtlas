# Ticket 24 — a way back, wherever the UX has a path

**Status:** done
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 3 — interactive local dashboard; 6 — domain flows and an ordered guided tour
**Blocks:** none
**Blocked by:** none

## Problem

The dashboard has several places where the reader goes *in* to something and
has no plain way out:

- drilling into a region card, which replaces the overview canvas
- selecting a file, which lights its edges and dims the rest
- stepping through the tour, where the only movement offered is forward

There is a breadcrumb, and clicking the canvas background clears a selection,
but both are things the reader has to discover. A person who has just arrived
somewhere wants a button that says *back*, in the place they are looking.

## Acceptance criteria

- [x] Drilling into a region shows a back control that returns to the
      overview, and it is reachable by keyboard.
- [x] The tour offers a previous step alongside the next one, disabled at the
      first step rather than absent — a control that appears and disappears
      is harder to aim at than one that greys out.
- [x] Back from a file selection returns to the region view with the region
      still open, not to the overview: one step back, not all the way out.
- [x] `Escape` does whatever the back control does, at every level.
- [x] The control states where it goes ("Back to regions") rather than
      just "Back", since the same word at three depths means three things.
- [x] Existing exits keep working — the breadcrumb and the click-the-canvas
      gesture are not replaced, only supplemented.

## Notes

The levels form a stack: overview → region → file, with the tour a separate
path through the same map. Worth writing down what "back" means at each level
before building any of it, because the bug this feature usually ships with is
a back button that skips a level or lands somewhere the reader never was.
