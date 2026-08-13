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
- [ ] Curve styling matches the spread, so a fan reads as a fan rather than
      as crossed lines that happen to start apart. Left unticked 2026-08-13:
      no curve styling was changed — the per-edge alternatives React Flow
      ships were evaluated on measured geometry and every one came out
      identical or worse than the default (see "On curve styling" below), so
      bending the curves any further needs a custom edge component, which is
      a visual change a human must look at first.
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

**On "curve styling".** Two facts, replacing an earlier claim here that "there
is no per-edge curve knob to turn" — which was false. First, the default
bezier's `pathOptions.curvature` is dead on every downward import:
`calculateControlOffset` (`@xyflow/system` dist) returns `0.5 * distance`
whenever the target is ahead of the source, ignoring curvature — verified
2026-08-13 both in that source and by measurement (curvature 0.25 and 0.9
produce byte-identical downward paths). Second, per-edge knobs do exist:
`edge.type` selects among React Flow's built-in edge types — `straight`,
`step`, `smoothstep` (with `pathOptions.offset`/`borderRadius`),
`simplebezier` — per edge, no custom component, no new dependency.

Those built-ins were evaluated 2026-08-13 on geometry computed from
`fileFlow`'s own output (a six-edge fan leaving one 200px card, source
anchors 34.4px apart, targets 218px apart; the 24-edge past-capacity fan;
and a stacked two-cycle for the upward case), sampling each candidate's SVG
path and measuring the distance between adjacent strands along their length:

- `simplebezier` — rejected. Its path string is byte-identical to the
  default's on every downward edge (6/6 on the six-fan, 24/24 on the
  24-fan): both put their control points at the vertical midpoint, so it
  cannot change a single pixel of the dominant case. On the upward
  cycle-closing edge it penetrates a card body 28.6px against the default's
  28.3px — no better there either.
- `smoothstep` — rejected. Every strand between two rows runs its horizontal
  segment on the same rail (y = 94 for all six strands of the six-fan), so
  adjacent strands close to within 0.09px (offset 20, radius 5) or 0.02px
  (offset 40, radius 16) against the default's 10.30px minimum — the spread
  anchors converge back into one line, the very defect this ticket removed.
  The rail sits at the midpoint of the gapped rows whatever `offset` is, so
  no parameter choice moves it. On the 24-fan, strands on distinct points:
  0.07px against the default's 1.14px.
- The default bezier keeps adjacent strands at least 10.30px apart along the
  whole six-fan and 1.14px on the 24-fan, without crossing: the anchor
  ranking makes the left-to-right order agree at both ends, and separation
  in between is a blend of the two end gaps.

So the default stays, on the numbers rather than on a shrug: what styles the
curve here is the pair of anchors it runs between and the side each sits on,
both per-edge, plus the ranking that keeps the fan from crossing itself. If
the fan ever needs more bend than that, the escalation is a custom edge
component computing its own path — a visual change a human must look at
first. (The upward cycle-closing edge on a stacked two-cycle threads through
card bodies under every built-in type alike, 26.7–28.7px deep, so that too
waits on such a component, not on an `edge.type`.)

Handles are painted no longer (`.react-flow__handle` in `styles.css`): a busy
card exposes up to 28 of them on this repository's own map, and React Flow's
default 6px dot at that density draws a dotted rule under the card.

**Proved able to fail** (recorded 2026-08-13). Each guard was broken one at a
time at build time and its test went red with the output quoted:

- Forced every side to a single point → "gives every edge leaving one card
  its own point" failed: expected 1 to be 6.
- Ranked by edge ID instead of by where the other end was drawn → "ranks the
  points by where the other card is drawn, so the fan does not cross"
  failed: the line to the card at 536 leaves left of the one at 318:
  expected 232 to be greater than 404.
- Capacity clamp removed → "repeats points in ascending runs once the card
  is full, never off it" failed: expected 24 to be 15.
- Repetition rewritten as `k % capacity` → the same test failed: the fan
  went backwards, so two of its lines cross: expected 14 to be greater than
  or equal to 186.
- Ranking dropped while the map's edge list arrived reversed → the
  determinism test failed: anchors s8…s0 where s0…s9 was expected.
- `regionFlow`'s fan pass skipped → "spreads the overview's region cards"
  failed: region:r0 exposes no point named undefined.
- Card rendering a single handle only → the render suite failed: expected
  [['s0',14]] to deeply equal [['s0',14],['s1',71],…].
- `.react-flow__handle` rule absent → the stylesheet contract test went red.

Re-verified 2026-08-13 by re-running two of these mutations against the
committed suite: ranking by edge ID reproduced "the line to the card at 536
leaves left of the one at 318: expected 232 to be greater than 404", and
removing the capacity clamp reproduced "expected 24 to be 15". The source
was restored byte-identical afterwards.
