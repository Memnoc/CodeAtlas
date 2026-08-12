# Ticket 41 — the fold has no story to walk

**Status:** done — 2026-08-12, resolved by option 1
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** *this ticket is about the absence of one*
**Blocks:** ~~`/harden`~~ — cleared 2026-08-12
**Blocked by:** none
**Filed:** 2026-08-12, from ticket 39's write-up

## Problem

Ticket 39 (`74f5c3e`) shipped the fold: the side panel folds to a rail, the
region chips fold, and a Focus control does both. It is built, tested and
committed, and it reads `done`.

**`/harden` will not walk it.** Harden builds its checklist from the spec's
numbered User Stories section. Ticket 39 was asked for on 2026-08-12 while
looking at the running dashboard — after the spec was written and after every
other ticket was already `done` — so there is no story for it, and a feature
with no story is not on the list.

Ticket 39 records this in its own body, but ticket 39 reads `done`. Anyone
scanning statuses to see what is outstanding sees nothing. That is why this
is a separate, open ticket: an unmade decision that is invisible is an
unmade decision that gets shipped past.

## This is a decision, not code

Nothing here is implemented by `/implement`. The pipeline's rule is that
downstream never edits upstream, so amending the spec is the spec author's
call and not a ticket a build agent can execute. What this ticket is for is
making sure the call gets made *before* `/harden` runs, rather than being
discovered by its absence afterwards.

## The two options

**1. Add a story to the spec.** Amend the User Stories section of
`docs/specs/2026-08-09-codeatlas-v1.md` with a story covering the fold, then
run `/harden` and let it walk it like the other twenty-one. This is the
pipeline working as designed: upstream changes, downstream verifies.

**2. Accept it as unverified V1 scope.** Name it in the spec's
`## Verification` section alongside the unverifiables, with the user's name
against it, so nobody later mistakes silence for a pass.

Either is defensible. What is not defensible is neither.

## Resolved: option 1, 2026-08-12

The user chose to add the story. **Story 22** now exists in
`docs/specs/2026-08-09-codeatlas-v1.md`, so `/harden` will walk the fold like
the other twenty-one.

A second gap surfaced while closing this and is filed as **ticket 44**: the
`serve` question-hint (`25e12a1`) had shipped with no ticket and no story
either, and nobody had noticed. **Story 23** covers it. Two features had
shipped unmentioned; the reason this ticket was worth filing open rather than
noting inside ticket 39 is that it was the thing that made anyone go looking
for the second one.

## Acceptance criteria

- [x] One of the two options above is chosen by the user, by name.
- [x] If option 1: the story exists in the spec's User Stories section and
      `/harden` walks it.
- [ ] ~~If option 2~~ — not taken.

## Notes

**There is a second gap inside this one, and option 1 does not close it.**
No test in the dashboard suite can show that folding made the canvas bigger —
jsdom lays nothing out, so the canvas is zero pixels wide folded and zero
pixels wide open, and the width lives in `grid-template-columns` in a
stylesheet the suite never loads. `tests/fold.test.tsx` says so at the top and
asserts only what is observable: which controls exist afterwards, that nothing
folds away the only way to reach something, and that the fold is remembered.

So even a story that `/harden` walks gets verified **by eye**, like stories 3,
6, 7 and 8. That is not an argument against writing the story — a walked story
with a human verdict is worth much more than an unlisted feature — but the
verdict will say `pass (by eye)` and should say so.

This is the same limit that hid two walkthrough clipping bugs (`db14ced`,
`64b442b`) through 170 green tests until somebody looked at the screen.
