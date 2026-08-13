# Ticket 04 — nothing points at a hidden file

**Status:** done
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 3 — search hits, focus, tour stops and the diff overlay auto-reveal
a file the default view hid, so that a feature that points at a file never
points at nothing
**Blocks:** none
**Blocked by:** 03 — there is nothing to reveal until the default view hides
something

## Problem

The moment the drill view hides files by default, four existing features
acquire a silent failure mode. Search finds a file and highlights nothing.
The tour walks to a stop that isn't drawn. Focus jumps to an absent card. The
diff overlay marks changed files that the reader cannot see. Each fails
quietly — the worst kind of failure, because the reader concludes the feature
is broken or, worse, that the file is fine.

Auto-reveal is therefore a requirement of ticket 03's disclosure, not a
nicety layered on top.

## What to build

Any feature that points at a specific file reveals that file first.

## Acceptance criteria

- [x] A search hit on a hidden file reveals it before highlighting it.
- [x] Focusing a hidden file reveals it before jumping to it.
- [x] A tour stop on a hidden file reveals it before narrating it.
- [x] The diff overlay reveals every hidden file it marks.
- [x] Auto-reveal feeds the same projection input the manual affordance
      does — one mechanism, not a second path with its own bugs.
- [x] Revealing for one of these never resets a manual "show all", and never
      silently reverts mid-visit.
- [x] One test per entry point, each proven able to fail: hide the target,
      remove the reveal, watch the feature point at nothing.

## Notes

Prefer revealing the whole region over revealing one card. A single extra
card appearing among 40 is a card with no neighbours and no context; the
reader followed a pointer into a region and is entitled to see the region.
If the implementation finds a reason to prefer the narrower behaviour, record
it in the ticket rather than choosing silently.

## What was built

The whole region, as preferred — no reason to narrow it appeared, and one
argued against it: the revealed set the "show all" control writes is
region-keyed, so answering in regions is what keeps auto-reveal on the single
projection input instead of adding a second.

`regionsHiding(regions, fileIds)` in `dashboard/src/app/graph.ts` names the
regions whose default drill view holds any of those files back, read off
`drawnFiles` — the same rule the canvas draws by, so it can neither reveal a
region that was already showing the target nor miss one that was not. It is
pure and takes no state, as ADR-0011 requires of everything on this seam.
`MapExplorer`'s `autoReveal` writes its answer into the same `revealed` set
the affordance toggles, additively; `reveal()` calls it for search, the tour
and every panel that points the canvas at a file, and an effect calls it for
the diff overlay's marks — across the whole map, because the overlay names
files the reader has not drilled into yet, and re-applied after a grouping
change clears the set.

Proof of failure, with the reveal removed (2026-08-13): the search hit, the
FILES-tab row and the tour stop each left `file:wide/f000.ts` off the canvas
with nothing selected, and the diff overlay reported the file it had marked
as `not drawn`.

Repaired after review (2026-08-13): the grouping change was asymmetric. It
cleared the revealed set — correct, a layer ID and a domain ID can collide —
but kept the selection, and the one-shot reveal that had put the selected
card on the canvas does not re-fire. A reader who searched a hidden file,
regrouped and drilled back to it met a detail panel describing a card the
canvas was no longer drawing: story 3's own failure, arriving by the back
door. **Let-go**, not re-apply: the switch already discards where the reader
was standing, so the selection goes with the open region and the reveal that
served it. Re-apply was the other defensible shape and was rejected — it
would need a second, state-driven write into `revealed` that fires on every
change of `regions`, which is the mechanism that re-opens a region the reader
has just collapsed, and it would still leave the selection invisible until
they drilled back in. Two tests now hold the line, each proven able to fail
by breaking the behaviour first: the asymmetry itself, and the
collapse-stays-collapsed the overlay's effect had only ever been checked by
hand.

Also un-bent: `shows detail for a real node from the self-scan` had been
rewritten in ticket 03 to take whichever card the canvas happened to be
drawing, dodging this gap. It chooses off the map again — the first crates
file the scan emits, `crates/codeatlas/Cargo.toml`, significance 0 and so
below the cut — and fails without the reveal.
