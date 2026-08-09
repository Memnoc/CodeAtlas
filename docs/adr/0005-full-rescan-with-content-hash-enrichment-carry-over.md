---
status: accepted
date: 2026-08-07
proposed-by: Claude Fable 5
approved-by: Memnoc
---

# ADR-0005: Full structural rescan every run; enrichment carried over by content hash

## Context

The baselines built incremental structural updates (fingerprints, git-diff
unions, graph splicing) because their pipeline was slow enough to make re-runs
painful. A native Rust tree-sitter parse makes a full structural rebuild of a
few-hundred-file repo effectively free, while the genuinely expensive layer —
paid LLM enrichment — must never be re-purchased for unchanged code.

## Decision

Every run recomputes the entire structural graph from scratch. LLM annotations
are stored in `.codeatlas/` keyed by node identity plus content hash; on each
run they re-attach to nodes whose hash is unchanged, and changed nodes revert
to `structural` provenance. `--enrich` processes only structural-provenance
nodes. In plain terms: the map is always rebuilt fresh and can never drift,
and money is only ever spent on code that actually changed.

## Considered options

- **Full rescan + hash-keyed carry-over** — chosen because it is the simplest
  model that keeps re-runs instant and enrichment spend proportional to the
  delta, with no cache-invalidation bugs possible for structure.
- **Fully incremental graph (baseline model)** — rejected because cross-file
  edge splicing is a correctness trap that only pays off on repos large enough
  (100k+ files) that even native parsing is slow; can be added later behind
  the same artifacts if profiling demands it.
- **SQLite store with JSON export** — rejected because a binary store diverges
  from the public JSON contract (ADR-0003) and graph sizes (~1 MB for a
  460-file repo) do not need a database.
