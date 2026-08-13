# Ticket 05 — a fan rather than a knot

**Status:** ready
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

- [ ] Each edge attaches at its own point along the card edge; two edges on
      one card do not share a point while spread capacity remains.
- [ ] Beyond capacity the degradation is stated and deterministic — points
      repeat in a defined order rather than overflowing the card or
      collapsing back to the centre.
- [ ] Spreading is computed in the pure projection and is assertable as
      geometry in the projection suite.
- [ ] Determinism holds: same map, same state, byte-identical positions and
      anchors.
- [ ] Curve styling matches the spread, so a fan reads as a fan rather than
      as crossed lines that happen to start apart.
- [ ] The share artifact stays under ticket 01's ceiling — this is the
      visualization ticket most likely to move it.
- [ ] No new dependency (ADR-0006); no layout library (ADR-0011).

## Notes

Anchor assignment must be a function of the edge and the card, not of
iteration order over a hash map — that is the usual way determinism dies in
this kind of change, and the projection suite's byte-identical assertion is
what catches it.

Both the overview and the drill view draw cards. Spread both, or state why
one is left alone.
