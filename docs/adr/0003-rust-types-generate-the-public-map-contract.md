---
status: accepted
date: 2026-08-07
proposed-by: Claude Fable 5
approved-by: Memnoc
---

# ADR-0003: Rust types generate the JSON Schema that is the public map contract

## Context

With a Rust core and a TypeScript dashboard (ADR-0002), the graph schema —
node types, edge types, weights, layers, tour, provenance — exists on both
sides of a language border and will drift unless one side is generated.
Separately, CodeAtlas wants its map format to be a documented public contract
("produce this file and I'll render it") so that other producers — such as a
future Northstar skill — can emit maps the dashboard renders.

## Decision

The Rust structs (serde + schemars) are the single source of truth. The build
emits a JSON Schema file, and the dashboard's TypeScript types are generated
from that schema. The generated JSON Schema is published, versioned with
semver, and is the official CodeAtlas map contract. In plain terms: there is
exactly one definition of what a map file looks like, the machine derives
everything else from it, and anyone can build a tool that produces or consumes
CodeAtlas maps.

## Considered options

- **Rust → JSON Schema → TS** — chosen because it is the strongest toolchain
  direction (schemars → JSON Schema → TS generation is mature) and the public
  contract artifact falls out of the build instead of being hand-maintained.
- **Handwritten JSON Schema as source** — rejected because Rust codegen from
  JSON Schema is the weakest link in the chain and would mean hand-tuning
  generated Rust.
- **Both defined by hand with contract tests** — rejected as a permanent
  synchronization tax.

## Consequences

Schema changes start in Rust and are breaking-change-managed through the
published contract's semver. The share-artifact redaction exhaustiveness test
(ADR-0006) is derived from this same schema, so a new field cannot ship until
it is classified as share-safe or redacted.
