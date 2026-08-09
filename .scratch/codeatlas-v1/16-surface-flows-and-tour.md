# 16 — Surface domain flows and the guided tour in the dashboard

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** The newcomer's half of spec story 6. Ticket 06 projects
domain flows and orders the tour; ticket 13 enriches their names and
narration — but no consumer ever renders them. `grep -rn "tour\|domain_flows"
dashboard/src/` matches only `map.generated.ts` (the generated types) and the
`index.ts` re-export: `MapExplorer`, `App`, `nodes`, `graph`, and `overlay`
never read either field. A newcomer can only reach the tour by reading the
raw JSON, which is exactly what story 3 says the dashboard exists to prevent.

Found by `/harden` on 2026-08-09: each of tickets 06, 08, and 13 passed its
own acceptance criteria; the gap is between them.

The same walk found the tour unbounded — it emits exactly one step per file
node (148 steps on CodeAtlas itself, 3000 on a 3000-file repo, including
`Cargo.lock`, `package-lock.json`, and every test fixture). An enumeration of
every file is not a guided tour, so surfacing it as-is would not satisfy the
story either. Both halves belong to this ticket.

**Blocked by:** none — 06, 08, and 13 are done.

**Status:** ready

- [ ] The dashboard surfaces the guided tour: an ordered, navigable walk that
      moves the canvas selection step by step, showing each step's label
- [ ] The dashboard surfaces domain flows, grouped by domain, with each
      flow's name and its ordered steps selectable on the canvas
- [ ] Both degrade honestly: a map with no `tour`/`domain_flows` (both are
      optional in the contract) renders without the affordances and without
      errors
- [ ] Mechanical and enriched labels both display, with the provenance badge
      already used for node detail
- [ ] The tour is bounded and curated rather than one step per file — a
      newcomer-sized walk over architecturally significant nodes, with the
      selection rule documented and deterministic
- [ ] Tour ordering no longer ranks isolated files first: on CodeAtlas's own
      map the current step 1 is `tests/scan.rs` with fan-in 0 and fan-out 0,
      ahead of `lib.rs` at step 9
- [ ] The share artifact renders the same affordances (same renderer, per
      ticket 14) with redacted labels intact
- [ ] Dashboard tests drive the tour and flow affordances from the map-contract
      seam, per the spec's Testing Decisions
