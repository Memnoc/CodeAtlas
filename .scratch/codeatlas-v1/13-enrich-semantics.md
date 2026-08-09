# 13 — Enrich layers, flows, and tour narration

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** Enrichment extends beyond node summaries to the teaching
layer: meaningful layer names, business-domain flow names, and narrated tour
steps. Prompts are bounded — the model receives mechanically summarized
topology (fan-in/out, entry-point scores) and slots to fill, never the whole
graph — and incremental runs re-purchase nothing that is unchanged. This
kills the baselines' second-worst cost (whole-graph prompts re-run on every
update).

**Blocked by:** 06 — Mechanical semantics; 11 — Enrichment core.

**Status:** ready

- [ ] Layer names, domain-flow names, and tour narration are typed slots
      filled through the provider trait
- [ ] No prompt contains the full serialized graph — only summarized topology
      and the slots in question (bounded-prompt property asserted at the
      provider seam)
- [ ] Carry-over applies: unchanged layers/flows/tour steps are not re-sent
      on incremental runs
- [ ] With enrichment absent or failed, the mechanical labels from ticket 06
      remain — degrade, never break
- [ ] Fake-provider tests assert answers land in the right slots and the map
      stays schema-valid
