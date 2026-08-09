# CodeAtlas

Map a codebase: its structure, and the relationships between its parts.

CodeAtlas scans a repository and emits a knowledge graph — files, functions,
classes, and the import/export/call edges between them — then renders it as an
interactive map you can search, walk, and share. It runs offline by default:
scanning, serving, diffing, and sharing never open a non-loopback socket, and
a sealed build exists in which egress is not a forbidden action but a
compile error.

Scanning CodeAtlas itself produces 598 nodes and 1096 edges in about 60 ms.

## Quick start

```sh
# one-time: the dashboard is compiled into the binary, so its deps must exist
cd dashboard && npm ci && cd ..

cargo build --release

./target/release/codeatlas scan .     # writes .codeatlas/knowledge-graph.json
./target/release/codeatlas serve .    # opens the map on http://127.0.0.1:4173/
```

Everything CodeAtlas writes lands in `.codeatlas/` under the scanned root — add
it to your `.gitignore` and scanning never dirties a worktree.

## Commands

| Command | What it does |
| --- | --- |
| `scan [PATH]` | Walk the repo, parse it, write the map to `.codeatlas/knowledge-graph.json`. `--enrich` additionally fills prose slots through an LLM (see below). |
| `serve [PATH]` | Serve the dashboard and the local map from memory on `127.0.0.1`. `--port` chooses the port; there is deliberately no `--host`. |
| `diff [PATH]` | Project a git diff onto the map: changed nodes plus their one-hop blast radius, written to `.codeatlas/diff-overlay.json`. Pure git and graph traversal — no LLM, no network. |
| `share [PATH]` | Export one self-contained, redacted HTML file that opens by double-click, with no server and no external requests. |
| `schema` | Print the JSON Schema of the map contract. |

The dashboard picks up the diff overlay automatically when one exists, offering
a toggle that distinguishes changed nodes from the ones they affect.

## Languages

| Language | Extensions |
| --- | --- |
| TypeScript | `.ts`, `.tsx` |
| JavaScript | `.js`, `.jsx`, `.mjs`, `.cjs` |
| Rust | `.rs` |
| Python | `.py` |
| Go | `.go` |
| C | `.c`, `.h` |
| C++ | `.cpp`, `.cc`, `.cxx`, `.hpp`, `.hh`, `.hxx` |
| Markdown | `.md`, `.markdown` — relative links become edges; no symbols |

Parsing uses tree-sitter grammars compiled into the binary; nothing is
downloaded at runtime. Files in unsupported languages still appear as nodes,
so the map stays complete. Every parser resolves imports and calls
conservatively: an edge that cannot be resolved to a node inside the map is
dropped rather than emitted dangling.

## The map contract

The emitted map conforms to a published, versioned contract
(`contract/map.schema.json`, currently **0.3.0**) generated from the Rust
types in `crates/codeatlas/src/map.rs` — the single source of truth. The
dashboard's TypeScript types are generated from the same schema, and CI fails
on any drift between the types and their generated artifacts. Consumers other
than the bundled dashboard can rely on the schema; `contract/README.md` states
the compatibility policy.

Node descriptions carry a `provenance` field of `structural` or `llm`, so a
reader can always tell a mechanically derived fact from a generated one.

## Enrichment (optional)

`scan --enrich` fills the map's prose slots — node summaries, layer names,
domain-flow names, tour narration — by calling the Claude API. It is entirely
opt-in, and the mechanical values are always present underneath: enrichment
relabels reality, it never creates it. If the provider fails, is unreachable,
or is never configured, you still get a complete, schema-valid structural map.

Annotations are cached in `.codeatlas/` keyed by node identity and a content
hash, so a later scan re-attaches unchanged answers for free and only
re-purchases the parts of the map that actually changed.

Credentials resolve like the official SDKs: `ANTHROPIC_API_KEY` first, then an
`ant auth login` profile. The default model is `claude-opus-5` (`--model`
overrides). Prompts are bounded — the model receives the slots being filled and
summarized topology, never the serialized graph.

## Security

`--enrich` is the only command capable of egress, and its only possible
destination is `api.anthropic.com`; redirects and environment proxies are
disabled at the transport level. Building with `--no-default-features`
produces a sealed binary containing no networking code at all, where every
command still works and `--enrich` refuses with a clear message.

These are tested claims, not documentation. **[docs/SECURITY.md](docs/SECURITY.md)**
is the audit entry point: it maps each guarantee to the code and the committed
test that enforces it, names the CI jobs that run them, and states the honest
limitations.

## Development

```sh
cargo test --workspace                        # default build
cargo test --workspace --no-default-features  # sealed build
cd dashboard && npm test -- --run             # dashboard
```

CI runs both Rust configurations, the dashboard suite, and a contract-drift
check that regenerates the schema and the TypeScript types and fails on any
diff. If you change a contract struct, regenerate both:

```sh
cargo run -p codeatlas -- schema > contract/map.schema.json
cd dashboard && npm run generate
```

Requires a Rust toolchain (edition 2024) and Node 24. Tests run offline; the
egress suite uses unprivileged Linux network namespaces and skips with an
explicit message where those are unavailable.

## Design record

The decisions behind this design, with their trade-offs, are recorded as ADRs
in [`docs/adr/`](docs/adr/) — CLI-first rather than prompt orchestration, a
Rust core with a TypeScript dashboard, Rust types generating the public
contract, enrichment behind a provider trait, content-hash carry-over, and
zero egress enforced by a compile-time feature gate. The V1 scope lives in
[`docs/specs/`](docs/specs/).

## Status

V1 is complete: all fifteen tickets shipped, both build configurations green.
The known next area is the dashboard at scale — layout currently places nodes
on a fixed grid within each layer, which reads well for small repositories and
becomes crowded for large ones.
