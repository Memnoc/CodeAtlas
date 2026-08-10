# CodeAtlas V1

> Crystallised from the 2026-08-07 `/adr` interview. Decisions live in
> [docs/adr/](../adr/README.md); evidence in
> [the baseline study](../research/2026-08-07-baseline-repoatlas-understand-anything.md);
> the original pitch in
> [the intake digest](../intake/2026-08-07-codeatlas-pitch-and-adr-agenda.md).

## Problem Statement

A developer dropped into an unfamiliar codebase — a new job, an interview, a
repo they inherited — needs a map: what the pieces are and how they connect.
The tools that proved this map is genuinely useful (Understand Anything and its
fork RepoAtlas) take ~25 minutes per run and carry features their own users
neither need nor understand, so mapping a codebase is a scheduled event, not a
reflex.

Worse, those tools can't leave the hobby drawer. An engineer who wants to run
one on proprietary code at work faces a security review it cannot pass: the
pipeline's egress is a property of prompts, not of code, so nobody can prove
where the source goes. The people who would benefit most — teams with large
private codebases — are exactly the people who can't use it.

## Solution

A developer runs one command in any repo and gets a precise structural map —
files, functions, classes, and the import/call/containment relationships
between them — in seconds, offline, for free. They explore it in a local
dashboard, hand a reviewer the blast radius of a diff, and walk a newcomer
through the architecture with domain flows and a guided tour.

When they want the map to *teach* — prose summaries, meaningful layer names,
narrated tours — they opt into enrichment, and pay only for code that changed
since the last run. A colleague with nothing installed receives the map as a
single HTML file that opens on double-click and states exactly what was
redacted from it. A security auditor approves the tool by reading code, not
promises: one build variant contains no networking code at all.

## User Stories

1. As a developer, I want a complete structural map of a codebase in seconds
   with no LLM involved, so that mapping a repo is something I do casually
2. As a developer, I want the map to capture relationships — imports, calls,
   containment, exports — not just a file tree, so that I can trace how
   components actually connect
3. As a developer, I want an interactive local dashboard (graph canvas,
   search, layer grouping, node detail), so that I navigate the map instead of
   reading JSON
4. As a developer, I want to opt into LLM enrichment that fills prose slots —
   node summaries, layer names, domain-flow names, tour narration — so that
   the map explains itself to someone who doesn't know the code
5. As a developer, I want re-runs to re-purchase enrichment only for code
   whose content changed, so that cost is proportional to my delta, not my
   repo
6. As a newcomer to a codebase, I want domain flows and an ordered guided
   tour, so that I learn the architecture in the order it makes sense
7. As a code reviewer, I want a diff's changed nodes and their one-hop blast
   radius overlaid on the map, so that I can judge the risk of a change
8. As a developer, I want to export a single self-contained redacted HTML
   file, so that I can share the map with someone who has nothing installed
9. As a security auditor, I want a sealed build that contains no networking
   code, plus an egress test suite over the default build, so that approving
   CodeAtlas is a code review rather than a trust exercise
10. As a recipient of a shared map, I want the artifact to disclose what was
    redacted from it, so that I know what I am and am not seeing
11. As the CLI, I want to rebuild the structural graph from scratch on every
    run, so that the map can never drift from the code
12. As the CLI, I want to re-attach stored annotations only to nodes whose
    content hash is unchanged and revert the rest to structural provenance,
    so that stale prose never describes new code
13. As the CLI, I want enrichment responses as schema-guaranteed structured
    output filling typed slots in an already-built graph, so that no
    output-repair machinery exists
14. As the CLI, I want an enrichment failure (network down, no credentials,
    API error) to leave a valid structural map behind, so that the LLM layer
    degrades and never breaks the product
15. As the CLI, I want files I cannot parse (unsupported language, syntax
    errors) to still appear as file nodes with whatever edges are resolvable,
    so that the map is complete even where analysis is shallow
16. As an external producer (for example a future Northstar skill), I want a
    published, semver-versioned JSON Schema for the map format, so that I can
    emit files the CodeAtlas dashboard renders
17. As the dashboard, I want to consume only local files and make zero
    external requests, so that viewing a map is fully offline

## Implementation Decisions

The load-bearing decisions are recorded as ADRs; this spec must not contradict
them:

- [ADR-0001](../adr/0001-cli-first-program-not-prompt-orchestration.md) — the
  CLI owns the whole pipeline; skills are ~20-line wrappers
- [ADR-0002](../adr/0002-rust-core-typescript-dashboard.md) — Rust core,
  TypeScript/React dashboard
- [ADR-0003](../adr/0003-rust-types-generate-the-public-map-contract.md) —
  Rust structs generate the JSON Schema that is the public map contract; the
  dashboard's TS types are generated from it
- [ADR-0004](../adr/0004-enrichment-via-direct-claude-api-behind-a-provider-trait.md)
  — enrichment calls the Claude API directly with structured outputs, behind a
  provider trait
- [ADR-0005](../adr/0005-full-rescan-with-content-hash-enrichment-carry-over.md)
  — full structural rescan every run; enrichment carried over by content hash
- [ADR-0006](../adr/0006-zero-egress-enforced-by-compile-time-feature-gate.md)
  — zero egress enforced by a compile-time feature gate; sealed build; egress
  tests; schema-derived redaction exhaustiveness

Spec-level decisions on top of those:

- **V1 capability cut**: structural knowledge graph, dashboard, opt-in
  enrichment, share artifact, diff impact, domain flows + guided tour.
  Everything else waits (see Out of Scope).
- **Artifacts** live in `.codeatlas/` at the repo root: the map file, the
  enrichment annotation store, and any overlays. The map file validates
  against the published schema; everything the dashboard renders comes from
  this directory.
- **Graph shape** follows the baselines' converged design: typed node IDs, a
  small closed set of node types and weighted typed edge types, layers, an
  ordered tour, and per-node `provenance: structural | llm`. Exact type sets
  are defined by the Rust structs and published via the contract, not
  hand-listed here.
- **Every enriched field has a mechanical fallback** computed deterministically
  in the same run: directory-derived layers, mechanical summaries ("Rust
  file, 214 lines: 3 functions"), domain flows projected from directories and
  call chains. Enrichment relabels reality; it never creates it. A map with
  zero LLM involvement is complete, just tersely labeled.
- **Enrichment prompts are bounded.** The model receives per-node/per-module
  slots to fill and mechanically summarized topology (fan-in/out, entry-point
  scores) for layer naming and tour narration — never the whole graph in one
  prompt. Incremental runs re-enrich only structural-provenance nodes; they do
  not re-run whole-graph passes.
- **Diff impact is deterministic**: git diff → changed nodes → one-hop blast
  radius → an overlay file the dashboard renders. Zero LLM involvement.
- **The tour's ordering is mechanical, its narration is enrichment.** Entry
  points and step order come from topology scoring; without enrichment the
  tour exists as an ordered walk with mechanical labels.
- **The dashboard is served by the CLI** on a loopback address, from assets
  embedded in the binary — no Node runtime, no dev server, no downloads. The
  share artifact is the same renderer inlined into one HTML file.
- **V1 languages**: TypeScript/JavaScript, Rust, Python, Go, C, and C++
  grammars with import resolution (`#include` resolution and header/source
  pairing for C/C++), plus Markdown link edges. This covers CodeAtlas itself
  (dogfooding both halves) and the mainstream cases; each further language is
  one grammar plus one import resolver behind the same parser interface.
  Settled 2026-08-09 during ticketing.
- **Language coverage is honest**: unsupported or unparseable files degrade
  per story 15 rather than disappearing.

## Testing Decisions

Two seams, confirmed 2026-08-09. All tests assert external behaviour at these
boundaries; no test reaches into pipeline internals.

- **Seam 1 — the map contract.** The CLI is tested by running it against small
  fixture repos committed in-tree and asserting properties of the emitted map
  file: expected nodes and edges exist, IDs are well-formed, the file
  validates against the generated schema, provenance and carry-over behave per
  ADR-0005 (edit a fixture file, re-run, assert reversion). The dashboard,
  share artifact, and diff overlay are tested from the other side of the same
  seam: feed a graph file in, assert what is rendered or emitted. Assertions
  are structural properties, not byte-golden files — layout and ordering are
  free to change.
- **Seam 2 — the enrichment provider trait.** A fake provider returns canned
  typed responses; tests assert annotations land in the right slots, are
  stored keyed by content hash, and re-attach or expire correctly. Failure
  injection at the same seam covers story 14: the provider errors, the
  structural map survives. No test ever performs real network I/O.
- **The security posture is tested, not documented** (ADR-0006): the egress
  suite asserts the default path opens no sockets; CI builds and tests both
  feature configurations; the sealed build's compilation is itself the proof
  it contains no networking code. The redaction exhaustiveness test walks the
  generated schema and fails if any field is neither share-safe nor redacted —
  a new field cannot ship unclassified.
- **The contract is enforced across the language border**: CI regenerates the
  JSON Schema and TS types and fails on drift between committed and generated
  artifacts.
- **Prior art**: none in this repo (greenfield). The fixture-repo pattern
  mirrors the baselines' benchmark harness; the property-not-golden style is
  the norm to establish here.

## Out of Scope

- **Chat / explain / onboard commands** — out; they are cheap graph-grep
  consumers that can arrive later as thin skill wrappers over the published
  map (deferred in the interview, V2 candidates).
- **The Northstar → CodeAtlas producer skill** — out; it needs the published
  contract to exist first (parked in the intake doc).
- **A `claude -p` / local-model provider** — out; the provider trait keeps the
  door open (ADR-0004).
- **Incremental structural graph splicing** — out; full rescan is effectively
  free at V1 scale and splicing is a correctness trap (ADR-0005).
- **Figma, wiki/knowledge-base analysis, locales, theme engines, multi-platform
  installers, marketing site, demo mode** — out; the research doc identifies
  these as the baselines' bloat, and CodeAtlas has no installed base to serve.
- **Auto-update hooks on commit** — out; a run is fast enough to invoke
  deliberately, and hook machinery was a complexity source in both baselines.
- **PII/secret scanning of the codebase itself** — out; the security guarantee
  is about where data goes (egress, redaction), not about classifying the
  user's own code.

## Further Notes

- The V1 language list was confirmed — and extended with C and C++ — during
  `/to-tickets` on 2026-08-09.
- **Open question**: enrichment batching granularity (how many nodes' slots
  per API request) is left to implementation and should be settled by
  measurement, not up front.
- **Default enrichment model** is Claude Opus 5 (`claude-opus-5`),
  configurable; credential resolution per ADR-0004.
- `CONTEXT.md` does not exist yet; the glossary (node, edge, layer, tour,
  provenance, enrichment, sealed build, map contract, share artifact) should
  be seeded when implementation starts coining terms in code.
- Source material: [intake digest](../intake/2026-08-07-codeatlas-pitch-and-adr-agenda.md),
  [baseline research](../research/2026-08-07-baseline-repoatlas-understand-anything.md),
  [ADR index](../adr/README.md).

## Verification

`/harden` has walked all 17 user stories against the assembled system four
times. The first walk (**2026-08-09**, baseline `0a523da`) found one failure,
story 6, and filed it as ticket 16. The second walk (**2026-08-10**, baseline
`be320b1`, after ticket 16 landed) re-walked every story, not just the failed
one, because ticket 16 changed the tour projection, the map contract version,
and the dashboard. The third walk (**2026-08-10**, baseline `d8b535c`, after
ticket 17 landed) again re-walked every story, because ticket 17 changed
import resolution and edges are the substrate under the tour, the flows, the
layer projection and the diff blast radius. It found one failure, story 2,
and filed it as ticket 18. The fourth walk (**2026-08-10**, baseline
`1614a6a`, after tickets 18 and 19 landed) re-walked every story for the same
reason — ticket 18 changed import resolution again — and found story 2 failing
once more, on a third language. Filed as ticket 20.

Stories were driven through the real binaries — a default release build, a
sealed `--no-default-features` build, and a `test-provider` build for the
enrichment seam — against real repositories, not by reading code, except
where noted.

| # | Story | 08-09 `0a523da` | 08-10 `be320b1` | 08-10 `d8b535c` | 08-10 `1614a6a` |
|---|-------|-----------|-----------|-----------|-----------|
| 1 | Complete structural map in seconds, no LLM | pass | pass | pass | pass |
| 2 | Relationships: imports, calls, containment, exports | pass³ | pass³ | **fail** | **fail** |
| 3 | Interactive local dashboard | pass¹ | pass¹ | pass¹ | pass¹ |
| 4 | Opt-in enrichment fills prose slots | pass | pass | pass | pass |
| 5 | Re-runs re-purchase only changed content | pass | pass | pass | pass |
| 6 | Domain flows and an ordered guided tour | **fail** | **pass¹** | pass¹ | pass¹ |
| 7 | Diff's changed nodes and one-hop blast radius | pass | pass | pass | pass |
| 8 | Single self-contained redacted HTML export | pass¹ | pass¹ | pass¹ | pass¹ |
| 9 | Sealed build with no networking code, plus egress suite | pass | pass | pass | pass |
| 10 | Shared artifact discloses what was redacted | pass | pass | pass | pass |
| 11 | Structural graph rebuilt from scratch every run | pass | pass | pass | pass |
| 12 | Annotations re-attach only on unchanged content hash | pass | pass | pass | pass |
| 13 | Schema-guaranteed structured output, no repair machinery | pass² | pass² | pass² | pass² |
| 14 | Enrichment failure leaves a valid structural map | pass | pass | pass | pass |
| 15 | Unparseable files still appear as nodes | pass | pass | pass | pass |
| 16 | Published, semver-versioned map schema | pass | pass | pass | pass |
| 17 | Dashboard consumes only local files, zero external requests | pass | pass | pass | pass |

### The fourth walk: ticket 18 confirmed, and story 2 fails on a third language

**Ticket 18's repair holds.** Both Rust cases that failed on the third walk
now resolve: a crate naming itself from an integration test at the scan root,
and a workspace sibling under Cargo's `-`-to-`_` normalisation. On this
repository `crates/codeatlas/tests/share.rs` and the new `tests/embedded.rs`
reach `crates/codeatlas/src/`, and no probe lost an edge it previously had.

**Story 2 fails again, on Python.** `from pkg import util`, where `util` is a
module file, never reaches `pkg/util.py`: the resolver resolves the specifier
`pkg` and treats the imported name as a symbol only. With `pkg/__init__.py`
present the edge lands on the package initialiser — not false, since that file
really is executed, but the dependency on the module is invisible. Without it,
in a PEP 420 namespace package, there is no edge at all and the importer is an
orphan. Measured on two trees identical but for the presence of
`__init__.py`. Filed as **ticket 20**.

The fixture *looks* like it covers the form — `tests/fixtures/pyproj/` has
`from pkg import api` and `tests/scan.rs:1046` asserts the resulting edge —
but `api` there is a function re-exported from `pkg/__init__.py`, so the
package initialiser is the right answer in that case. The case where the
imported name is a **module** is untested, and no fixture has a namespace
package. That is a subtler version of the blind spot behind tickets 17 and 18:
not a missing form, but a form whose one tested instance happens to be the
instance that works.

Three walks have now found the same story failing on three languages, each
time through a wider probe rather than a change in standard. The pattern is
worth naming rather than repeating: **a language's import conventions are a
checklist, and the fixture that exercises one of them is not evidence for the
rest.** Ticket 20's probe list is the fourth iteration of that checklist and
should be carried into any future language.

### The third walk's failure: story 2, and the two passes it corrects

**Story 2** fails on 2026-08-10. The Rust parser drops every path that names a
crate in the scanned tree — `use codeatlas::map::…`, or across a workspace
`use atlas_engine::engine::run`. `parsers/rust.rs` resolves `crate::`,
`self::`, `super::` and `mod foo;`, and declines everything else as external,
at which point a crate that is *in the tree* is indistinguishable from
`serde`. Filed as **ticket 18**.

On CodeAtlas itself the cost is two dropped statements, both in
`tests/share.rs`. On a two-crate workspace — the ordinary shape of a Rust
project — it is total: zero inter-crate edges, every crate an island. This
repository being a single-crate workspace is exactly why the defect looked
cheap for two walks.

The ³ against story 2's earlier passes marks them as overclaims, in two
different ways, and both are worth naming rather than quietly restating:

- The **2026-08-09** pass was wrong for TypeScript. Under `NodeNext` the
  compiler obliges source to write `./x.js` for a file that is `x.ts`, and
  every such specifier was dropped — 38 of the dashboard's 46, islanding the
  whole subtree and leaving a Rust-plus-TypeScript project with a silently
  all-Rust tour. Ticket 17 fixed it; this walk confirms the repair (below).
- The **2026-08-10** (`be320b1`) pass repeated that error, and also missed the
  Rust gap now filed as ticket 18.

Both walks checked that the four edge kinds existed and were populated, which
they were, and never asked whether each language's *real* import conventions
resolved. That is the check this walk added: a probe repository per V1
language, written the way projects in that language are actually written.
TypeScript (NodeNext, `index` files), Python (absolute intra-project,
`from .x import`, `from . import x`, `import pkg.mod`), Go (module-path) and
C/C++ (quoted resolving, angled correctly declining) all passed. Rust was the
only failure. The lesson generalises past this spec: "the edge kind exists" is
not evidence that the edges exist.

### The first walk's failure, and its repair

**Story 6** failed on 2026-08-09: domain flows and the tour were produced
correctly and landed in the map file, but no consumer rendered either one, and
the tour was unbounded — one step per file node (148 on CodeAtlas itself,
3000 on a 3000-file repo), led by an integration-test file with fan-in 0 and
fan-out 0, with `lib.rs` nine steps behind it. This was a between-the-slices
gap: ticket 06 projected the flows, ticket 13 enriched their labels, ticket
08's scope stopped at "nodes and edges grouped by layer", and nobody owned
surfacing the result.

Ticket 16 closed both halves and the 2026-08-10 walk confirms it. On
CodeAtlas's own map the tour is now 12 steps over 154 files, opening at the
spec document and `lib.rs` and closing at `map.rs`, with `tests/scan.rs` gone
from the walk entirely; on the 3000-file synthetic repo it is also 12. The
dashboard surfaces a navigable walk that moves the canvas selection step by
step, and an index of domains expanding to flows and then to selectable call
chains; both carry the same provenance badge as node detail, and both
disappear cleanly when the map omits the field. The share artifact carries
the identical affordances with redacted labels intact.

### Between-the-slices checks

Beyond the story list, the 2026-08-10 walk probed the seams no single ticket
owned:

- **The whole pipeline in sequence on one repo** — `scan --enrich` → edit →
  `diff` → `share` → `serve`. The served dashboard delivered the enriched map
  (17/17 nodes, 7/7 tour steps narrated) and the overlay together; all four
  artifacts coexist under `.codeatlas/`.
- **The bounded tour meeting enrichment carry-over** — the new seam ticket 16
  created. On a 21-file repo with 75 purchased slots, adding one connected
  file cost 5 re-purchases: its own two nodes, the new flow it roots, the
  layer whose membership changed, and the one tour step whose cited fan-in
  changed. Tour membership itself stayed stable. Cost remains proportional to
  the delta, and the reason for every re-purchase is nameable.
- **Older maps through newer tooling** — a 0.1.0 map with no `layers`,
  `domain_flows`, or `tour` is served unchanged, shares successfully, and
  renders with the new panels simply absent.
- **A map that does not conform** — `share` refuses it by name rather than
  emitting a broken artifact.
- **Repositories with nothing to walk** — three unconnected files yield an
  empty tour, no error, and a complete map; 3000 files in one import cycle
  yield a 12-step tour and zero flows, because a flow needs a root nothing
  calls.

The 2026-08-10 (`d8b535c`) walk re-ran all of the above and added the seams
ticket 17 created:

- **Per-language import conventions**, described above — the check that found
  ticket 18, and the one whose absence let story 2 pass twice.
- **Ticket 17's repair, confirmed at the seam it broke.** Import edges 88 →
  129; `dashboard/src/app` 1 → 33; all 14 files under `dashboard/src/`
  connected; the tour from zero dashboard files to five, so a walk of a
  Rust-plus-TypeScript project is no longer silently all-Rust.
- **The blast radius that the missing edges had hidden** — editing
  `dashboard/src/app/graph.ts` now reports 5 changed nodes and a 5-file
  one-hop radius. Before ticket 17 the whole dashboard held one edge, so a
  reviewer asking the map for the risk of a dashboard change was shown
  nothing. This is story 7 depending on story 2, and it is the strongest
  argument that a dropped-specifier defect is never local to its parser.
- **Carry-over when topology changes but content does not** — the seam ticket
  17 opens, since it changes edges without changing any node ID or content
  hash. Adding one connected file cost exactly 2 slots; the file whose fan-in
  changed kept its prose (content-keyed, correctly) and an unanswered slot on
  the newcomer kept its mechanical label. Cost stays proportional to the
  delta.
- **Referential integrity of externally-produced maps** — a dangling edge is
  schema-legal, so neither the schema nor `share` rejects it. The dashboard is
  resilient rather than lucky: a map whose edge references a missing node, and
  one whose tour step does, both render without throwing. Recorded as a
  limitation below, not a failure — no CodeAtlas-produced map has ever carried
  one (zero dangling edges across all six maps validated this walk).
The 2026-08-10 (`1614a6a`) walk re-ran all of the above and added the seam
tickets 18 and 19 created:

- **A second round of import conventions**, the check that found ticket 20.
  Passing: TypeScript type-only imports, `export *` barrels, deep relative
  paths; Python aliased dotted imports; Rust aliased crate imports and
  `pub use` re-export chains; Go internal packages; C++ includes from a
  separate include directory. Failing: `from package import module`.
- **Ticket 19's split, tested by tampering.** Moving the egress build out of
  `dashboard/dist` meant the dashboard suite no longer scans the bytes that
  ship, and `crates/codeatlas/tests/embedded.rs` was added to cover them. That
  claim was verified rather than assumed: an external URL was appended to a
  built asset in `dist`, the binary rebuilt, and `embedded.rs` failed with
  `assets/index-…js: https://evil.example.com/collect` at the same moment the
  dashboard's own suite was green over its own fresh build. The gap ticket 19
  opened is genuinely closed, and the dashboard suite alone would not have
  caught it.
- **The egress suite's own vacuity guard.** `nonEmptyFilesUnder` refuses an
  empty build, and the guard test hands every check an empty directory, a
  partial one, and an `index.html` referencing nothing. All five netns egress
  tests genuinely asserted here rather than taking their documented skip —
  `unshare -r -n` works on this machine and no run printed `SKIPPED`.
- **An annotation store written before ticket 17, read after it** — attempted
  and *not* completed: the pre-fix worktree cannot build, because the build
  script compiles the dashboard and a fresh worktree has no
  `dashboard/node_modules` (the trap already recorded in `0a523da`). The seam
  was reached the other way instead, through the content-versus-topology test
  above, which is the property that actually matters: ticket 17 changed no
  node ID and no content hash, so no stored annotation could be invalidated
  by it.

### Notes on the passes

1. **Stories 3, 6 and 8** — no browser can be driven in this environment
   (headless Firefox fails to map a framebuffer), so neither the dashboard nor
   the share artifact has been watched painting. All three were driven through
   their real React components with real user events — 36 dashboard tests,
   including walking the tour and opening flow chains on CodeAtlas's own
   self-scan map, and share mode rendering with `globalThis.fetch` deleted
   outright — and the served dashboard was exercised over HTTP from the binary
   (`/` 200 html, `/api/map` 200 json, `/api/diff` 200, unknown paths 404,
   bound to 127.0.0.1 only), with the story-6 affordances confirmed present in
   the served production bundle. Layout and paint remain unwatched.
2. **Story 13** — verified at the provider seam: correctly addressed answers
   land, while an answer for a nonexistent node, an unprefixed key, a
   nonexistent layer, and a blank answer are all silently ignored rather than
   repaired. The Claude-facing half (`output_config.format` carrying a JSON
   schema, parsed exactly once, with a refusal or truncation an error rather
   than a repair attempt) was confirmed by reading `enrich/claude.rs`; it was
   not exercised against the live API, which would require credentials and
   spend. Re-confirmed on 2026-08-10 (`d8b535c`): of six answers offered to
   the provider — one correctly addressed, one for a nonexistent node, one
   unprefixed, one for a nonexistent layer, one blank, one for a nonexistent
   tour node — exactly the correctly addressed one landed, no phantom nodes
   were created, and the map stayed referentially intact.
3. **Story 2's pass on the first two walks was an overclaim**, in the ways set
   out above. It is recorded here rather than rewritten, because the point of
   this section is the audit trail: both walks confirmed the four edge kinds
   were populated without asking whether each language's real conventions
   resolved, and a verdict table that quietly changed its own history would
   hide the only lesson worth keeping.

### Other evidence recorded

Scans are byte-identical across repeated runs and identical cold vs. warm;
deleting a file removes its node and leaves no dangling edges, and restoring
it reproduces the exact map (story 11). CodeAtlas itself maps in 0.06s and
3000 files in 0.09s (story 1). Editing one file dropped re-purchase from 30
slots to 2; a plain rescan reverted exactly that file's prose to structural
and restoring its content re-attached the stored prose with no provider call
(stories 5 and 12). Enrichment failure, an unconfigured provider, and absent
credentials each leave a complete structural map behind and say so (story
14). All five network-namespace egress tests genuinely ran rather than
skipping (0 ignored), and both CI feature configurations are green — as of
`1614a6a`, **103** tests on the default build and **96** sealed, plus **45**
dashboard tests. The sealed binary's undefined network symbols are `bind`,
`listen`, `socket` and `socketpair` — std's `TcpListener`, which `serve` needs
for loopback — and notably **not** `connect` or `getaddrinfo`, which the
default build does link: the sealed build cannot open an outbound connection
or resolve a name, and its dependency tree contains no networking crates at
all (0, against 11 in the default build). `__tls_get_addr` also appears in
both and is thread-local storage, not TLS — a false friend worth naming so a
future auditor does not read it as a crypto stack. Regenerating the schema and
the dashboard's TS types produces no drift.

Story 17 was additionally audited at the artifact rather than the source: in
the production bundle served by the binary, the only `fetch` targets are
`/api/map`, `/api/diff` and Vite's modulepreload polyfill over local
`/assets`, all same-origin, with no `XMLHttpRequest`, `WebSocket`,
`EventSource` or `sendBeacon` anywhere. The absolute URLs it does contain are
inert strings: `w3.org` SVG/MathML namespaces passed to `createElementNS`, and
`react.dev`/`reactflow.dev` in library error text and the React Flow
attribution.

### Known limitations confirmed, not blocking

- The map contract's `version` pattern is release-only semver: `0.3.0-rc.1`
  and `1.0.0+build.5` are rejected. The contract README documents semver
  policy without stating that prereleases are disallowed.
- Enrichment carry-over is keyed at file-content granularity — a class and
  its file share a content hash — so touching one line in a large file
  re-purchases every node in it. Conservative and never stale, but coarser
  than "only the code that changed".
- The "mapped N files" message reports node count, not file count (693 vs.
  187 on CodeAtlas itself; 6000 vs. 3000 on the synthetic repo). Unchanged
  across four walks.
- **Referential integrity is not part of the contract.** A dangling edge —
  one whose `source` or `target` names no node — validates against the
  published schema and is accepted by `share`. It matters only for
  externally-produced maps (story 16), since CodeAtlas's own scans have never
  emitted one, and the dashboard renders such a map without throwing. Worth a
  decision in V2: either the schema cannot express it and the check belongs in
  `share`, or the contract README should say plainly that producers own it.
- **One dashboard egress assertion can pass without asserting anything.**
  Found 2026-08-10, after this walk, and filed as ticket 19: the egress test
  and the Rust build script both build into `dashboard/dist`, and when the
  build script wins the race the "no websocket endpoint" check loops over an
  empty directory and reports success. Story 17's verdict does not rest on it
  — the served production bundle was audited directly over HTTP, independently
  of `dist` — but the phrase "37 dashboard tests" above should be read knowing
  one of them cannot fail under the right timing.
- **The dashboard renders React Flow's attribution link** to
  `reactflow.dev?utm_source=attribution`, in the served page and in the share
  artifact. It is an `<a>`, not a request, so stories 8 and 17 hold — nothing
  is fetched and the artifact opens fully offline. But it is an off-origin
  link inside an artifact designed to be handed to third parties, and
  `proOptions.hideAttribution` is not set. A deliberate call either way, not
  an accident to leave undocumented.
- A partial-batch enrichment failure discarding earlier successful batches
  could not be reproduced through the CLI seam: both offline test providers
  are all-or-nothing, so this remains a code-review finding only.
- The tour is bounded but the **flow list is not**: CodeAtlas's own map now
  carries 154 flows, 143 of them in a single domain. The dashboard mitigates
  this by opening as a collapsed index of domains, but bounding or ranking
  flows themselves is a V2 question.
- Test-fixture files can earn tour slots. This repository contains eight
  miniature fixture repositories whose files genuinely participate in an
  import graph. As of `d8b535c` none of them holds a tour slot — the
  dashboard files that ticket 17 reconnected outrank them — but nothing
  prevents it, and excluding them would need a path heuristic the map contract
  cannot justify.

**Shipped status:** **not shipped.** 16 of 17 stories pass as of
**2026-08-10** (`1614a6a`); story 2 fails and is filed as ticket 20. Every
other story has now passed on four consecutive walks, and the failures have
been the same story each time — a different language's import conventions,
found by widening the probe rather than by lowering the bar.

Shipping needs ticket 20 built, a re-walk of story 2, and then explicit
acceptance of the two passes that rest on evidence short of watching the real
thing — the browser-unwatched rendering behind stories 3, 6 and 8, and the
unexecuted live-API half of story 13.

One judgement worth recording for whoever runs the next walk: story 2 is the
only story whose subject matter is open-ended. The other sixteen have a
bounded definition of done, and four walks have found nothing new in them.
Story 2's scope is "every import convention in six languages", so a fifth walk
that probes a seventh convention may well fail it a fourth time. That is the
story behaving as written, not the system regressing — but if the intent is
"the common conventions in each V1 language resolve", the story should be
rewritten to say so and given an explicit checklist, so that passing it means
something finite. Deciding that is a spec change, not a harden verdict.
