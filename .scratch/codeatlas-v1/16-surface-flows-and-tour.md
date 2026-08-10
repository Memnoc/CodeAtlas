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

**Status:** done

- [x] The dashboard surfaces the guided tour: an ordered, navigable walk that
      moves the canvas selection step by step, showing each step's label
- [x] The dashboard surfaces domain flows, grouped by domain, with each
      flow's name and its ordered steps selectable on the canvas
- [x] Both degrade honestly: a map with no `tour`/`domain_flows` (both are
      optional in the contract) renders without the affordances and without
      errors
- [x] Mechanical and enriched labels both display, with the provenance badge
      already used for node detail
- [x] The tour is bounded and curated rather than one step per file — a
      newcomer-sized walk over architecturally significant nodes, with the
      selection rule documented and deterministic
- [x] Tour ordering no longer ranks isolated files first: on CodeAtlas's own
      map the current step 1 is `tests/scan.rs` with fan-in 0 and fan-out 0,
      ahead of `lib.rs` at step 9
- [x] The share artifact renders the same affordances (same renderer, per
      ticket 14) with redacted labels intact
- [x] Dashboard tests drive the tour and flow affordances from the map-contract
      seam, per the spec's Testing Decisions

**How it landed.** The tour's selection rule is two deterministic passes in
`semantics::build_tour`: files score `import fan-in + fan-out + 1 for hosting
an entry point` (one point for the file, not one per function, so a test
module full of test functions cannot out-rank a hub), zero-scoring files are
off the walk, the top `TOUR_MAX_STEPS` (12) survive, and the survivors are
then ordered `fan-out − fan-in + the entry-point bonus` so composition roots
open and foundation modules close. On CodeAtlas's own map that is 12 steps
of 154 files, led by the spec doc and `lib.rs`, with `map.rs` last and
`tests/scan.rs` gone. The contract's `tour` description was corrected to say
the walk is bounded (patch bump 0.3.0 → 0.3.1; schema and TS types
regenerated). The dashboard gained `TourPanel`, `FlowsPanel`, and a shared
`ProvenanceBadge`, and the canvas now marks whatever the sidebar selected.

`/crosscheck` findings folded back in: the flows panel opens as a collapsed
index of domains (CodeAtlas's own map has 141 flows, 137 of them under
`crates`, which as a flat list would reproduce the very defect this ticket
fixes on the tour side); sidebar selections now bring the node into view, so
a highlight on a 600-node canvas is not off-screen; `README.md`'s contract
version, a stale CSS comment, and a vestigial entry-point counter were
corrected.

Not covered: no browser could be driven here (Firefox headless fails to map
a framebuffer), so layout and paint remain unwatched — the same gap harden
recorded for stories 3 and 8. Two things this ticket deliberately did not
change: test-fixture files can still earn a tour slot (CodeAtlas's own repo
contains eight miniature repos, and their files genuinely participate in an
import graph — excluding them would need a path heuristic the contract
cannot justify), and the flow list itself is still unbounded, which is a V2
question rather than a criterion here.
