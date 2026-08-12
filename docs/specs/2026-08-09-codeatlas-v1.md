# CodeAtlas V1

> Crystallised from the 2026-08-07 `/adr` interview. Decisions live in
> [docs/adr/](../adr/README.md); evidence in
> [the baseline study](../research/2026-08-07-baseline-repoatlas-understand-anything.md);
> the original pitch in
> [the intake digest](../intake/2026-08-07-codeatlas-pitch-and-adr-agenda.md).
>
> **Amended 2026-08-11** from a second `/adr` interview
> ([ADR-0007](../adr/0007-the-annotation-store-is-a-committed-repository-artifact.md),
> [ADR-0008](../adr/0008-enrichment-through-an-authenticated-claude-cli-behind-its-own-feature.md),
> [ADR-0009](../adr/0009-codebase-questions-are-answered-by-the-serving-binary.md)),
> which added stories 18–21, rewrote story 2, and amended stories 4 and 9.
> Amended passages carry their date. The spec is amended rather than
> superseded because `/harden` walks one numbered story list per release, and
> all four new stories are V1 scope.

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

**Added 2026-08-11.** There is a second barrier, and it stops the same people
for a different reason. Everything that makes the map *explain itself* — the
prose summaries, the named layers, the narrated tour — is gated behind an
Anthropic API key, and in many organisations only administrators can obtain
one. A developer who cannot get a key gets a structurally complete map with
nothing but mechanical labels on it, and there is no way for a colleague who
*did* get one to hand over the explanations. Meanwhile the reader who most
needs those explanations — someone new to the codebase — is also the reader
least able to guess what to search for, because searching by name presumes you
already know the names.

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

**Added 2026-08-11.** Enrichment stops being a per-developer purchase. One
person on a team enriches once and commits the result, so everyone who clones
the repository gets the explanations offline and without a credential of their
own — and that one person no longer needs an API key either, because
enrichment can run through a Claude CLI they are already logged into. Once a
map is open, a reader can ask it a question in their own words instead of
guessing at names, and be shown where in the code the answer lives.

## User Stories

1. As a developer, I want a complete structural map of a codebase in seconds
   with no LLM involved, so that mapping a repo is something I do casually
2. As a developer, I want the map to capture relationships — imports, calls,
   containment, exports — not just a file tree, so that I can trace how
   components actually connect. **Rewritten 2026-08-11**: satisfied when every
   convention in the checklist below resolves, in each of the six V1
   languages where the convention exists. Adding a convention to the
   checklist is a spec change; the story is not a standing invitation to find
   a seventh.
   - *Imports*: a plain module import; a named/member import; an aliased
     import; a namespace or whole-module import; a relative import; a
     package-or-directory import resolving through an initialiser or index
     file; a header/source pairing (C/C++ only)
   - *Calls*: an unqualified call to an imported name; a qualified call
     through an imported module; a qualified call through an aliased module;
     a qualified call through a nested module path
   - *Non-edges, which matter as much*: a call whose receiver is a value
     rather than a module; a call into a package outside the repository; an
     import that resolves to no file in the repository
3. As a developer, I want an interactive local dashboard (graph canvas,
   search, layer grouping, node detail), so that I navigate the map instead of
   reading JSON
4. As a developer, I want to opt into LLM enrichment that fills prose slots —
   node summaries, layer names, domain-flow names, tour narration — so that
   the map explains itself to someone who doesn't know the code.
   **Amended 2026-08-11**: through whichever enrichment provider is selected,
   which is no longer presumed to be the Claude API — see story 19
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
   CodeAtlas is a code review rather than a trust exercise.
   **Amended 2026-08-11**, to the sentence every document must now hold to:
   *CodeAtlas has exactly two ways to reach a model — an HTTPS POST to
   `api.anthropic.com`, and spawning the already-authenticated `claude` CLI.
   Each sits behind its own Cargo feature; each is reachable only from
   `scan --enrich` and `serve --ask`. The sealed build has neither.* Three
   build configurations are therefore auditable, not two: both features,
   neither, and the CLI without the HTTP client
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
    degrades and never breaks the product. **Amended 2026-08-11**: the failure
    list now includes the CLI provider's own modes — the program is not
    installed, it is installed but not logged in, or it exits non-zero — and
    the requirement is unchanged for each
15. As the CLI, I want files I cannot parse (unsupported language, syntax
    errors) to still appear as file nodes with whatever edges are resolvable,
    so that the map is complete even where analysis is shallow
16. As an external producer (for example a future Northstar skill), I want a
    published, semver-versioned JSON Schema for the map format, so that I can
    emit files the CodeAtlas dashboard renders
17. As the dashboard, I want to consume only local files and make zero
    external requests, so that viewing a map is fully offline. **Confirmed
    unchanged 2026-08-11**: story 21 adds a question box, but the request it
    makes is same-origin to the local server — the same category as the
    `/api/map` request the dashboard already makes — so this story stands
    word for word

*Stories 18–21 added 2026-08-11.*

18. As a developer with no API key of my own, I want enrichment someone else
    already paid for to arrive with the repository, so that cloning is all I
    have to do to get a map that explains itself
19. As a developer whose organisation will not issue me an API key, I want to
    enrich through a Claude CLI I am already logged into, so that CodeAtlas
    never handles a credential and I never have to ask an administrator for
    one
20. As a first-time user of the dashboard, I want a walkthrough that
    highlights each control in the live interface and says what it does, so
    that I learn the application without clicking every button to find out
21. As a newcomer to a codebase, I want to ask the map a question in my own
    words and be shown which nodes answer it, so that I can find things
    before I know what they are called

*Stories 22–23 added 2026-08-12, after every ticket was already `done`. Both
describe work that had already shipped — the pipeline running backwards, which
is worth admitting in the spec rather than hiding in a ticket. They are here so
that `/harden` walks them; a shipped feature with no story is not verified, it
is merely unmentioned. See tickets 39 and 44.*

22. As a reader looking at a large map, I want the panel and the region chips
    to fold away — separately, or both at once — so that the canvas gets the
    space, and I want what I folded to still be folded when I come back.
    **Verifiable by eye only**: jsdom lays nothing out, so no test in this
    repository can show that anything got bigger; the committed tests assert
    which controls survive a fold, that nothing folds away the only route to
    something, and that the choice is remembered
23. As a reader running `codeatlas serve` without `--ask`, I want to be told
    that questions exist and how to turn them on, so that a feature the
    dashboard correctly hides is not a feature I can never discover. The
    dashboard must stay silent about it — advertising a question this server
    cannot answer is worse than saying nothing — so the terminal is where it
    belongs, and a build compiled with no backend at all must not offer a flag
    it cannot honour

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
  tests; schema-derived redaction exhaustiveness. Its rule is unchanged by the
  2026-08-11 amendments; the list of what sits behind the gate is not
- [ADR-0007](../adr/0007-the-annotation-store-is-a-committed-repository-artifact.md)
  — the annotation store is a committed repository artifact
- [ADR-0008](../adr/0008-enrichment-through-an-authenticated-claude-cli-behind-its-own-feature.md)
  — enrichment can run through an already-authenticated Claude CLI, behind its
  own `agent-cli` feature
- [ADR-0009](../adr/0009-codebase-questions-are-answered-by-the-serving-binary.md)
  — codebase questions are answered by the serving binary, not the dashboard

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

*Added 2026-08-11:*

- **The annotation store is published, the map file is not.** Every scan
  writes a `.codeatlas/.gitignore` that ignores the regenerated map and
  publishes the annotation store, so the artifact exists without anyone
  remembering a second command. The store gains provider, model, and date
  fields: prose entering code review has to say what produced it. *Refined
  2026-08-11 during ticket 30, and recorded in ADR-0007: a scan writes that
  file when it is absent and never overwrites an edited one.*
- **Committing prose and redacting prose are the same policy, not opposite
  ones.** The line is the trust boundary. A share artifact goes to a recipient
  chosen at send time who does not hold the source, so its prose is redacted;
  a committed annotation store reaches only people who already hold the code
  it describes, so it discloses nothing new.
- **Provider selection gets a first-class surface.** A `--provider` flag joins
  the existing `CODEATLAS_ENRICH_PROVIDER` variable. The recognised CLI spec
  is `cli:claude` and nothing else — a generic `cli:<program>` would make
  "CodeAtlas executes whatever you name it" a true sentence, which is a much
  worse claim to defend than "CodeAtlas can invoke `claude`".
- **The spawned CLI is a completion, not an agent.** No tools, no MCP servers,
  a working directory outside the repository, and an allowlisted environment
  (`PATH`, `HOME`, `XDG_*`) that deliberately excludes `ANTHROPIC_API_KEY` so
  that `cli:` means the CLI's own credential rather than silent API billing.
  Without the tool lockdown the CLI could read files through its own tooling
  and void the standing guarantee that the model never receives file contents.
- **Questions are answered by the binary, over `POST /api/ask`, behind an
  explicit `serve --ask`.** This is `serve`'s first non-GET verb. Without the
  flag `serve` stays provably egress-free. Questions travel through the same
  provider trait as enrichment, so both credential paths work without a second
  integration, and they are answered from a bounded slice of the map alone —
  never file contents — with answers citing node IDs so a reader can check
  them. The feature works on an unenriched map, answering from mechanical
  summaries; gating it on enrichment would add a way to fail for a reason the
  reader cannot see.
- **The walkthrough of the application is not the tour of the codebase.** Two
  things called "tour" in one product confuses both the code and the reader,
  so story 20's feature and story 6's are named distinctly in the UI, and
  starting one must not leave the other half-running.

## Testing Decisions

Two seams, confirmed 2026-08-09; two more added 2026-08-11 for work the
original two cannot see. All tests assert external behaviour at these
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
- **Seam 3 — the spawned program's process interface** (added 2026-08-11, for
  story 19). The subprocess sits *below* the provider trait, so seam 2 cannot
  see the four things that actually break: argv construction, environment
  scrubbing, stdout parsing, and exit-code handling. Tests point the provider
  at a **fake executable** that echoes canned JSON and assert on what it was
  invoked with and what the provider made of what came back — no network, no
  credentials, no spend. The injection point is gated behind the
  `test-provider` feature exactly as the `fake:`/`fail` backends already are,
  so no shipped binary gains a way to run an arbitrary program.
- **Seam 4 — `POST /api/ask` over real HTTP** (added 2026-08-11, for story
  21). Run the real binary, speak HTTP/1.1 to it over 127.0.0.1, assert on the
  response — and assert that the route is absent without `--ask`. The
  *bounding* half of the story asserts at seam 2 instead: what reaches the
  provider, and that it stays within the stated bound however the question is
  phrased.
- **The three build configurations are all tested** (amended 2026-08-11): both
  features, neither, and `agent-cli` without `network`. The third is the
  configuration ADR-0008 exists to make expressible; untested, it is a claim
  rather than a guarantee. The sealed build's new refusal needs a
  differently-shaped proof from the old one, because `cargo tree` cannot see a
  subprocess: a behavioural test that the sealed binary rejects `cli:claude`,
  and a byte probe finding no `claude` program string — both with the default
  build as a live control, or neither asserts anything.
- **Prior art**: the fixture-repo pattern and property-not-golden style
  established across `crates/codeatlas/tests/`. Story 18's test follows the
  temp-git-repo shape the egress suite already uses; story 21's follows the
  run-the-real-binary-and-speak-HTTP shape of the serve suite; story 20's
  follows the dashboard suite's real-components-and-real-user-events shape.
  Story 2's rewritten checklist becomes a fixture table, one row per
  convention per language, so a gap reads as a failing row rather than as an
  absence nobody notices.

## Out of Scope

- **Chat / explain / onboard *commands*** — still out as CLI subcommands; they
  are cheap graph-grep consumers that can arrive later as thin skill wrappers
  over the published map (deferred in the 2026-08-07 interview). **Narrowed
  2026-08-11**: story 21 brings question-answering into the dashboard over
  `POST /api/ask`, so what remains out is the *command-line* surface, not the
  capability. Multi-turn conversation is out too — a question and its answer,
  not a session.
- **The Northstar → CodeAtlas producer skill** — out; it needs the published
  contract to exist first (parked in the intake doc).
- **A `claude -p` provider** — **brought in scope 2026-08-11** as story 19
  ([ADR-0008](../adr/0008-enrichment-through-an-authenticated-claude-cli-behind-its-own-feature.md)).
  ADR-0004 rejected it partly because its output was free text; the CLI's
  `--json-schema` retired that objection, and the credential barrier made it
  worth building rather than merely possible.
- **A local-model provider** — still out; the provider trait keeps the door
  open (ADR-0004), and nothing in V1 needs a third backend.
- **A configurable API endpoint** (Bedrock, Vertex, a corporate gateway) —
  out, decided 2026-08-11; it would collide with the tested invariant that the
  transport can be steered nowhere. If ever revisited, the honest shape is an
  explicit flag that the map *records*.
- **Generating share artifacts from the dashboard** — out; it would put
  ADR-0006's allowlist redaction into a second language, and the copy that
  drifts is the one that leaks. The CLI stays the only thing that writes one.
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
  be seeded when implementation starts coining terms in code. **The
  2026-08-11 amendments add terms worth glossing at the same time**:
  annotation store, enrichment provider, the `agent-cli` feature, and the ask
  route.
- Source material: [intake digest](../intake/2026-08-07-codeatlas-pitch-and-adr-agenda.md),
  [baseline research](../research/2026-08-07-baseline-repoatlas-understand-anything.md),
  [ADR index](../adr/README.md).

*Added 2026-08-11:*

- **`README.md` and `docs/SECURITY.md` currently make claims the amended story
  9 contradicts** — that `--enrich` is the only egress-capable command and its
  only possible destination is `api.anthropic.com`. They are deliberately left
  alone until the code changes, because they describe what the code does and
  editing them now would make three *tested* claims false. They change with
  the implementation, held to story 9's sentence.
- **Open question**: how the bounded slice for a question is selected, and
  what the bound is. ADR-0009 requires a stated bound and mechanical
  selection; the number and the ranking rule are for implementation to settle
  by measurement, in the same spirit as the enrichment batch size above.
- **Open question**: whether story 20's walkthrough steps are hand-written or
  derived from the components present. A hand-written list goes stale the next
  time the header changes; a derived one may not have anything useful to say.
  *Settled 2026-08-11 in ticket 26: both halves, because they come from
  different places. The prose is hand-written — nothing can derive "Domain
  groups files by the call flows the map found" from a DOM node — and lives in
  `dashboard/src/app/walkthrough.ts`; which of those steps a given page is
  walked through is read off the live interface by `resolveWalkthroughSteps`,
  so a share artifact with no question box is never told about one. The
  staleness objection is answered as a test problem, by two drift guards in
  `dashboard/tests/walkthrough.test.tsx` that hold the declaration and the
  rendered interface together in both directions.*
- **Story 13 stops being unverifiable once story 19 lands.** Five `/harden`
  walks have recorded story 13 as verified only at the provider seam, because
  the Claude-facing half needs credentials and real spend. The CLI provider
  makes a genuine end-to-end enrichment run possible with a Claude Code seat
  and no API key — which is an argument for sequencing story 19 *before* the
  sixth walk rather than after it.
- **Known consequence, recorded rather than solved**: an annotation store
  committed by one person is prose nobody else reviewed line by line, arriving
  through a normal pull. The store's new provider/model/date fields exist so a
  reviewer can at least see what produced it.

## Verification

`/harden` has walked all 17 user stories against the assembled system five
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

The fifth walk (**2026-08-10**, baseline `e68c184`, after ticket 20 and the
dashboard rework landed) re-walked every story again: ticket 20 changed import
resolution a third time, and `e68c184` replaced the dashboard wholesale, which
is stories 3, 6, 7 and 8 in one commit. **Ticket 20's repair holds** — all
four Python forms it was filed for now resolve. **Story 2 fails a third
consecutive walk**, this time on an axis no previous walk had probed: not
import conventions but *call* conventions. Filed as ticket 21.

Stories were driven through the real binaries — a default release build, a
sealed `--no-default-features` build, and a `test-provider` build for the
enrichment seam — against real repositories, not by reading code, except
where noted.

| # | Story | 08-09 `0a523da` | 08-10 `be320b1` | 08-10 `d8b535c` | 08-10 `1614a6a` | 08-10 `e68c184` |
|---|-------|-----------|-----------|-----------|-----------|-----------|
| 1 | Complete structural map in seconds, no LLM | pass | pass | pass | pass | pass |
| 2 | Relationships: imports, calls, containment, exports | pass³ | pass³ | **fail** | **fail** | **fail** |
| 3 | Interactive local dashboard | pass¹ | pass¹ | pass¹ | pass¹ | pass¹ |
| 4 | Opt-in enrichment fills prose slots | pass | pass | pass | pass | pass |
| 5 | Re-runs re-purchase only changed content | pass | pass | pass | pass | pass |
| 6 | Domain flows and an ordered guided tour | **fail** | **pass¹** | pass¹ | pass¹ | pass¹ |
| 7 | Diff's changed nodes and one-hop blast radius | pass | pass | pass | pass | pass¹ |
| 8 | Single self-contained redacted HTML export | pass¹ | pass¹ | pass¹ | pass¹ | pass¹ |
| 9 | Sealed build with no networking code, plus egress suite | pass | pass | pass | pass | pass |
| 10 | Shared artifact discloses what was redacted | pass | pass | pass | pass | pass |
| 11 | Structural graph rebuilt from scratch every run | pass | pass | pass | pass | pass |
| 12 | Annotations re-attach only on unchanged content hash | pass | pass | pass | pass | pass |
| 13 | Schema-guaranteed structured output, no repair machinery | pass² | pass² | pass² | pass² | pass² |
| 14 | Enrichment failure leaves a valid structural map | pass | pass | pass | pass | pass |
| 15 | Unparseable files still appear as nodes | pass | pass | pass | pass | pass |
| 16 | Published, semver-versioned map schema | pass | pass | pass | pass | pass |
| 17 | Dashboard consumes only local files, zero external requests | pass | pass | pass | pass | pass |

### The fifth walk: ticket 20 confirmed, and story 2 fails on a new axis

**Ticket 20's repair holds.** All four Python forms it was filed for resolve:
`from pkg import util` reaches `pkg/util.py`; the same form reaches
`ns/parse.py` in a PEP 420 namespace package with no `__init__.py`; the
relative `from .util import helper` inside an initialiser resolves; and
`from pkg import shadow`, where a module and a symbol share a name, reaches
the module. No probe lost an edge it previously had.

**The dashboard rework was re-walked in full.** `e68c184` replaced the
explorer, so stories 3, 6, 7 and 8 were re-driven against the real self-scan
map through the real components: 8 region cards on the canvas, the grouping
switch, search selecting `graph.ts` and opening its detail panel with an edge
list, drilling into `crates` drawing all 134 file cards; the tour walked
**12 of 12** steps in order with exactly one canvas node selected at each; the
flows panel opening domain → flow → 5 ordered steps and selecting a step on
the canvas; the diff overlay marking 1 changed and 8 affected file cards for a
real `git` edit. The share artifact is one 438 KB file with nothing referenced
from disk.

**Story 2 fails on call conventions.** A call edge is produced only when the
callee is an *unqualified name bound directly by an import*. Every qualified
form — reaching a function through the module holding it — resolves to
nothing, in every language: Python `util.helper()` and `pkg.util.other()`,
TypeScript `import * as util` then `util.helper()`, Rust `util::helper()` and
`crate::util::helper()`, and Rust `use util::helper;` (a bare local-module
path in a `use`, which the resolver declines even though `mod util;` already
produced the file edge). The import edges are correct in all of these; it is
the call-binding step alone.

The cost is not marginal. CodeAtlas's own map holds **7** cross-file Rust call
edges against 447 same-file ones for a 134-file crate, and `src/lib.rs` — the
CLI's whole command dispatch — has **zero** outgoing call edges despite
calling `scan::scan`, `enrich::run`, `diff::run` and `serve::serve`. The map
cannot trace the program's own top-level control flow. It surfaces in the
product too: domain flows are projected from call chains, so the dashboard's
Domain grouping files **134 of 219** files under `No call flow`. Filed as
**ticket 21**.

This is the fourth iteration of one pattern, and the axis is the news. Walks
three and four widened the *import*-convention checklist and found a language
gap each time; this walk kept that checklist green and widened a different
one. The spec's existing lesson generalises: **a language's call conventions
are a checklist too, and the fixture that exercises one of them is not
evidence for the rest.** Whatever the sixth walk probes, it should be an axis
no earlier walk has touched, not a wider sweep of one already green.

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

The 2026-08-10 (`e68c184`) walk re-ran the seam checks above and added the
seams ticket 20 and the dashboard rework created:

- **Call conventions across four languages** — the new axis, described above,
  and the check that found ticket 21.
- **The `build.rs` `NODE_ENV` fix, tested by tampering.** `2f6894d` exists
  because vitest's `NODE_ENV=test` propagated through `self-scan.test.tsx` →
  `cargo` → `build.rs` and planted a development React bundle in `dist/` — an
  ADR-0006 breach in the shipped binary. Verified rather than assumed: an
  external URL was appended to a built asset, and `embedded.rs` failed by name
  (`assets/index-BobAGIlu.js: https://evil.example.com/collect`); then a
  forced rebuild under `NODE_ENV=test` produced a **production** bundle
  (2 minified-error markers, 0 development markers) and the suite went green.
  Both halves of the claim hold: the guard catches tampering, and the fix
  survives a hostile ambient environment.
- **The rewritten renderer meeting the share artifact.** The share artifact
  inlines the same renderer the rework replaced, so it was re-checked end to
  end on an enriched map: 438 KB, one file, no `src`/`href` to disk, and only
  the three documented-inert hostnames. All 14 enriched prose strings are
  absent from the artifact and present in the map — redaction is real, not
  cosmetic — and the artifact embeds its own redaction record (marker, the
  four-field policy, per-field counts) behind a rendered disclosure banner.
- **The egress allowlist re-read as policy, not convenience.** The shipped
  bundle contains `react.dev` (2), `reactflow.dev` (3) and `w3.org` (31). All
  are excused by `tests/common::is_inert`, which is a documented allowlist for
  XML namespace identifiers, React's minified-error decoder text, and React
  Flow's attribution anchor. The distinction is load-bearing: the development
  bundle's URLs are *not* on that list, which is why `embedded.rs` caught the
  breach `2f6894d` fixes. The byte probe of the sealed binary was re-run with
  `strings` after `grep -c` returned a vacuous zero for both builds — the
  corrected probe shows `api.anthropic.com` 1/0, `x-api-key` 1/0,
  `anthropic-version` 1/0, `ureq` 410/0, `rustls` 1091/0 for default/sealed.
- **Enrichment carry-over at symbol granularity.** Appending one function to a
  three-function file reverted that file *and all three of its symbols* to
  structural provenance while untouched files kept their prose, and the next
  enrich re-purchased **4 of 14** slots. Reverting a whole file's symbols is
  more conservative than strictly required — `greet` and `format` were
  textually unchanged — but it errs toward never leaving stale prose, and cost
  stays proportional to the changed file rather than the repository.
- **Degenerate repositories, re-run.** An empty repository maps 0 files; three
  unconnected files give 3 nodes, 0 edges, an empty tour and no flows; a
  30-file import cycle gives a 12-step tour and zero flows. All three validate
  against the published schema, as do four other maps produced this walk.

### Notes on the passes

1. **Stories 3, 6, 7 and 8** — no browser can be driven in this environment
   (headless Firefox fails to map a framebuffer), so neither the dashboard nor
   the share artifact has been watched painting. All were driven through
   their real React components with real user events — 86 dashboard tests as
   of `e68c184`, including walking the tour and opening flow chains on
   CodeAtlas's own self-scan map, and share mode rendering with
   `globalThis.fetch` deleted outright — and the served dashboard was
   exercised over HTTP from the binary (`/` 200 html, `/api/map` 200 json,
   `/api/diff` 200, unknown paths 404, bound to 127.0.0.1 only), with the
   story-6 affordances confirmed present in the served production bundle.
   Layout and paint remain unwatched.

   Story 7 carries this footnote from the fifth walk onward: before
   `e68c184` the overlay was asserted on node marks, which needed no canvas;
   the reworked explorer rolls the overlay up to the file cards it draws, so
   the verdict now rests on rendered canvas nodes (1 changed, 8 affected in
   the `dashboard` region for a real `git` edit) like the other three.
   The underlying overlay file is still checked directly and remains
   browser-independent: 17 changed nodes and 14 affected, deterministic.

   Two motion behaviours added in `e68c184` are **unverified** rather than
   passed: the 140 ms canvas transitions and the 240 ms viewport easing. jsdom
   does not run CSS transitions and the viewport duration is not observable
   through the DOM, so both were confirmed only by inspecting the compiled
   bundle (the transition rule and its `prefers-reduced-motion` override are
   present in the shipped CSS). Nothing in the story list depends on them.
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
- ~~The "mapped N files" message reports node count, not file count (693 vs.
  187 on CodeAtlas itself; 6000 vs. 3000 on the synthetic repo). Unchanged
  across five walks — 801 vs. 219 at `e68c184`. It has now been carried long
  enough to be a decision rather than an oversight: it is the only place the
  CLI states a number to the user that is not true, and it is a one-word fix
  at `lib.rs:100`.~~ **Fixed 2026-08-11 in ticket 34**, which was the next
  work to touch that function; the count is now of `file`-kind nodes, with a
  test that fails if it reverts to `nodes.len()`.
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
