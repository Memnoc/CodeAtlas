# Ticket 25 — connections need their own colour, not the cards' colour

**Status:** done
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

- [x] Edges take a warm accent from Rosé Pine — `gold` is the obvious
      candidate (`#f6c177` on Moon, `#ea9d34` on Dawn) — and no colour is
      introduced that is not in the palette.
- [x] It works in all three theme states: Dawn, Moon, and the system default
      that stamps no `data-theme` attribute.
- [x] Lit and dimmed edges stay distinguishable from each other after the
      change; the dim state is what makes a selection readable, and a warmer
      hue at low opacity can lose that.
- [x] The arrowhead on a lit edge picks up the same accent — a warm line with
      a cool arrowhead reads as two things.
- [x] Contrast against the canvas background is checked at both themes rather
      than assumed; gold on the Dawn background is the risky one. Computed,
      not eyeballed — and Dawn was indeed the risky one. See Notes.

## Notes

Aesthetic, but not only aesthetic: the reason to warm the edges is that hue
separates them from the cards faster than weight or opacity can. Judge the
result on a real drilled-in region with fifty-odd edges, not on a two-node
sketch.

### Measured contrast, against the canvas background

| | Dawn `#faf4ed` | Moon `#232136` |
|---|---|---|
| lit (`--link`, 2px) | **2.05:1** | 9.55:1 |
| rest (`--link` @0.55, 1px) | 1.48:1 | 3.87:1 |
| dim (`--link` @0.14) | 1.10:1 | 1.37:1 |
| *previous* rest, for comparison | 1.12:1 | 1.25:1 |

Every resting edge is more visible than before on both themes, which is the
state the change was for — the hairline texture is what had to separate from
the cards.

A first attempt introduced a second, darker gold per theme for the resting
state. `/crosscheck` caught it: `#d3a04d` and `#8d7f61` are in no Rosé Pine
variant, which breaks this criterion and the stylesheet's own header rule.
Removing them cost nothing and gained something — one gold at three opacities
is identical on Dawn (1.48 vs 1.50) and markedly better on Moon (3.87 vs
2.14), because the invented colour was muddier than the palette's own.

**One number is worth knowing about: the lit edge on Dawn is 2.05:1**, under
the 3:1 that WCAG asks of non-text. Rosé Pine Dawn has exactly one gold
(`#ea9d34`) and darkening it would leave the palette, which the request
explicitly ruled out. A lit edge also carries double stroke width and an
arrowhead, so colour is not the only thing marking it — but this is a real
trade of contrast for palette fidelity, made deliberately and recorded here
rather than discovered later. Moon is unaffected.
