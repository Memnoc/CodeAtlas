# Ticket 03 — the drill view opens readable

**Status:** ready
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 1 — drilling into a large region shows the files that matter
first; 2 — one affordance reveals the remainder; 4 — the revealed state is an
input to the pure projection; 6 (the rankings clause) — the tour, the default
drill view and the rankings agree by construction
**Blocks:** 04 (auto-reveal needs something to reveal), 05
**Blocked by:** 02 — the selection reads the published significance

## Problem

Drilling into this repository's own `crates` region draws 159 file cards at
once. The layout work in V1 made that drawing as good as an ordered layered
layout can make it, and it is still not a picture anyone reads: the region
where the code is densest is the region where the map stops helping.

The fix is disclosure, not layout. Show the files that carry the region;
keep the rest one gesture away.

## What to build

The default drill view of a region is its top 40 files by published
significance; one affordance reveals the rest. The revealed set is a
parameter of the projection, so the same map in the same state always draws
the same picture.

## Acceptance criteria

- [ ] A region with more than 40 files draws its top 40 by published
      significance, ties broken on path.
- [ ] An affordance naming the true remainder ("show all N files") reveals
      them; it is absent when the region holds 40 or fewer.
- [ ] The revealed set is an argument to the pure projection, never state
      inside it: same map plus same revealed set gives byte-identical
      positions, asserted.
- [ ] A map with no significance still draws — every file ties, path order
      decides, nothing crashes and no region comes out empty.
- [ ] Region cards and panels keep reporting the region's true file count.
      The default view hides cards; it never hides facts.
- [ ] Forty is a named constant carrying its rationale — the count the V1
      reference material demonstrated readable — not a literal threaded
      through call sites.
- [ ] Projection tests cover selection, the tie-break and determinism; a
      jsdom test covers the gesture reaching the revealed state.
- [ ] The dashboard's rankings read the published significance rather than
      deriving their own. `mostDependedOn` currently computes a private
      fan-in ranking that excludes self-imports, while the published formula
      counts them — so the same file can rank differently in the panel than
      it does in the tour. After this ticket, no consumer re-derives the
      number. A test pins the agreement, and is proven able to fail by
      restoring a private derivation.

## Notes

The affordance is a disclosure control, not a filter: revealing is
region-scoped and does not survive as a global preference. Reading a region
in full is a thing someone does once, deliberately.

Do not add a knob for the count. The open question is recorded in the spec's
Further Notes and answered "not now" — a setting nobody asked for is
speculative generality.
