# Ticket 17 — the conversation, beside the map

**Status:** ready
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 26 — the conversation in a column beside the canvas rather than a
band above it, so the map stays on screen while the reader reads what it
answered
**Blocks:** none
**Blocked by:** 09 — there is no thread to move until the thread exists

## Problem

The answer panel V1 built is a band between the search bar and the canvas,
sized for one answer at full viewport width. Ticket 09's thread stacks turns
into that band, so every exchange pushes the map further off the screen —
Memnoc walked it on the live map on 2026-08-13 and the canvas below the
answer was mostly empty dark. Prose at full viewport width is hard to read,
the citation chips sprawl, and a citation click highlights a card the reader
cannot see. The reader chooses between the conversation and the thing it
describes, which is the choice the citations exist to remove.

## What to build

The conversation moves to a column docked beside the canvas: bounded width,
thread scrolling internally, canvas keeping the remainder and staying
interactive. The behaviour ticket 09 built — thread, usage lines, running
total, dismiss, client-side turn bound — moves unchanged.

## Acceptance criteria

- [ ] The conversation renders in a column beside the canvas; the canvas
      stays visible and interactive while the column is open.
- [ ] The column's width is bounded and the thread scrolls inside it; a
      six-turn conversation never changes the canvas's size mid-read.
- [ ] A citation click selects and reveals its node on the canvas the reader
      is looking at — the column and the canvas work together, asserted in
      one test that clicks a citation and finds the card drawn.
- [ ] One question or six, the conversation lives in the same place: the
      single-question reader gets the column too, not a special case.
- [ ] Everything ticket 09 tested still passes with its assertions pointed
      at the column: thread growth, per-turn usage, running total and its
      honesty rules, "Dismiss conversation", the 6-turn wire bound. Moved
      assertions are edits with a stated reason, not deletions.
- [ ] The newest exchange is visible when it arrives — the column scrolls to
      it — and reading an older turn is never interrupted by scroll theft
      mid-read.
- [ ] Escape leaves through the existing cascade, one rung, no second
      handler; the walkthrough, if any step anchors on the answer panel,
      still points at something that exists.
- [ ] Keyboard focus and the focus-return rule survive: opening the column
      does not seize focus from the search box; closing it returns focus per
      the existing `useFocusReturn` discipline.
- [ ] jsdom gesture→state tests cover open, thread-in-column, citation→card,
      dismiss; geometry (bounded width, internal scroll) rides the
      stylesheet-contract pattern. Each guard proven able to fail, record in
      the ticket, dated.

## Notes

This is a move, not a redesign: the thread's DOM and copy move largely as
they are; what changes is where the panel docks and how it sizes. If the
move tempts a broader restyle of the panel's contents, that is scope creep —
file it.

The column competes for the right edge with nothing today (the info panel
docks left). If the implementation finds a real collision — share banner,
export menu — resolve it by stacking order through the existing cascade and
record the choice, not by inventing a new layer rule; the three walkthrough
placement bugs of V1 were all stacking-context inventions.

Width: bounded, not fixed — a named constant with its reason, responsive
below it. The V1 reference material's side panels ran ~360–400px; pick
inside that band and say why in the code.
