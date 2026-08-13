# Ticket 05 — a fan rather than a knot

**Status:** done
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 5 — a well-connected file's edges land at spread points along its
card, so that it reads as a fan rather than a knot
**Blocks:** none
**Blocked by:** 03 — same projection module; the drill view lands first so the
anchors are written against the final shape

## Problem

Every card exposes one attachment point per side, so every edge touching a
well-connected file converges on the same pixel. The diagnosed cause of the
visual mush is not the layout — V1 shipped a layered layout with cycle
cutting, barycentre sweeps and transpose rounds — it is that twelve lines all
arrive at one point and become one thick smear that says nothing about which
file connects to which.

This is why no layout library was needed and none was taken
([ADR-0011](../../docs/adr/0011-no-layout-library-a-share-ceiling-enforces-it.md)):
the defect lives in the projection's anchors, where an engine would not have
touched it.

## What to build

Per-edge anchor points, spread deterministically along the card edge, with
curve styling to match.

## Acceptance criteria

- [x] Each edge attaches at its own point along the card edge; two edges on
      one card do not share a point while spread capacity remains.
- [x] Beyond capacity the degradation is stated and deterministic — points
      repeat in a defined order rather than overflowing the card or
      collapsing back to the centre.
- [x] Spreading is computed in the pure projection and is assertable as
      geometry in the projection suite.
- [x] Determinism holds: same map, same state, byte-identical positions and
      anchors.
- [x] Curve styling matches the spread, so a fan reads as a fan rather than
      as crossed lines that happen to start apart. See the note below on what
      "curve styling" turned out to mean.
- [x] The share artifact stays under ticket 01's ceiling — this is the
      visualization ticket most likely to move it.
- [x] No new dependency (ADR-0006); no layout library (ADR-0011).

## Notes

Anchor assignment must be a function of the edge and the card, not of
iteration order over a hash map — that is the usual way determinism dies in
this kind of change, and the projection suite's byte-identical assertion is
what catches it.

Both the overview and the drill view draw cards. Spread both, or state why
one is left alone.

## What was built

`dashboard/src/app/anchors.ts` — a new pure module, no dependency added.

**The rule.** Within one side of one card the edges are ranked by where their
*other* end was drawn, ties broken on edge ID by the producer's string order
(`byPath`, the comparator `significance.ts` already publishes). The card
offers points evenly spread between insets 14px from each corner; a side with
one edge keeps the card's centre. Capacity is how many points fit 12px apart
— fifteen on a 200px file card, seventeen on a 226px region card.

**Beyond capacity.** Edge `k` of `n` takes point `floor(k · used / n)`, so
points repeat in ascending runs. Neighbouring edges in the fan share a point;
nothing is drawn past the card's corner and nothing collapses to the centre.

**On "curve styling".** There is no per-edge curve knob to turn. React Flow's
bezier takes `pathOptions.curvature`, but `calculateControlOffset` ignores it
whenever the target is ahead of the source — which is every downward import,
the dominant case in a layered drawing — and uses half the span instead. So
what styles the curve here is the pair of anchors it runs between and the
side each sits on, both now per-edge, plus the ranking that keeps the fan from
crossing itself. Bending the curves any further would need a custom edge
component computing its own path; that is a visual change nobody has looked
at, and it was left out rather than guessed at.

Handles are painted no longer (`.react-flow__handle` in `styles.css`): a busy
card exposes up to 28 of them on this repository's own map, and React Flow's
default 6px dot at that density draws a dotted rule under the card.
