# Ticket 22 — the search overlay does not dismiss, and its icon is too small

**Status:** done
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 3 — interactive local dashboard (graph canvas, search, layer grouping, node detail)
**Blocks:** none
**Blocked by:** none

## Problem

Two faults in the search bar, both reported from use on 2026-08-11.

**The results overlay will not go away.** Clicking anywhere outside it — the
canvas, the panel, the header — leaves it open, covering most of the viewport.
The only way out is to clear the query. An overlay that occludes the whole
canvas and captures nothing outside itself is a trap: the reader has to work
out what the app wants before they can carry on.

**The magnifier icon is too small** to read as an affordance next to a
full-width input. It reads as a decoration rather than as the thing that says
"this is search".

## Acceptance criteria

- [x] A click anywhere outside the overlay closes it, leaving the query text
      alone — a reader who clicks the canvas is dismissing the results, not
      abandoning their search.
- [x] `Escape` closes it too, and returns focus to the input.
- [x] Selecting a result closes it, which is already true; keep it true.
- [x] Dismissal does not swallow the click that caused it — clicking a region
      card both closes the overlay and selects that card.
- [ ] The icon is sized to sit with the input rather than beside it; pick the
      size against the rendered bar, not in the abstract. **Changed 16px →
      22px with a pinned line-height and a fixed 24px box, but chosen in the
      abstract: no browser can be driven in this environment, so the rendered
      bar has never been looked at.** Left unticked deliberately — the
      criterion asked for a judgement against the real thing, and that
      judgement is the reader's to make.
- [x] Driven by real user events in the dashboard suite, not by calling the
      handler directly.

## Notes

Watch for the ordinary outside-click bug: a `mousedown` listener on
`document` that fires before React's synthetic `click`, so the overlay closes
and the underlying element never receives the event. The criterion above
exists to catch exactly that.
