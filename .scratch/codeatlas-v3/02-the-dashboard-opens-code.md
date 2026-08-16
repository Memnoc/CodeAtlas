# 02 — The dashboard opens code

**Spec:** `docs/specs/2026-08-16-codeatlas-v3.md` — a fresh session reads
this first, then ADR-0013.

**What to build:** Selecting a node offers opening its code; the source
appears beside the map (plain text until ticket 03), a symbol opens
scrolled and lit at its own lines, truncation is announced visibly, and
the affordance simply does not exist when the capabilities route says
open code is off.

**Blocked by:** 01 — The source route behind the flag.

**Status:** ready

- [ ] An open affordance appears wherever a node is already selected
      (drill view, magnify, the panels) exactly when capabilities says
      open code is on; its absence when off is asserted, not assumed
- [ ] Opening a file node renders its source without losing the map or
      the current selection
- [ ] Opening a symbol resolves to its containing file client-side and
      lands scrolled with the symbol's `range` lit — the contract already
      carries the range; the wire speaks file nodes only
- [ ] A truncated envelope renders its notice visibly; a 404 (deleted
      file) reads as an honest message, never an empty panel
- [ ] Gesture→state covered at the jsdom seam; geometry goes to the
      stylesheet contract only if the view needs pinning (prior art: the
      conversation column's pass³ split)
- [ ] Full dashboard suite and typecheck green
