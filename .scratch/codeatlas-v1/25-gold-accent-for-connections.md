# Ticket 25 — connections need their own colour, not the cards' colour

**Status:** ready
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 3 — interactive local dashboard (graph canvas, search, layer grouping, node detail)
**Blocks:** none
**Blocked by:** none

## Problem

Edges and cards currently sit in the same cool part of the palette, so at a
glance the lines read as more card-chrome rather than as the relationships
they are. On a drilled-in region the eye has to work to separate the two.

Requested 2026-08-11: warm the connections up — more yellow or orange —
**staying inside the Rosé Pine palette**.

## Acceptance criteria

- [ ] Edges take a warm accent from Rosé Pine — `gold` is the obvious
      candidate (`#f6c177` on Moon, `#ea9d34` on Dawn) — and no colour is
      introduced that is not in the palette.
- [ ] It works in all three theme states: Dawn, Moon, and the system default
      that stamps no `data-theme` attribute.
- [ ] Lit and dimmed edges stay distinguishable from each other after the
      change; the dim state is what makes a selection readable, and a warmer
      hue at low opacity can lose that.
- [ ] The arrowhead on a lit edge picks up the same accent — a warm line with
      a cool arrowhead reads as two things.
- [ ] Contrast against the canvas background is checked at both themes rather
      than assumed; gold on the Dawn background is the risky one.

## Notes

Aesthetic, but not only aesthetic: the reason to warm the edges is that hue
separates them from the cards faster than weight or opacity can. Judge the
result on a real drilled-in region with fifty-odd edges, not on a two-node
sketch.
