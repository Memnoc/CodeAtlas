# 03 — Import and call edges

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** The map now shows relationships, not a tree: import edges
resolved across files (relative paths, index files, extension inference),
export edges, and call edges between functions, all typed and weighted per
the contract. Edges that cannot be resolved to a node in the map are dropped,
never emitted dangling.

**Blocked by:** 02 — Parser interface + TS/JS extraction.

**Status:** ready

- [ ] Import statements resolve to `imports` edges between file nodes for
      TS/JS, including relative paths and index-file conventions
- [ ] Exported symbols produce `exports` edges; function invocations produce
      `calls` edges where the callee is resolvable
- [ ] Edge types carry the fixed weights defined by the contract structs
- [ ] No edge in the emitted map references a missing node (referential
      integrity asserted in tests)
- [ ] Fixture-repo test asserts specific expected edges, including one
      cross-file import chain and one call edge
