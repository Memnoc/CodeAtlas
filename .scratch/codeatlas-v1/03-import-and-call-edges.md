# 03 — Import and call edges

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** The map now shows relationships, not a tree: import edges
resolved across files (relative paths, index files, extension inference),
export edges, and call edges between functions, all typed and weighted per
the contract. Edges that cannot be resolved to a node in the map are dropped,
never emitted dangling.

**Blocked by:** 02 — Parser interface + TS/JS extraction.

**Status:** ready

**Carry-over from ticket 02 crosscheck:** introduce a typed node-ID type
before more ID-minting sites appear (three inline `format!` sites exist);
consolidate the symbol-kind → node-kind/id-prefix mapping into one place;
decide whether arrow functions assigned to consts count as functions
(currently they don't, which undercounts idiomatic TS).

- [ ] Import statements resolve to `imports` edges between file nodes for
      TS/JS, including relative paths and index-file conventions
- [ ] Exported symbols produce `exports` edges; function invocations produce
      `calls` edges where the callee is resolvable
- [ ] Edge types carry the fixed weights defined by the contract structs
- [ ] No edge in the emitted map references a missing node (referential
      integrity asserted in tests)
- [ ] Fixture-repo test asserts specific expected edges, including one
      cross-file import chain and one call edge
