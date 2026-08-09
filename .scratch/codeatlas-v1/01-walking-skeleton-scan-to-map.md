# 01 — Walking skeleton: scan → schema-valid map

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** Run `codeatlas scan` in any repository and get
`.codeatlas/knowledge-graph.json` containing project metadata and a file node
for every source file, validating against a JSON Schema generated from the
Rust structs. The whole pipeline exists end to end — scan, build, save,
validate — one inch wide: no parsing yet, just the skeleton every later
ticket thickens.

**Blocked by:** None — can start immediately.

**Status:** ready

- [ ] A Cargo workspace builds a single `codeatlas` binary
- [ ] `codeatlas scan` walks the repo honoring ignore rules (gitignore plus
      sensible defaults like `node_modules`, `target`, `.git`)
- [ ] The emitted map has a semver `version` field, project metadata, and file
      nodes with typed IDs (`file:<relative-path>`)
- [ ] The JSON Schema is generated from the Rust structs (schemars), not
      written by hand
- [ ] A fixture repo committed in-tree has a test asserting the emitted map
      contains the expected file nodes and validates against the generated
      schema
- [ ] Two runs on the same input produce identical output
