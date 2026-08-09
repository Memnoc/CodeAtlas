---
status: accepted
date: 2026-08-07
proposed-by: Memnoc
approved-by: Memnoc
---

# ADR-0002: Rust core with a TypeScript dashboard

## Context

The dashboard must be web technology (React Flow has no serious rival for the
graph UI), so TypeScript exists in the project regardless; the open choice was
the language of the CLI/analysis core. The baselines prove TypeScript with
web-tree-sitter WASM already reaches ~2.3-second structural runs, so raw
parsing speed alone did not force the choice.

## Decision

The CodeAtlas core (CLI and analysis engine) is written in Rust with native
tree-sitter bindings; the dashboard remains TypeScript/React. In plain terms:
the analyzer ships as a single fast binary with no runtime to install, at the
cost of maintaining two languages.

## Considered options

- **Rust core + TS dashboard** — chosen for the single static binary (no Node
  dependency on the host), native tree-sitter performance, and the strongest
  possible security/audit artifact — including compile-time egress guarantees
  (see ADR-0006) that a Node binary cannot offer.
- **TypeScript everywhere** — rejected despite being faster to build and
  giving one schema definition for free; performance was already adequate, but
  the audit story and distribution story are weaker. The schema-drift cost of
  two languages is neutralized by ADR-0003.
- **Go core + TS dashboard** — rejected because its tree-sitter bindings are
  community-maintained and weaker than Rust's or Node's.

## Consequences

The graph schema exists on both sides of the Rust/TS border and would drift
without a single source of truth — resolved by ADR-0003. Build times and
development speed are slower than a TS monorepo; this is accepted.
