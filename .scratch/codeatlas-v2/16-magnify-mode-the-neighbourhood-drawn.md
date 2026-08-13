# Ticket 16 — magnify mode: the neighbourhood, drawn

**Status:** done
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 24 — a magnify mode draws only the focused file and the files it
connects to; 25 — leaving magnify returns the reader where they were
**Blocks:** none
**Blocked by:** 03 (the projection must already take a disclosure set as an
argument), 04 (magnify reveals hidden neighbours through the same mechanism
auto-reveal uses)

## Problem

Focus dims the map in place and lights one file's used-by and uses paths.
On a large map that is a promise the drawing cannot keep: the lit edges run
the full width of the canvas to cards too small to read, so the reader learns
*that* the file connects to something and never *what*. Memnoc hit this on
2026-08-13 focusing `hello.c` on this repository's own map — the neighbours
were at the far edge of a canvas several screens wide.

The drill view answers density by hiding what is not significant. This
answers it by hiding what is not connected. Same principle, different
relevance test.

## What to build

A mode that draws only the focused file and its direct neighbours, laid out
so the neighbourhood is legible — and a way back to where the reader was.

## Acceptance criteria

- [x] Magnifying a file draws that file and its direct neighbours — the files
      it imports and the files that import it — and nothing else.
- [x] The neighbourhood is laid out the way the drill view lays out a region:
      imports run downward, so what the file leans on sits below it and what
      leans on it sits above.
- [x] A neighbour the default view had hidden appears — magnify reveals
      through the same mechanism ticket 04's auto-reveal uses, not a second
      one. (The lens itself writes no reveal at all — see "The two recorded
      decisions" below for how the criterion's two halves are both kept.)
- [x] The magnified set is an argument to the pure projection, never state
      inside it: same map, same focused file, byte-identical positions.
- [x] Leaving magnify returns the reader to the view they came from, with
      their selection intact. It is a lens, not a navigation step.
- [x] A file with no relating edges magnifies to itself alone and says so,
      rather than drawing an empty canvas.
- [x] Escape leaves magnify through the existing cascade — one cascade, never
      a second handler.
- [x] Projection tests cover neighbourhood selection, the layering and
      determinism; a jsdom test covers entering and leaving; each guard proven
      able to fail.
- [x] The share artifact stays under the ceiling (ticket 01): measured
      1,442,328 bytes on 2026-08-13 against the 2,097,152-byte ceiling,
      dashboard bundle rebuilt with the lens included.

## Notes

**Depth is 1, with no control.** Direct neighbours only. Depth 2 pulls in the
neighbours' neighbours, which on this repository's densest files is most of
the region again — the problem the mode exists to solve. If a reader wants
the next hop they magnify the neighbour. A depth knob is speculative
generality until someone asks for it.

**Existing focus stays.** This is a switchable mode beside dim-in-place, not
a replacement: the dimmed view keeps the file's position in the hierarchy
visible, which is information magnify deliberately throws away.

Scope discipline: this draws a neighbourhood. It is not a path explorer, not
a second search, and not a new panel — the info panel already names what a
file touches.

## What was built

`neighbourhoodOf(map, fileId)` and `magnifyFlow(map, magnified, cardHeight,
captionOf)` in `dashboard/src/app/graph.ts`, plus one extraction: the tail of
`fileFlow` — layering, ordering, the parked block, the fan — became the
private `layeredFlow`, which both canvases now call. "Laid out the way the
drill view lays out a region" is therefore the same code, not a promise
between two copies; the existing drill-layout suite passed the extraction
untouched.

In `MapExplorer.tsx` the lens is one piece of state, `magnifiedId`. The
"Magnify {file}" control sits in the breadcrumb beside the reveal affordance
(same `.reveal` class — no new CSS), appears whenever a selected file is not
already the magnified one, and so also offers the next hop: select a
neighbour on the lens and the control reads "Magnify {neighbour}". While
magnified the crumb trail reads region › file, the note counts the
neighbours, and a file with none says "imports nothing and nothing imports
it — drawn alone" over its single card. `backStep` gained the lens as the
innermost rung of the overview → region → file → lens stack, which is how
both the back control and Escape leave it — the cascade's existing last
step, no second handler. Region chips, the overview crumb and a grouping
change all drop the lens: each is about to change the very canvas the lens
covers, and the grouping switch drops it with the selection it lets go of
(ticket 04's asymmetry, kept).

**The two recorded decisions.**

*Imports only.* `calls` is a relating kind too (CONTEXT.md), and this ticket
words the neighbourhood as "the files it imports and the files that import
it". Imports it is: every canvas this dashboard draws relates files by
imports alone — `fileFlow`'s links are imports, and the dim-in-place focus
lights exactly those drawn edges — so the lens agreeing with them means the
two focus modes can never name different neighbours for the same file.
`calls` edges also run symbol-to-symbol, which would need a roll-up rule no
other drawing has, and the info panel already names what a file calls. The
decision is pinned by the projection test "relates files by imports, not
calls", which also holds the honest corollary: a file whose only relating
edge is a call magnifies alone.

*No reveal writes to enter or leave.* The lens draws from the map — the
magnified set handed to `magnifyFlow` never passes the drill view's default
cut — so a hidden neighbour appears because the lens's relevance test is
connectivity, not significance, and nothing is written anywhere. That is
what lets leaving restore the prior view exactly: open region, revealed
set, selection, grouping were never touched. Auto-reveal's mechanism remains
the only reveal path there is, and it still runs where it always ran — a
pointer. Clicking a card on the lens goes through `reveal()` (on the lens a
card may be one the view underneath holds back, or another region's
entirely), which auto-reveals the pointed-at file's region into the same
region-keyed set the affordance toggles, so leaving never lands the reader
on a detail panel describing a card the canvas is not drawing — ticket 04's
own failure mode. A pointer at a file the lens draws keeps the lens; a
pointer at anything else lowers it, because its destination is a canvas the
lens is covering.

Tests: nine projection tests in `regions-and-insights.test.ts` ("the
magnified neighbourhood") and seven jsdom tests in `magnify.test.tsx`
("magnify draws the neighbourhood").

**Proved able to fail** (recorded 2026-08-13). Each guard was broken one at
a time and its test went red with the output quoted; both files were
restored byte-identical afterwards (verified by diff against a pristine
snapshot).

Projection (`graph.ts`):

- Neighbourhood counted `calls` edges too → "relates files by imports, not
  calls" failed: expected true to be false; the membership test also caught
  the fifth card.
- Neighbourhood walked two hops → "stops at one hop — the next hop is one
  more magnify" failed: expected true to be false; membership and induced-
  subgraph tests fell with it.
- Lens links flipped source/target → "imports run downward" failed:
  expected 260 to be less than 130.
- Self-import kept → "draws the induced subgraph" failed: expected 5 edges
  to deeply equal 4.
- Lens deferred to the significance cut (top-40 intersect) → "draws a
  neighbour the default drill view holds back" failed: expected
  ['file:app/f059.ts'] to deeply equal ['file:app/f000.ts', …].
- `magnifyFlow` memoized its first drawing → "takes the magnified set as an
  argument and keeps none of it" failed: expected positions not to be
  byte-identical across different sets.
- Files ordered by the set's insertion order (edge-list order) → "draws the
  same lens however the map orders its edges" failed: reversed edges moved
  cards.

Component (`MapExplorer.tsx`):

- Flow memo ignored the lens → all seven jsdom tests failed, first at
  expected 40 to be 4.
- Entering wrote a reveal (`autoReveal` over the neighbourhood) → "draws the
  hidden neighbour without writing a reveal" failed after leaving: expected
  60 to be 40, the "(20 hidden)" control gone.
- Leaving cleared the selection → "leaves through the back control to
  exactly where the reader was" failed: expected null to be
  'file:wide/f059.ts'.
- `backStep`'s lens rung removed → "leaves through the one Escape cascade"
  failed: expected 4 to be 40 — Escape skipped the lens.
- Alone message dropped → "magnifies a file with no relating edges to
  itself alone, and says so" failed: '0 neighbours — …' did not contain
  'imports nothing and nothing imports it'.
- `reveal()` always lowered the lens → "keeps the lens for a pointer at a
  file it draws" failed: expected 60 to be 4.
- `reveal()` never lowered the lens → "drops the lens for a pointer at a
  file it does not draw" failed: expected 4 to be 3.

Suites on the restored code, 2026-08-13: dashboard 251/251 across 18 files
plus a clean `tsc --noEmit`; `cargo test` all green including the share
ceiling test, artifact measured 1,442,328 bytes against the 2,097,152-byte
ceiling (baseline 1,427,734 measured earlier the same day — the lens and the
map's two new test-file nodes account for the growth).
