---
status: accepted
date: 2026-08-07
proposed-by: Claude Fable 5
approved-by: Memnoc
---

# ADR-0004: Enrichment calls the Claude API directly, behind a provider trait

## Context

Optional enrichment (summaries, layer names, tour narration, domain-flow
naming) needs an LLM, and Rust has no official Anthropic SDK — the sanctioned
path for unsupported languages is raw HTTPS against the Messages API. The
baselines received freeform agent output and spent a 54 KB merge script plus
hundreds of prompt lines repairing it; the Messages API's structured outputs
(`output_config.format`) instead guarantee schema-valid JSON responses.

## Decision

The Rust CLI calls the Claude API directly over HTTPS, using structured
outputs so the model fills typed prose slots in an already-built graph. The
integration sits behind a small provider trait so alternative backends (a
`claude -p` subscription-billed provider, a local model) can be added without
touching the pipeline. In plain terms: when enrichment is requested, the tool
talks to exactly one external service, and the answers come back in a shape
the tool has already validated.

## Considered options

- **Direct API behind a provider trait** — chosen because schema-guaranteed
  JSON eliminates all output-repair machinery, and a single endpoint
  (`api.anthropic.com`) is the auditable egress surface the security posture
  (ADR-0006) requires.
- **Shell out to `claude -p`** — rejected as the default because output is
  free text (the repair problem returns), it requires Claude Code installed,
  and egress through another program weakens the audit claim. Remains a
  candidate provider implementation later, reusing the user's subscription.
- **Build both transports in V1** — rejected as scope; the trait keeps the
  door open.

## Consequences

Enrichment bills as Anthropic API usage (per-token) rather than a Claude
subscription. Credential resolution should mirror the SDKs: `ANTHROPIC_API_KEY`
first, then an `ant auth login` OAuth profile (bearer token via
`ant auth print-credentials --access-token` with the `oauth-2025-04-20` beta
header).
