# 10 — Diff impact overlay

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** A reviewer runs the diff command and sees, on the map, the
changed nodes and their one-hop blast radius — which components a change
actually touches. Entirely deterministic: git diff in, overlay artifact out,
dashboard toggle renders it. Zero LLM involvement (spec story 7).

**Blocked by:** 03 — Import and call edges; 08 — Dashboard renders a map.

**Status:** done

- [x] The diff command derives changed nodes from git diff, including
      uncommitted working-tree changes
- [x] The one-hop blast radius over the graph's edges is computed and written
      as an overlay artifact in `.codeatlas/`
- [x] The dashboard offers an overlay toggle that highlights changed and
      affected nodes distinctly
- [x] Fixture test: modify one file, assert the overlay contains exactly the
      expected changed and affected node sets
- [x] Runs on the deterministic path — no LLM, no network
