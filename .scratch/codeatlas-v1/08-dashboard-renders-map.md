# 08 — Dashboard renders a map

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** Feed the dashboard a map file and explore it: a React Flow
graph canvas with layer grouping, fuzzy search, and a node detail panel
showing summaries, edges, and the provenance badge. Demoable via the dev
server against fixture maps and CodeAtlas's own map; serving from the binary
is ticket 09.

**Blocked by:** 06 — Mechanical semantics; 07 — Published contract.

**Status:** ready

- [ ] The dashboard loads a graph file conforming to the contract and renders
      nodes and edges on a React Flow canvas grouped by layer
- [ ] Search finds nodes by name/path; selecting a node shows its detail
      (summary, edges, line range, provenance badge)
- [ ] Rendering makes zero external requests — all assets and fonts are local
      (asserted, not assumed)
- [ ] The dashboard compiles against the generated TS types only
- [ ] Renders both a small fixture map and CodeAtlas's own self-scan map
      without errors
