# Ticket 17 — the conversation, beside the map

**Status:** done
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

- [x] The conversation renders in a column beside the canvas; the canvas
      stays visible and interactive while the column is open.
- [x] The column's width is bounded and the thread scrolls inside it; a
      six-turn conversation never changes the canvas's size mid-read.
- [x] A citation click selects and reveals its node on the canvas the reader
      is looking at — the column and the canvas work together, asserted in
      one test that clicks a citation and finds the card drawn.
- [x] One question or six, the conversation lives in the same place: the
      single-question reader gets the column too, not a special case.
- [x] Everything ticket 09 tested still passes with its assertions pointed
      at the column: thread growth, per-turn usage, running total and its
      honesty rules, "Dismiss conversation", the 6-turn wire bound. Moved
      assertions are edits with a stated reason, not deletions.
- [x] The newest exchange is visible when it arrives — the column scrolls to
      it — and reading an older turn is never interrupted by scroll theft
      mid-read.
- [x] Escape leaves through the existing cascade, one rung, no second
      handler; the walkthrough, if any step anchors on the answer panel,
      still points at something that exists.
- [x] Keyboard focus and the focus-return rule survive: opening the column
      does not seize focus from the search box; closing it returns focus per
      the existing `useFocusReturn` discipline.
- [x] jsdom gesture→state tests cover open, thread-in-column, citation→card,
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

## What was built

A move, not a redesign: `AnswerPanel`'s DOM and copy are untouched apart
from the scroll seam below. What changed is where it mounts and how it
sizes.

- `dashboard/src/app/MapExplorer.tsx` — the panel moved from between the
  search row and the chip row into the workspace, as the canvas's next
  sibling. The Escape cascade is byte-for-byte the same handler, same rung
  (answer after path panel, before the back step): the rung is about what
  dismissal costs, not where the panel docks. Focus: the column is wired to
  `useFocusReturn`, its return target the search input (a second ref merged
  onto the field), so dismissing from inside the column puts the keyboard
  back where the next question starts — and a reader who parked focus
  elsewhere is left there, per the hook's own guard. Opening never took
  focus and still does not.
- `dashboard/src/app/styles.css` — `.workspace`/`.workspace-folded` gain an
  `auto` third track that collapses to nothing while no question is open;
  `.answer` becomes the column: `--conversation-column: 380px` (the named
  constant — inside the 360–400 reference band; ~55ch of the 13px answer
  prose, and a 1440px viewport keeps ~700px of canvas beside the 360px
  info panel), `width: min(var(--conversation-column), 38vw)` for the
  responsive floor, `overflow-y: auto` for the internal scroll,
  `border-left` instead of `border-bottom` for the new dock edge. No
  z-index, no position: the export menu (30) and walkthrough (100) keep
  painting over it through the existing order — no right-edge collision
  existed, none was resolved, no layer rule invented.
- `dashboard/src/app/AnswerPanel.tsx` — the one behavioural addition, the
  **autoscroll rule, recorded**: the column follows the conversation only
  for a reader already pinned to the bottom of it (within
  `PINNED_SLACK_PX = 40` of the bottom edge — about two lines of prose, so
  a trackpad flick still counts). A fresh column starts pinned, every
  scroll gesture re-decides it, and an arriving answer never yanks a reader
  who scrolled up to re-read an older turn. Dismissal resets the pin, so
  the next conversation starts at its own bottom.
- `dashboard/src/app/walkthrough.ts` / `tests/walkthrough.test.tsx` — prose
  only ("band" → "column", "directly inside `.explorer`" → "inside the
  workspace"). The `data-walkthrough="answer"` marker moved with the panel
  and `WALKTHROUGH_TRANSIENT` still names it: no walkthrough step anchors
  the answer panel, so nothing points at a missing element — asserted by
  the unchanged walkthrough suite.

**Ticket 09's assertions: zero edits.** Every conversation/ask/walkthrough
test finds the thread by `aria-label`, role, or text — none by position —
so thread growth, per-turn usage, the running total and its honesty rules,
"Dismiss conversation", the 6-turn wire bound, and the three Escape-cascade
tests all pass against the column exactly as written for the band.

New tests: `tests/conversation.test.tsx` gains a story-26 block — column
beside a still-drawn canvas, single-question reader gets the same column,
citation click draws the card beside the open column, open leaves focus in
the search box, dismiss returns focus to it, autoscroll-when-pinned,
no-scroll-theft. `tests/stylesheet-contract.test.ts` gains a story-26 block
— width bounded 360–400 as the named constant, internal scroll, the `auto`
workspace track, no stacking context on `.answer`.

## Proved able to fail (recorded 2026-08-14)

Born red first, 08:04, against the pre-move sources: 7 of the 11 new tests
failed — column-beside-canvas ("expected <section class="answer">.parentElement
to be .workspace"-shaped placement failures for open and single-question),
focus return on dismiss (toHaveFocus on the search input failed),
autoscroll-when-pinned ("expected 700 to be 1000"), and three stylesheet
guards (no `--conversation-column`, no `overflow-y`, `.workspace` columns
ended at `1fr`). The four that pass against today's sources were each
broken one at a time, the red output quoted, and the source restored
byte-identical (diff-verified against pre-mutation snapshots):

1. Citation button's `onClick` made a no-op → "draws the cited card on the
   canvas beside the open column" failed: "expected null to be
   'file:src/main.ts'".
2. `autoFocus` added to the dismiss control → "leaves focus in the search
   box when the column opens" failed: "Expected element with focus:
   <input aria-label=\"Search nodes\" …>".
3. The `pinned` check dropped from the scroll effect → "never steals the
   scroll from a reader partway up an older turn" failed: "expected 1000
   to be 100".
4. `z-index: 31` added to `.answer` → "invents no stacking context" failed:
   "expected '…' not to match /z-index/"; separately `position: relative`
   → same test: "not to match /position:/".
5. `--conversation-column: 480px` → "bounds the column inside the reference
   band" failed: "expected 480 to be less than or equal to 400".

Suites, measured 2026-08-14 after every mutation was restored: dashboard
`npm test` 272 passed / 0 failed (19 files; 261 before this ticket) and
`npm run typecheck` clean; `cargo test --workspace` 255 passed / 0 failed.
Share ceiling (ADR-0011): the artifact weighed 1,516,812 bytes against the
2,097,152-byte ceiling on this repository's own map, measured 2026-08-14
after the column rode into the embedded dashboard (baseline 1,507,645 on
2026-08-13; the move cost 9,167 bytes).

**Residual, stated:** the pinned threshold (40px) is a judgment call
recorded where it lives; a reader parked between 0 and 40px from the
bottom is treated as following along. And the column inherits the band's
deliberate absence of a walkthrough step — a reader who never asks a
question never sees it, which is the transient-marker rule working as
designed.
