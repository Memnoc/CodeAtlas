# 12 — Claude API provider

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** The real provider behind the trait: direct HTTPS to the
Claude Messages API using structured outputs, so responses arrive
schema-valid and no repair machinery exists anywhere (ADR-0004). All of it
lives behind the Cargo network feature gate that ticket 15 will seal.

**Blocked by:** 11 — Enrichment core.

**Status:** done

- [x] The provider implements the trait via raw HTTPS to the Messages API —
      the binary's only possible egress destination
- [x] Requests use structured outputs so responses are guaranteed
      schema-valid; there is no parse-repair code path
- [x] Credentials resolve like the SDKs: `ANTHROPIC_API_KEY` first, then the
      `ant` OAuth profile (per ADR-0004)
- [x] Default model is `claude-opus-5`, overridable by flag or config
- [x] Missing credentials or API errors degrade cleanly: clear message,
      structural map intact, non-zero only for the enrichment step
- [x] All networking code is behind the network Cargo feature; the crate
      still compiles without it
