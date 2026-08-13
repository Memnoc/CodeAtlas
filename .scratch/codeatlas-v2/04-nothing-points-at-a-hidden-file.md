# Ticket 04 — nothing points at a hidden file

**Status:** ready
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

- [ ] A search hit on a hidden file reveals it before highlighting it.
- [ ] Focusing a hidden file reveals it before jumping to it.
- [ ] A tour stop on a hidden file reveals it before narrating it.
- [ ] The diff overlay reveals every hidden file it marks.
- [ ] Auto-reveal feeds the same projection input the manual affordance
      does — one mechanism, not a second path with its own bugs.
- [ ] Revealing for one of these never resets a manual "show all", and never
      silently reverts mid-visit.
- [ ] One test per entry point, each proven able to fail: hide the target,
      remove the reveal, watch the feature point at nothing.

## Notes

Prefer revealing the whole region over revealing one card. A single extra
card appearing among 40 is a card with no neighbours and no context; the
reader followed a pointer into a region and is entitled to see the region.
If the implementation finds a reason to prefer the narrower behaviour, record
it in the ticket rather than choosing silently.
