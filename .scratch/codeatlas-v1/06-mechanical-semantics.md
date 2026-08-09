# 06 — Mechanical semantics: layers, domain flows, tour order

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** The zero-LLM map becomes complete: every file belongs to a
directory-derived layer, domain flows are projected from call chains rooted
at entry points, and the guided tour exists as a topologically ordered walk
with mechanical labels. Enrichment (ticket 13) will only relabel what this
ticket creates — "enrichment relabels reality; it never creates it."

**Blocked by:** 03 — Import and call edges.

**Status:** ready

- [ ] Every file node is assigned to exactly one layer derived from directory
      structure
- [ ] Domain flows are projected mechanically: domains from top-level
      directories, flows as call chains starting at functions nothing else
      calls
- [ ] Tour steps are ordered by topology scoring (fan-in/out, entry-point
      score) and carry mechanical labels
- [ ] All of this runs in the default deterministic path — zero LLM, zero
      network — with `provenance: structural` throughout
- [ ] Same input produces the same layers, flows, and tour order every run
- [ ] Fixture test asserts layer coverage (no orphan files) and at least one
      expected flow
