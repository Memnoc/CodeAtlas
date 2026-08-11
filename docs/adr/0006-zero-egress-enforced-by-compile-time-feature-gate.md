---
status: accepted
date: 2026-08-07
proposed-by: Claude Fable 5
approved-by: Memnoc
---

# ADR-0006: Zero egress is enforced by a compile-time feature gate

> Egress surface extended by [ADR-0008](./0008-enrichment-through-an-authenticated-claude-cli-behind-its-own-feature.md)
> and [ADR-0009](./0009-codebase-questions-are-answered-by-the-serving-binary.md) —
> the feature-gate rule below is unchanged; the list of what sits behind it is
> not. The Decision's clause naming the enrichment provider as the only
> network code was corrected on 2026-08-11 for that reason.

## Context

CodeAtlas must be runnable on proprietary code and survive a workplace
security audit. The working assumption is that passing code to one approved
LLM (Claude) is sanctioned; everything else must be offline — no third-party
services, no telemetry, no external calls from the dashboard. A documented
policy is not audit-grade; the guarantee has to be verifiable.

## Decision

All egress-capable code lives behind a Cargo feature. The standard binary
includes those features but the default (deterministic) path never touches the
network; a sealed
`--no-default-features` build produces a binary that contains no networking
code at all. An egress test suite asserts the default path opens no sockets
and the dashboard serves only local, vendored assets; a redaction
exhaustiveness test derived from the map schema (ADR-0003) ensures no schema
field can reach a share artifact without being classified share-safe or
redacted, and the artifact discloses its own redaction. In plain terms: in the
sealed build, sending data anywhere is not a forbidden action but an
impossible one.

## Considered options

- **Compile-time feature gate + egress tests** — chosen because "here is the
  build where exfiltration is a compile error" is the strongest claim
  available to a security review, and Rust (ADR-0002) makes it cheap.
- **Single binary + egress test suite only** (RepoAtlas ADR-0002 model) —
  rejected because the audit artifact is weaker for one less release target.
- **Runtime policy only** — rejected; documentation without enforcement is
  the pet-project posture this project exists to avoid.

## Consequences

Tree-sitter grammars and all dashboard assets must be compiled in or vendored
— no runtime downloads on any path. CI builds and tests both feature
configurations.
