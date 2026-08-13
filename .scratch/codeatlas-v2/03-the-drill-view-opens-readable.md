# Ticket 03 — the drill view opens readable

**Status:** done
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

- [x] A region with more than 40 files draws its top 40 by published
      significance, ties broken on path.
- [x] An affordance naming the true remainder ("show all N files") reveals
      them; it is absent when the region holds 40 or fewer.
- [x] The revealed set is an argument to the pure projection, never state
      inside it: same map plus same revealed set gives byte-identical
      positions, asserted.
- [x] A map with no significance still draws — every file ties, path order
      decides, nothing crashes and no region comes out empty.
- [x] Region cards and panels keep reporting the region's true file count.
      The default view hides cards; it never hides facts.
- [x] Forty is a named constant carrying its rationale — the count the V1
      reference material demonstrated readable — not a literal threaded
      through call sites.
- [x] Projection tests cover selection, the tie-break and determinism; a
      jsdom test covers the gesture reaching the revealed state.
- [x] The dashboard's rankings read the published significance rather than
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

## Built

`drawnFiles` in `dashboard/src/app/graph.ts` holds the whole selection rule —
top [`DRILL_DEFAULT_CARDS`] by published significance, ties on path — and the
chosen files keep the order the region lists them in, so a region the default
view draws whole is laid out exactly as it was before this ticket. The revealed
set is `fileFlow`'s fifth argument, a `ReadonlySet` of region IDs: the same
input the manual affordance feeds and the one ticket 04's auto-reveal will feed
too. Nothing about the disclosure lives inside the projection.

The affordance sits in the breadcrumb beside the count it is about and names
both true numbers — `Show all 60 files (20 hidden)` — toggling to
`Show the top 40`, absent entirely on a region of forty files or fewer. The
crumb's "N importing nothing here" now counts the cards actually drawn: a file
the default view is holding back is not importing nothing, it is simply not on
the canvas. Revealing is cleared when the grouping changes, because a layer and
a domain can both be called `crates` and a reveal that survived would open a
region nobody asked for.

`mostDependedOn` is retired for `mostSignificant`, which reads the published
number and ranks on it. **This changes what the info panel names.** On this
repository's own map, scanned 2026-08-13, the old fan-in ranking's top six were
`dashboard/src/index.ts`, `dashboard/src/app/MapExplorer.tsx`,
`crates/codeatlas/src/map.rs`, `dashboard/tests/drive.ts`,
`dashboard/tests/fixtures/small-map.json`, `crates/codeatlas/src/enrich.rs`;
the published ranking's are `MapExplorer.tsx`, `index.ts`, `enrich.rs`,
`docs/adr/README.md`, `docs/specs/2026-08-09-codeatlas-v1.md`,
`crates/codeatlas/src/lib.rs` — and the last three are tour stops the panel
could not previously name. The section is retitled "Files that matter" and its
rows print `significance N` rather than `← N files`, because the number is no
longer a count of importers and labelling it as one would be a lie. A map that
publishes no significance ranks nothing rather than inventing an order.

Guards proven able to fail, each restored afterwards: dropping the `path`
tie-break from the comparator (the reverse-ordered fixture drew f044…f005);
sorting `region.files` in place, and remembering the last revealed set in a
module-level variable (both tripped the purity assertion); requiring
`significance` to be present and removing the small-region early return (a map
without the field drew nothing); dropping the `revealed` argument at the call
site; showing the control on a region drawn whole; printing the drawn-card
count in the crumb instead of the region's; testing `revealed.size > 0` instead
of `revealed.has(region.id)`, which revealed every region at once; not clearing
the reveal on a grouping change; and restoring the pre-ADR-0010 fan-in
derivation inside `mostSignificant`, which inverted the fixture's order to
a.ts, b.ts, c.ts, hub.ts with every count 1.

The self-scan suite gained the story on real data — this repository's `crates`
layer opens at forty cards with one control naming the rest — and its detail
test now clicks a card the canvas is drawing rather than the first file the map
happens to list, which the default view no longer draws.
