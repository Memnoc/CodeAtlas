# 04 — The share artifact stays innocent

**Spec:** `docs/specs/2026-08-16-codeatlas-v3.md` — a fresh session reads
this first, then ADR-0013's share rejection.

**What to build:** A share artifact built from an enriched map carries no
source, no source-route reference, and no open-code affordance — a share
recipient is precisely someone who does not hold the code — and the
two-megabyte ceiling holds with the final V3 bundle embedded.

**Blocked by:** 02 — The dashboard opens code; 03 — Highlighted by the
vendored grammars.

**Status:** done

- [x] The artifact byte-contains no source-route reference, asserted
      beside the existing external-host scan — made true structurally,
      not by scan-dodging: every route-speaking function moved to
      `wire.ts`, reached only by `App`'s served branch and only by
      dynamic import, so the wire chunk is a chunk `index.html` never
      references and the inliner never embeds; the scan walks the whole
      `serve::REGISTRY` (all five routes gone from the artifact, not
      just `/api/source`), and was proven able to fail twice — a route
      string planted in the payload tripped it, and a static import of
      `wire.ts` collapsed the chunk back into the bundle and tripped it
      from the other side; both reverted. The `routes.rs` drift gate
      followed the constants to `wire.ts` and now pins `SOURCE_ROUTE`
      too (tamper-tripped, reverted)
- [x] The serverless dashboard never renders the open-code affordance —
      ticket 02's absent-in-share jsdom test still stands, and a second
      test pins the stronger fact: with a live capabilities route
      answering `open_code: true` beside the payload, the affordance
      stays absent and share mode speaks zero requests. Both proven able
      to fail (affordance forced on in `App`'s share branch — both
      tripped; a fetch planted in the share branch — the zero-request
      guard tripped), tampers reverted
- [x] The ceiling test green on this repository's own enriched map, the
      measured size recorded here on completion (measured, never
      promised) — **1,665,798 bytes**, under the 2,097,152-byte ceiling;
      the same real artifact now also proves no registry route and no
      source line rides it (probe: the longest JSON-transparent line of
      serve.rs — the first tamper leaked a quoted line and sailed
      through as escaped bytes, so the probe was strengthened to a line
      the two leak paths cannot rewrite, then tripped honestly)
- [x] Redaction behaviour unchanged: the existing share suite green —
      14/14, every pre-existing redaction, exhaustiveness, determinism
      and fail-closed test untouched; full Rust suite green in all three
      feature configurations, dashboard 307/307 with `tsc --noEmit`,
      fmt and clippy clean
