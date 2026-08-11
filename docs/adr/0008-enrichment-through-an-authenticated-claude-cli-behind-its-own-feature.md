---
status: accepted
date: 2026-08-11
proposed-by: Claude Opus 5
approved-by: Memnoc
---

# ADR-0008: Enrichment can run through an already-authenticated Claude CLI, behind its own feature

## Context

ADR-0004 chose the direct Claude API and parked `claude -p` as "a candidate
provider implementation later", rejecting it partly because "output is free
text (the repair problem returns)". The Claude CLI now takes `--json-schema`
with `--output-format json`, so that objection no longer holds — and an API
key remains the only real way in for anyone outside Anthropic, which puts
enrichment out of reach in organisations where only administrators can obtain
one.

## Decision

Enrichment accepts `cli:claude`, which spawns the user's already-authenticated
Claude CLI as a schema-constrained completion, so CodeAtlas never handles a
credential; the provider compiles only under a new `agent-cli` Cargo feature,
separate from `network` and likewise in the default set. In plain terms:
someone who cannot get an API key but already has Claude Code can enrich a
map, and a security reviewer can still compile a build in which neither route
exists.

## Considered options

- **A separate `agent-cli` feature** — chosen because a subprocess contains no
  HTTP client, so filing it under `network` would make that feature's name and
  ADR-0006's "all network code is the Claude provider" false; and because two
  orthogonal features make a third configuration expressible — **HTTP client
  absent, approved CLI permitted** — which is exactly the posture of an
  organisation that forbids outbound HTTP from tooling but has already
  approved Claude Code.
- **Fold `cli:` into the existing `network` feature** — rejected: fewer moving
  parts, but it collapses two genuinely different egress mechanisms into one
  switch and leaves the useful third configuration unreachable.
- **A generic `cli:<program>` provider** — rejected for V1. "CodeAtlas will
  execute whatever program you name" is a far worse sentence in a security
  document than "CodeAtlas can invoke `claude`", and the provider trait keeps
  the door open.
- **A configurable API endpoint** (Bedrock, Vertex, a corporate gateway) —
  rejected because it collides with a tested invariant, that the transport
  can be steered nowhere (`the_agent_can_neither_follow_redirects_nor_use_an_env_proxy`).

## Consequences

The spawned process is a pure completion, not an agent: no tools, no MCP
servers, and a working directory outside the repository. Without that, the CLI
could read files through its own tooling and quietly void `docs/SECURITY.md`'s
standing promise that the model receives "never file contents".

Its environment is an explicit allowlist (`PATH`, `HOME`, `XDG_*`) rather than
an inherited one, and deliberately excludes `ANTHROPIC_API_KEY` so that `cli:`
unambiguously means the CLI's own credential rather than silently billing the
API through a subprocess. A security document's worth is bounded answers, and
"the whole environment" is not one.

Provider selection gains a `--provider` flag beside the existing
`CODEATLAS_ENRICH_PROVIDER` variable: an environment-variable-only switch is
the same discoverability failure this decision exists to fix.

**The sealed build needs a new kind of proof.** `crates/codeatlas/tests/sealed.rs`
works by asserting no networking crates are linked, and a subprocess adds no
dependency — so that test would pass whether or not this provider were
compiled in. Two probes replace it: the sealed binary must reject `cli:claude`
with the "not available in this build" message, and `scripts/sealed-probe.sh`
must find no `claude` program string in its bytes. Both need the default build
as a live control, or they assert nothing.

CI grows a third configuration, `--no-default-features --features agent-cli`.
The configuration this ADR was chosen to make expressible is a claim rather
than a guarantee until something runs it.
