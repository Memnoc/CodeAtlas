# Ticket 39 — folding the frame away

**Status:** done
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** *none* — see "What this ticket is missing", below
**Blocks:** none
**Blocked by:** none
**Filed:** 2026-08-12, from a request made while looking at the running
dashboard
**Scope:** V1 — decided 2026-08-12 by the user, mid-session

## Problem

Requested 2026-08-12: *"let's add the ability to the sidebars to collapse. In
fact, all the ux should be able to collapse leaving space to the navigation as
much as we can."*

The map is the thing worth looking at and the frame around it is not small. On
a 1280×600 window the side panel is 360px of the width — 28% of it — and the
region chips wrap to two or three rows on a repository with many regions. Both
are useful while being used and pure cost the rest of the time.

## What was built

Two folds and a control that does both.

- **The side panel** folds to a 30px rail carrying one button: the way back.
- **The region chips** fold, leaving the row. The row keeps the count, the
  fold control and the diff overlay toggle.
- **Focus**, in the top bar, folds both at once and restores both. It reads as
  pressed while *anything* is folded, whichever control did the folding.

What is folded is remembered in `localStorage` under `codeatlas-chrome`.

Two rules shaped it, and both are asserted in `dashboard/tests/fold.test.tsx`:

- **Folded means unmounted, not hidden.** The interface walkthrough resolves
  its steps against the elements on the page, so a panel hidden with CSS would
  still be found and then spotlighted as a hole of no size. Unmounting makes
  the walkthrough skip the step, which is what it already does for a control a
  given page lacks.
- **Nothing folds away the only way to do something.** The diff overlay toggle
  lives among the chips but is not one, so it stays when they go. The search
  field and the top bar do not fold at all — search is how a reader finds a
  node they cannot see, which makes it the last thing that should disappear
  when they ask for more map.

A thirteenth walkthrough step (`focus`) names the new control, because a
walkthrough that does not mention the thing that hid the panel is a
walkthrough that leaves a reader stuck.

## Acceptance criteria

- [x] The side panel folds and unfolds, and the folded state carries a visible
      way back.
- [x] The region chips fold and unfold without taking the diff overlay toggle
      with them.
- [x] One control folds and restores both, and reports itself as pressed after
      a fold made with either of the other two.
- [x] The fold survives a reload; anything unreadable in storage opens
      everything, because a reader who cannot see the panel and does not know
      it exists cannot ask for it back.
- [x] Folding unmounts rather than hides, so the walkthrough skips a folded
      step instead of spotlighting nothing.
- [x] Driven by real user events; every guard tampered and seen to fail.

## What this ticket is missing, and it matters

**There is no story in the spec for this.** It was asked for while looking at
the running dashboard, after the spec was written and after every other ticket
was `done`. `/harden` walks the spec's numbered User Stories; a feature with no
story is not on that list and will not be walked.

Two honest options, and the choice is the reader's:

1. **Add a story** to the spec's User Stories section and let `/harden` walk it
   like the rest. This is the pipeline working as designed — upstream changes,
   then downstream verifies.
2. **Accept it as unverified V1 scope**, named in the `## Verification`
   section alongside the unverifiables, so nobody later mistakes silence for a
   pass.

**Also unverified by any test here: that anything actually got bigger.** jsdom
lays nothing out, so the canvas is zero pixels wide folded and zero pixels wide
open — a test comparing the two would pass over a fold that did nothing. The
width lives in `grid-template-columns`, in a stylesheet the suite never loads.
This is the same limit that hid two walkthrough clipping bugs (`db14ced`,
`64b442b`) until somebody looked at the screen. Look at the screen.
