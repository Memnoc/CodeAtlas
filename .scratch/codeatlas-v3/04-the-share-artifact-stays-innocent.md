# 04 — The share artifact stays innocent

**Spec:** `docs/specs/2026-08-16-codeatlas-v3.md` — a fresh session reads
this first, then ADR-0013's share rejection.

**What to build:** A share artifact built from an enriched map carries no
source, no source-route reference, and no open-code affordance — a share
recipient is precisely someone who does not hold the code — and the
two-megabyte ceiling holds with the final V3 bundle embedded.

**Blocked by:** 02 — The dashboard opens code; 03 — Highlighted by the
vendored grammars.

**Status:** ready

- [ ] The artifact byte-contains no source-route reference, asserted
      beside the existing external-host scan
- [ ] The serverless dashboard never renders the open-code affordance —
      capabilities absence is the mechanism, asserted at the jsdom seam
      rather than inferred from it
- [ ] The ceiling test green on this repository's own enriched map, the
      measured size recorded here on completion (measured, never promised)
- [ ] Redaction behaviour unchanged: the existing share suite green
