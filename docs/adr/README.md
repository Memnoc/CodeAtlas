# Architecture Decision Records

| ADR | Decision | Status | Date |
|-----|----------|--------|------|
| [0001](./0001-cli-first-program-not-prompt-orchestration.md) | CodeAtlas is a CLI-first program; skills are thin wrappers | accepted | 2026-08-07 |
| [0002](./0002-rust-core-typescript-dashboard.md) | Rust core with a TypeScript dashboard | accepted | 2026-08-07 |
| [0003](./0003-rust-types-generate-the-public-map-contract.md) | Rust types generate the JSON Schema that is the public map contract | accepted | 2026-08-07 |
| [0004](./0004-enrichment-via-direct-claude-api-behind-a-provider-trait.md) | Enrichment calls the Claude API directly, behind a provider trait | accepted | 2026-08-07 |
| [0005](./0005-full-rescan-with-content-hash-enrichment-carry-over.md) | Full structural rescan every run; enrichment carried over by content hash | accepted | 2026-08-07 |
| [0006](./0006-zero-egress-enforced-by-compile-time-feature-gate.md) | Zero egress is enforced by a compile-time feature gate | accepted (surface extended by [0008](./0008-enrichment-through-an-authenticated-claude-cli-behind-its-own-feature.md), [0009](./0009-codebase-questions-are-answered-by-the-serving-binary.md)) | 2026-08-07 |
| [0007](./0007-the-annotation-store-is-a-committed-repository-artifact.md) | The annotation store is a committed repository artifact | accepted | 2026-08-11 |
| [0008](./0008-enrichment-through-an-authenticated-claude-cli-behind-its-own-feature.md) | Enrichment can run through an already-authenticated Claude CLI, behind its own feature | accepted | 2026-08-11 |
| [0009](./0009-codebase-questions-are-answered-by-the-serving-binary.md) | Codebase questions are answered by the serving binary, not the dashboard | accepted | 2026-08-11 |
