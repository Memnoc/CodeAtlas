# 02 — The dashboard opens code

**Spec:** `docs/specs/2026-08-16-codeatlas-v3.md` — a fresh session reads
this first, then ADR-0013.

**What to build:** Selecting a node offers opening its code; the source
appears beside the map (plain text until ticket 03), a symbol opens
scrolled and lit at its own lines, truncation is announced visibly, and
the affordance simply does not exist when the capabilities route says
open code is off.

**Blocked by:** 01 — The source route behind the flag.

**Status:** done

- [x] An open affordance appears wherever a node is already selected
      (drill view, magnify, the panels) exactly when capabilities says
      open code is on; its absence when off is asserted, not assumed
      — the offer lives on the Node detail panel, which is where every
      selection surfaces in all three contexts, and each context has its
      own test; the absence-when-off and absence-in-share tests were
      proven able to fail by forcing the gate open in `App` (both
      tripped), then reverted
- [x] Opening a file node renders its source without losing the map or
      the current selection — docked as a workspace column beside the
      canvas, the conversation column's own shape; proven able to fail
      by making opening clear the selection (four tests tripped)
- [x] Opening a symbol resolves to its containing file client-side and
      lands scrolled with the symbol's `range` lit — the contract already
      carries the range; the wire speaks file nodes only — request
      equality pins `GET /api/source?id=file%3A…` from a symbol
      selection; proven able to fail three ways (encoding dropped,
      range off by one, landing scroll removed — each tripped)
- [x] A truncated envelope renders its notice visibly; a 404 (deleted
      file) reads as an honest message, never an empty panel — both
      proven able to fail (notice inverted, alert removed — each tripped)
- [x] Gesture→state covered at the jsdom seam; geometry goes to the
      stylesheet contract only if the view needs pinning (prior art: the
      conversation column's pass³ split) — the view needed pinning for
      the same reason the conversation did (a growing column pushes the
      map off screen), so the stylesheet contract gains the source
      column's bounds and the workspace's second auto track; the new
      guards proven able to fail by a z-index and a literal width
- [x] Full dashboard suite and typecheck green — 301 tests across 20
      files, `tsc --noEmit` clean; the bundle-affected Rust suites
      (share, embedded, egress, serve) green too
