# The CodeAtlas map contract

`map.schema.json` in this directory is the official, public contract for
CodeAtlas map files (ADR-0003). Any producer — the `codeatlas` CLI, a
Northstar skill, or any third-party tool — that emits a file validating
against this schema produces a map the CodeAtlas dashboard renders.

## Source of truth

The schema is **generated, never edited by hand**. The Rust structs in
`crates/codeatlas/src/map.rs` (serde + schemars) are the single source of
truth; this file is regenerated from them with:

```sh
cargo run -p codeatlas -- schema > contract/map.schema.json
```

The dashboard's TypeScript types are generated from this schema in turn
(`npm run generate` in `dashboard/`). CI regenerates both artifacts and fails
on any diff against the committed versions, so the contract cannot drift from
the code.

## Versioning

The contract is versioned with **semver**, currently **0.2.0**
(`MAP_CONTRACT_VERSION` in `crates/codeatlas/src/map.rs`). Every map file
carries the contract version it conforms to in its top-level `version` field.

- **Major** — breaking change: removing or renaming a field, narrowing a
  type, adding a *required* field, removing an enum variant. Consumers must
  opt in.
- **Minor** — backward-compatible extension: a new *optional* field, a new
  enum variant, a loosened constraint. Existing maps stay valid.
- **Patch** — no shape change: descriptions, docs, generation details.

While the major version is 0, minor bumps may still break (standard semver
0.x semantics); the contract stabilizes at 1.0.0.

Any change to the schema **must** bump `MAP_CONTRACT_VERSION` accordingly and
regenerate the committed artifacts in the same commit.
