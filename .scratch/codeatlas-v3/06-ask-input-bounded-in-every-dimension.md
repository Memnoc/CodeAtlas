# 06 — Ask input bounded in every dimension

**Spec:** `docs/specs/2026-08-16-codeatlas-v3.md` — a fresh session reads
this first, then ADR-0012. This ticket is V2 ticket 08's residual list,
promoted.

**What to build:** The last two unbounded-or-untested corners of the ask
route close: carried citations get a per-field bound clamped like every
other carried field, and the 400 for a structurally-wrong turn — which the
route already draws — gets the test it never had.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] Carried citations are clamped per field — count and length — the way
      every carried field already is: mechanically, excess dropped, never
      a refusal, because the history is the dashboard's bookkeeping
      (ADR-0012's reasoning); observable on the wire
- [x] A structurally-wrong turn (missing field, non-array citations) draws
      the 400 it always drew, now pinned by a test proven able to fail
- [x] The new bound lives beside the existing five in the ask module, and
      `docs/SECURITY.md`'s bounds list names it
- [x] Full suite green in all three feature configurations
