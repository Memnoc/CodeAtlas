# Ticket 16 — magnify mode: the neighbourhood, drawn

**Status:** ready
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

- [ ] Magnifying a file draws that file and its direct neighbours — the files
      it imports and the files that import it — and nothing else.
- [ ] The neighbourhood is laid out the way the drill view lays out a region:
      imports run downward, so what the file leans on sits below it and what
      leans on it sits above.
- [ ] A neighbour the default view had hidden appears — magnify reveals
      through the same mechanism ticket 04's auto-reveal uses, not a second
      one.
- [ ] The magnified set is an argument to the pure projection, never state
      inside it: same map, same focused file, byte-identical positions.
- [ ] Leaving magnify returns the reader to the view they came from, with
      their selection intact. It is a lens, not a navigation step.
- [ ] A file with no relating edges magnifies to itself alone and says so,
      rather than drawing an empty canvas.
- [ ] Escape leaves magnify through the existing cascade — one cascade, never
      a second handler.
- [ ] Projection tests cover neighbourhood selection, the layering and
      determinism; a jsdom test covers entering and leaving; each guard proven
      able to fail.
- [ ] The share artifact stays under the ceiling (ticket 01).

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
