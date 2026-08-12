# CodeAtlas V2 agenda — harvested from the V1 ship

> Produced by `/next` on 2026-08-12, the day V1 shipped. Triage decisions in
> this doc are **Memnoc's**, made in the harvest session. This is an agenda
> for `/adr-with-docs`, not a commitment: nothing here is a spec, a glossary
> entry, or a decision until the interview makes it one.

## Where V1 landed

Shipped 2026-08-12. All 23 stories pass — six `/harden` walks plus the user's
own browser walk, recorded in the spec's `## Verification` with acceptance in
Memnoc's name (`docs/specs/2026-08-09-codeatlas-v1.md`). Suites at ship:
226/195/216 Rust across three build configurations, 194 dashboard, sealed
probe 11 checks. The map of this repository is enriched and the annotation
store is committed, so a plain clone explains itself. The spec is a record
now; V2 gets a new file.

## V2 candidates

**1. Dashboard visualization rework** — *the headline, user-confirmed
2026-08-12 (reaffirming the 2026-08-09 sequencing: polish before
distribution).*
Source: review harvest (memory `codeatlas-v1-review-harvest`), the
seven-ticket sketch (memory `codeatlas-visualization-ticket-sketch`),
`docs/intake/2026-08-10-dashboard-ui-reference.md`.
There is no layout algorithm: `graph.ts` positions nodes by index and edges
have zero influence on placement. The sketch's decisions for the ADR: dagre
vs ELK (sync vs async — ELK forces `toFlow()` async and reshapes the whole
dashboard test seam); default collapse aggressiveness; where the complexity
signal derives (lean Rust, per ADR-0003). Carry both pre-made insights:
progressive disclosure is load-bearing, not polish (the reference shows ~40
nodes, not 598); and tour curation + default disclosure + complexity band are
one question — *which nodes matter* — to be decided once, in Rust, not three
times in three heuristics.

**2. Multi-turn ask** — *user-supplied, added to V2 2026-08-12; needs its own
ADR.*
Source: spec Out of Scope ("Multi-turn conversation is out too — a question
and its answer, not a session"); the user asked for it twice on ship day.
The interview must settle: where conversation state lives (the serving
binary, per ADR-0009's spirit), how much history enters each prompt (a
stated bound, mechanically selected — the ADR-0009 pattern), and the budget
story (every turn is a provider call; ticket 40's estimate discipline
applies).

**3. Parser convention gaps** — source: review harvest §Parsers.
C++ namespaced calls (`geo::nsq()` never resolves; sparse edges on idiomatic
C++); Go package edges anchored to one file per package rather than the
defining file; Rust macro-interior calls (`println!`) invisible; C
`static inline` header functions unresolvable cross-file; Python duplicate
top-level `def` misattribution; Markdown self-link self-loop (skews fan-in).
Story 2's rewrite applies: each of these is a *spec change* to the convention
checklist, not a standing invitation — the interview decides which enter.

**4. Serve + security hygiene bundle** — source: deferred tickets
`.scratch/codeatlas-v1/36-*.md` and `38-*.md` (harvest them before `.scratch/`
is deleted), review harvest §Serve.
Ticket 36: the serve surface vs `docs/SECURITY.md` — three consecutive V1
tickets made that document false; the guard class is real. Ticket 38:
`read_headers` has no line cap, length cap, or deadline (loopback-only; the
false guarantee was already moved to Honest limitations). Plus: HEAD → 405
(RFC wants HEAD wherever GET), invalid UTF-8 → silent close instead of 400,
accept-error `continue` busy-spin risk, and a SECURITY.md limitations line
for the DNS channel (netns proves no TCP egress, not no-DNS).

**5. Distribution** — source: review harvest §Distribution; *sequenced after
1, per the user's standing decision.*
Tagged-release GitHub Actions workflow, static Linux/macOS binaries; the
dashboard is already embedded so one downloaded file works with zero runtime
dependencies. Release notes should say loudly that no key is needed for
scan/serve/diff/share/schema.

**6. Small dashboard warts** — source: ship-day session; `browser-walk.md`.
The header tally reads `STRUCTURAL · 0 structural, 1268 enriched` —
`structural` twice, grouping mode vs provenance. FILES-tab filter and fold
state reset on tab switch and sidebar fold (component state; hoisting into
`MapExplorer` is cheap). Both fold naturally into candidate 1.

**7. Seed `CONTEXT.md`** — source: spec Further Notes, promised and never
created. Every term is coined now: node, edge, layer, tour, provenance,
enrichment provider, annotation store, sealed build, map contract, share
artifact, the `agent-cli` feature, the ask route.

## Parked

- **Chat / explain / onboard CLI wrappers** — spec Out of Scope; the user's
  own do-not-start-unprompted list. The capability shipped in the dashboard;
  the command-line surface waits until something needs it.
- **Northstar → CodeAtlas producer skill** — *reconsidered 2026-08-12 because
  its blocker (a published contract) no longer exists, and parked again by
  the user.* Unblocked is not urgent.
- **Local-model provider** — spec Out of Scope; the provider trait keeps the
  door open (ADR-0004) and nothing yet needs a third backend.
- **Share/redaction hygiene** — review harvest: dual schema-walker divergence
  risk (recommend `redact()` bail on unwalkable constructs), disclosure
  banner conflates dropped vs replaced, `is_inert` duplicated cross-language,
  version pattern rejects prerelease semver (a surprise for story-16
  producers).
- **Enrichment internals** — review harvest: `fill_slots` fan-in/out tally
  duplicates `build_tour`'s byte-for-byte (divergence silently breaks
  carry-over hashing); flow-hash granularity re-purchases identically-
  prompted names; `MAX_TOKENS=8192` untested against the real model; no
  config-file model override. Note: the harvest's *partial-batch failure
  discards purchased summaries* is **fixed** (ticket 42, `394112f`).

## Dropped

- **Configurable API endpoint** (Bedrock/Vertex/gateway) — collides with the
  tested no-steering invariant; if ever revisited, an explicit flag the map
  *records* (spec Out of Scope, decided 2026-08-11; reaffirmed by omission).
- **Share artifacts from the dashboard** — the allowlist redaction in a
  second language is the copy that drifts and leaks (spec Out of Scope).
- **Incremental structural splicing** — full rescan is free at this scale;
  splicing is a correctness trap (ADR-0005).
- **Ticket 43, "what is worth enriching"** — *dropped by the user 2026-08-12,
  with the measurement attached rather than despite it.* The pain it targeted
  (44-minute silent runs, total-loss on interruption, cost surprise) was
  dissolved by tickets 40 and 42; function summaries feed the ask slice, so
  skipping them costs answer quality; the ticket's own text predicted this
  disposition and called it a successful outcome.
- **Auto-update hooks, PII/secret scanning, Figma-class features** — the
  baselines' recorded bloat (spec Out of Scope; research doc).

## Open questions — for the interview to grill first

1. **dagre or ELK** — and whether the sync-vs-async consequence for the
   `toFlow()` test seam decides it on its own (sketch decision 1).
2. **Which nodes matter** — the one rule behind tour curation, default
   disclosure, and the complexity band; where it lives (lean Rust) and what
   its first version measures (sketch, second insight).
3. **Multi-turn ask** — state location, history bound, budget line. Does
   ticket 40's "say what it costs before spending" discipline extend to a
   conversation?
4. **Which parser gaps enter story 2's checklist** — each is a spec change;
   C++ namespaces look highest-value (idiomatic C++ currently near-edgeless).
5. **The share artifact grows with every dashboard improvement** (layout
   library bundles in; it was 849 KB, now 1.3 MB) — is there a ceiling, and
   who enforces it?
6. **Annotation-store review** — the recorded-not-solved consequence (spec
   Further Notes): committed prose nobody reviewed line by line, arriving
   through a normal pull. Does V2 owe the reviewer anything beyond the
   provider/model/date fields?

## Hand-off

Fresh session, `/adr-with-docs`, this document as the agenda. V2 walks the
same road V1 did; its spec is a new file — a spec that shipped is a record,
never edited into a new version.
