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

`/harden` walked all 17 user stories against the assembled system on
**2026-08-09**, after all 15 build tickets read `done` (baseline `0a523da`;
a concurrent session landed the README commit `2aff081` partway through,
which touches no code).
Stories were driven through the real binaries — a default build, a sealed
`--no-default-features` build, and a `test-provider` build for the enrichment
seam — against real repositories, not by reading code, except where noted.

| # | Story | Verdict |
|---|-------|---------|
| 1 | Complete structural map in seconds, no LLM | pass |
| 2 | Relationships: imports, calls, containment, exports | pass |
| 3 | Interactive local dashboard | pass¹ |
| 4 | Opt-in enrichment fills prose slots | pass |
| 5 | Re-runs re-purchase only changed content | pass |
| 6 | Domain flows and an ordered guided tour | **fail** |
| 7 | Diff's changed nodes and one-hop blast radius | pass |
| 8 | Single self-contained redacted HTML export | pass¹ |
| 9 | Sealed build with no networking code, plus egress suite | pass |
| 10 | Shared artifact discloses what was redacted | pass |
| 11 | Structural graph rebuilt from scratch every run | pass |
| 12 | Annotations re-attach only on unchanged content hash | pass |
| 13 | Schema-guaranteed structured output, no repair machinery | pass² |
| 14 | Enrichment failure leaves a valid structural map | pass |
| 15 | Unparseable files still appear as nodes | pass |
| 16 | Published, semver-versioned map schema | pass |
| 17 | Dashboard consumes only local files, zero external requests | pass |

### The failure

**Story 6 — domain flows and an ordered guided tour.** Both artifacts are
produced correctly and land in the map file, but **no consumer renders
either one**: `tour` and `domain_flows` appear in `dashboard/src/` only in
the generated types and the `index.ts` re-export. A newcomer can reach the
tour only by reading raw JSON — precisely what story 3 says the dashboard
exists to prevent. The tour is also unbounded: exactly one step per file
node (148 on CodeAtlas itself, 3000 on a 3000-file repo, including
`Cargo.lock` and every fixture), and its top-ranked step on CodeAtlas's own
map is an isolated file with fan-in 0 and fan-out 0, ahead of `lib.rs` at
step 9.

This is a between-the-slices gap, not a ticket defect: ticket 06 projects
flows and orders the tour, ticket 13 enriches their labels, and ticket 08's
scope stops at "nodes and edges grouped by layer". Each passed its own
criteria; nobody owned surfacing the result. Filed as
`.scratch/codeatlas-v1/16-surface-flows-and-tour.md` (`Status: ready`),
covering both the missing affordances and the tour's boundedness.

### Notes on the passes

1. **Stories 3 and 8** — no browser was available in the harden session, so
   neither the dashboard nor the share artifact was watched rendering in a
   real browser. Both were driven through their real React components with
   real user events (22 dashboard tests: canvas nodes and edges, layer
   groups, search narrowing, node detail with provenance badge, edge
   navigation, and share mode rendering with `globalThis.fetch` deleted
   outright), and the served dashboard was exercised over HTTP from the
   binary. Layout and paint remain unwatched.
2. **Story 13** — verified at the provider seam: canned typed answers land in
   the right slots, and three deliberately malformed keys (nonexistent node,
   wrong prefix, no prefix) were silently ignored rather than repaired. The
   Claude-facing half (`output_config.format` with a JSON schema, parsed
   exactly once, no repair or retry-on-parse path) was confirmed by reading
   `enrich/claude.rs`; it was not exercised against the live API, which would
   require credentials and spend.

Other evidence worth recording: scans are byte-identical across repeated runs
and identical cold vs. warm (story 11); 3000 files map in 0.56s (story 1);
editing one file dropped re-purchase from 14 slots to 4 (story 5); restoring
a file's original content re-attached its stored prose with no provider call
(story 12); all five network-namespace egress tests genuinely ran here rather
than skipping, and both CI feature configurations are green (59 and 60 tests);
the sealed build links no networking crates at all and every command works in
it; and regenerating the schema and the dashboard's TS types produced no drift.

### Known limitations confirmed, not blocking

- The map contract's `version` pattern is release-only semver: `0.3.0-rc.1`
  and `1.0.0+build.5` are rejected. The contract README documents semver
  policy without stating that prereleases are disallowed.
- Enrichment carry-over is keyed at file-content granularity — a class and
  its file share a content hash — so touching one line in a large file
  re-purchases every node in it. Conservative and never stale, but coarser
  than "only the code that changed".
- The "mapped N files" message reports node count, not file count (599 vs.
  148 on CodeAtlas itself).
- A partial-batch enrichment failure discarding earlier successful batches
  could not be reproduced through the CLI seam: both offline test providers
  are all-or-nothing, so this remains a code-review finding only.

**Shipped status:** 16 of 17 stories pass. Story 6 is open on ticket 16;
this section should be updated after that ticket lands and harden re-walks it.
The two browser-dependent passes and the live-API half of story 13 are
recorded above as unwatched and await explicit acceptance.
