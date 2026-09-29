# Development, design record, status

Back to the [README](../README.md) · [Tour](tour.md) ·
[Commands](commands.md) · [Enrichment](enrichment.md) ·
[Security](SECURITY.md)

## Development

```sh
cargo test --workspace                        # default build
cargo test --workspace --no-default-features  # sealed build
cargo test --workspace --no-default-features --features agent-cli  # CLI, no HTTP client
cd dashboard && npm test -- --run             # dashboard
```

CI runs all three Rust configurations, the dashboard suite, and a
contract-drift check that regenerates the schema and the TypeScript types
and fails on any diff. Requires a Rust toolchain (edition 2024) and Node 24.
Tests run offline; the egress suite uses unprivileged Linux network
namespaces and skips with an explicit message where those are unavailable.

## Design record

The decisions behind this design, with their trade-offs, are recorded as
ADRs in [`docs/adr/`](adr/); each version's scope lives in
[`docs/specs/`](specs/).
CodeAtlas is built AI-assisted, guided by the Northstar
engineering pipeline: specs, tickets, test-first slices, cross-checked
reviews, with every decision recorded in those ADRs.

Stated once, as the **standing transparency position: CodeAtlas's AI is
strictly bring-your-own: `--ask` and enrichment call Anthropic's Claude
with credentials you supply, and nothing else in the tool talks to a
model**; the sealed build cannot even be compiled to. Wherever AI-written
prose appears, I did my best to expose it: the dashboard badges enriched text where it
renders it, the annotation store carries a machine-readable record naming
the provider, the model and the UTC date of the last run that wrote it,
and `share` removes AI prose from the exported file entirely. **Interaction
with the model is always labelled as interaction with the model**. This is
stated as practice, verified by the tests [`docs/SECURITY.md`](SECURITY.md) so a reader
never has to guess which words a model wrote.

## Status

V3 shipped on 2026-08-18: distribution: the prebuilt, checksummed,
provenance-attested binaries above, with a sealed variant beside each and
open code, a mapped file's source shown in the dashboard lit at its own
lines, opt-in via `serve --open-code`. The point releases since are
field-feedback laps: the scan progress line, the terminal menu, the
full-screen source view, the third Rosé Pine variant. Each version's spec
carries its own story-by-story Verification section
([V1](specs/2026-08-09-codeatlas-v1.md),
[V2](specs/2026-08-13-codeatlas-v2.md),
[V3](specs/2026-08-16-codeatlas-v3.md)).
