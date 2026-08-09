# 11 — Enrichment core: provider trait, fake provider, carry-over

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** `codeatlas scan --enrich` fills node-summary slots through
the provider trait of ADR-0004, and the carry-over economics of ADR-0005
work: annotations are stored keyed by node identity plus content hash,
re-attach for free when code is unchanged, and expire when it changes. Built
and proven entirely against a fake provider — no network, no tokens; the real
API provider is ticket 12.

**Blocked by:** 02 — Parser interface + TS/JS extraction.

**Status:** done

- [x] A provider trait abstracts enrichment; a fake provider returns canned
      typed responses in tests
- [x] `--enrich` selects only `provenance: structural` nodes and fills their
      summary slots; enriched nodes flip to `provenance: llm`
- [x] Annotations persist in `.codeatlas/` keyed by node identity + content
      hash and re-attach on later runs without any provider call
- [x] Editing a fixture file expires its annotation: the node reverts to
      structural provenance and is re-selected on the next `--enrich`
- [x] A provider failure mid-run leaves a complete, schema-valid structural
      map (spec story 14)
- [x] No test in this ticket performs network I/O
