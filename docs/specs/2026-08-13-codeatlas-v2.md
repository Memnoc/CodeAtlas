# CodeAtlas V2 — the polish lap

> Produced by `/to-spec` on 2026-08-13 from the `/adr-with-docs` interview of
> the same day. Decisions are Memnoc's, recorded in ADR-0010, ADR-0011 and
> ADR-0012; the agenda was `docs/intake/2026-08-12-codeatlas-v1-next.md`.
> This spec absorbs deferred tickets 36 and 38, so `.scratch/codeatlas-v1/`
> is now fully disposable.

## Problem Statement

A newcomer opens a served map of a real repository and drills into its
biggest region — and the map stops being a picture exactly where the code is
densest. CodeAtlas's own `crates` region draws 159 file cards at once, every
edge converging on one centre point per card, and the card that brought them
there says only "Files under crates/", which they already knew from the
directory name.

The same newcomer asks the map a question and gets a good answer — then
types the natural follow-up, "what calls it?", and gets nonsense, because
every question starts from zero. And while each question spends real model
tokens, nothing tells them what the exchange is costing.

A C++ developer who maps their repository gets almost no call edges at all:
idiomatic namespaced calls like `geo::nsq()` never resolve, so the map that
is supposed to explain their codebase draws it as a field of disconnected
boxes.

Meanwhile the people running and reviewing the tool carry three quiet
debts from V1: `docs/SECURITY.md` went false three times when the serve
surface changed and nothing but human review caught it; a slow loopback
client can park a serve thread forever, which makes one committed sentence
in that document untrue today; and the share artifact has grown 60% since
first measured, with nothing to stop it quietly becoming too big to hand to
anyone.

## Solution

Drilling into any region opens readable: the files that matter are on
screen, the rest are one gesture away, and nothing that points at a hidden
file — search, focus, the tour, the diff overlay — ever points at nothing.
Edges land spread along card edges instead of knotting at one point. When
enrichment has run, a region card says what the region *is*, in prose.

Asking becomes a conversation. Follow-ups understand "it" because each turn
carries the conversation with it; the server remembers no one; every answer
reports the tokens it actually consumed, and the reader watches the running
total instead of guessing.

C++ maps gain call edges on idiomatic code, so the visualization work pays
off there too.

And the quiet debts close: a route can no longer ship without the security
document naming it, a request read is bounded in every dimension a client
controls, and a committed test holds the share artifact under two megabytes
so growth becomes a decision someone makes.

## User Stories

**The drill view**

1. As a newcomer, I want drilling into a large region to show me the files
   that matter first, so that the densest region is still a picture I can
   read.
2. As a newcomer, I want one affordance that reveals a region's remaining
   files, so that the default view hides nothing from me permanently.
3. As a newcomer, I want search hits, focus, tour stops and the diff overlay
   to auto-reveal a file the default view hid, so that a feature that points
   at a file never points at nothing.
4. As the dashboard, I want the revealed/default state to be an input to the
   pure projection, so that the same map in the same state always draws the
   same picture.
5. As a newcomer, I want a well-connected file's edges to land at spread
   points along its card, so that it reads as a fan rather than a knot.

**The map says more**

6. As the scanner, I want to publish each file's significance in the map, so
   that the tour, the default drill view and the rankings agree by
   construction ([ADR-0010](../adr/0010-file-significance-is-published-once-in-the-map-contract.md)).
7. As a newcomer, I want a region card to describe the region in prose once
   enrichment has run, so that I learn what it is, not just where it lives.
8. As enrichment, I want region descriptions bought once and carried over by
   content hash, so that a re-scan never re-buys prose that is still valid.
9. As an external map producer, I want the new fields to be optional, so
   that every map that validated yesterday validates today.

**The conversation**

10. As a reader, I want my first question to work exactly as before, so that
    multi-turn changes nothing until I use it.
11. As a reader, I want a follow-up that says "it" to be answered from the
    nodes the conversation is already about, so that I can dig without
    re-describing my target every turn ([ADR-0012](../adr/0012-a-conversation-is-client-carried-bounded-input.md)).
12. As a reader, I want each answer to show measured token usage and a
    running conversation total, so that I know what this is spending without
    being shown an invented number.
13. As a reader on a backend that reports no usage, I want the usage display
    absent rather than fabricated, so that every number I see is real.
14. As the serving binary, I want to clamp over-bound history mechanically —
    oldest turns first — so that a client bookkeeping bug degrades an answer
    instead of erroring the reader's question.
15. As the serving binary, I want to hold no conversation state, so that the
    security posture never has to cover a session store.

**The C++ map**

16. As a C++ developer, I want namespaced calls like `geo::nsq()` to
    resolve, so that my map has call edges on idiomatic code.

**The serve surface and its documents**

17. As an auditor, I want every route the server answers named in
    `docs/SECURITY.md`, enforced by a test that derives the route set from
    the code itself, so that a new route cannot ship undocumented.
18. As the serving binary, I want reading a request bounded in total time,
    line length and line count, so that a slow or hostile loopback client
    cannot park a handler thread forever — and the availability sentence in
    `docs/SECURITY.md` becomes true.
19. As a reader's HTTP client, I want HEAD answered wherever GET is and a
    400 on a malformed request instead of a silent close, so that the server
    behaves the way HTTP promises.
20. As the serving binary, I want accept errors to back off rather than
    spin, so that file-descriptor exhaustion degrades service without
    burning a core.

**The small honesties**

21. As a newcomer, I want the header tally to stop reading "STRUCTURAL · 0
    structural", so that the grouping mode and the provenance count no
    longer share one word.
22. As a newcomer, I want the FILES tab's filter and fold state to survive
    tab switches and sidebar folds, so that the panel does not forget what I
    was doing.
23. As the repository owner, I want a committed test that fails when the
    share artifact exceeds its ceiling, so that growth is a decision, never
    an accident ([ADR-0011](../adr/0011-no-layout-library-a-share-ceiling-enforces-it.md)).

**The neighbourhood**

24. As a newcomer, I want a magnify mode that draws only the file I focused
    and the files it connects to, so that a neighbourhood on a large map is
    legible instead of lines running off the screen.
25. As a newcomer, I want leaving magnify to put me back where I was, so that
    it is a lens I look through rather than a place I have to navigate home
    from.

**The conversation, beside the map**

26. As a reader, I want the conversation in a column beside the canvas rather
    than a band above it, so that the map I am asking about stays on screen
    while I read what it answered.

## Implementation Decisions

**Contract** (ceremony per [ADR-0003](../adr/0003-rust-types-generate-the-public-map-contract.md):
schema and generated TypeScript regenerate together or the drift gate fails):

- `significance` — optional integer on file nodes: import fan-in + import
  fan-out + 1 if the file hosts an entry point, computed at scan. The
  decision, its formula and its consumers are
  [ADR-0010](../adr/0010-file-significance-is-published-once-in-the-map-contract.md).
  Tour *selection* switches to reading the stored number; tour *ordering*
  remains the tour's own.
- `description` — optional prose on a layer. The mechanical fallback is the
  text the dashboard already synthesises; structural layers only — domains
  are not contract entities and their synthesised text is already honest.

**Enrichment** gains one slot kind: a layer description, stored in the
annotation store and carried over by content hash exactly as enriched layer
names are (the [ADR-0005](../adr/0005-full-rescan-with-content-hash-enrichment-carry-over.md)
discipline; no new carry-over mechanism).

**The drill view.** The default view of a region is its top 40 files by
published significance, ties broken on path so the selection is
deterministic; one affordance reveals the rest ("show all N files"). The
revealed state is an input to the pure projection, never internal to it.
The auto-reveal set — search, focus, tour, diff overlay — is a requirement,
not a nicety: each of those features points at specific files, and pointing
at a hidden one is a silent failure. Forty is the count the V1 reference
material demonstrated readable; it is a named constant, not a promise.

**Edges.** Per-edge anchor points spread deterministically along the card
edge, computed in the pure projection, with curve styling to match. No
layout library — the decision and its enforcement (the two-megabyte share
ceiling test) are
[ADR-0011](../adr/0011-no-layout-library-a-share-ceiling-enforces-it.md).

**Magnify.** Focus dims the map in place, which stops working at the scale
this lap exists to fix: the lit edges run the width of the canvas to cards too
small to read, so the reader learns *that* a file connects to something and
never *what*. Magnify draws the induced subgraph instead — the focused file
and its direct neighbours — laid out by the drill view's own layering, so
imports still run downward. The neighbourhood is an input to the pure
projection exactly as the revealed set is, and a hidden neighbour is revealed
through auto-reveal's mechanism rather than a second one. Depth is one hop,
with no control: depth two is most of the region again on the files that need
this most, and the next hop is one more magnify. Dim-in-place focus survives
beside it — it keeps the hierarchy position magnify deliberately discards.

**The conversation.** The wire shape, bounds and retrieval rule are
[ADR-0012](../adr/0012-a-conversation-is-client-carried-bounded-input.md):
the request optionally carries up to 6 previous turns (question, answer,
citations), the server clamps rather than rejects, and the slice for each
turn is built citations-first with current-question term scoring filling the
remainder inside the existing 40-node bound. A bare question remains a valid
request. The response gains optional usage — input and output tokens —
parsed from the envelopes both provider backends already receive; a backend
that reports none produces no field. No cost-in-currency anywhere: on
subscription billing that number is notional. The dashboard renders the
conversation as a thread in the existing answer panel, shows per-turn usage
and a running total, and offers a way to start a fresh conversation; it
enforces the same turn bound the server does, so clamping is a backstop, not
the mechanism.

**The conversation column.** The answer panel V1 built was a band between
the search bar and the canvas, sized for one answer; a six-turn thread in
that band pushes the map off the screen, and the reader ends up choosing
between the conversation and the thing it describes. The thread moves to a
column docked beside the canvas: bounded width, scrolling internally, the
canvas keeping the remainder and staying interactive — so a citation click
lights a card the reader can actually see. One question or six, the
conversation lives in the same place. Decided 2026-08-14 from a reader's
walk of the shipped thread; story 26.

**The serve surface.** The module gains a route registry that the request
handler itself dispatches through, so the code and the route list are the
same thing (deferred ticket 36's second option, chosen in the interview: a
scanner over source text is a guard that cannot fail for routes spelled
unexpectedly). A committed test derives the route set from that registry and
fails when `docs/SECURITY.md` does not name every route, with the failure
naming the route and the document. Spec story 9's sentence from V1 is pinned
verbatim in both `README.md` and `docs/SECURITY.md`. Request reading gains a
deadline across the whole read plus caps on header-line length and count —
a deadline and two counters, not a state machine; the per-read timeout stays
as it is, because any number of reads that each beat it add up to no bound
at all (deferred ticket 38). HEAD is answered wherever GET is; a request
that cannot be parsed draws a 400 rather than a silent close; the accept
loop backs off on error instead of spinning. `docs/SECURITY.md`'s
limitations gain the DNS line: the netns tests prove no TCP egress, not
no-DNS-channel; the sealed tree probe is the complementary guarantee.

**C++ namespaced calls.** Namespaced symbols are stored and exported under
their qualified name — `geo::nsq` — which is the form call sites use, so
qualified calls resolve. This follows the shape ticket 37 established for Go
package-qualified calls, and it enters the parser convention checklist as
new fixture rows, not as a special case.

**The small honesties.** The header tally separates the grouping-mode label
from the provenance count so "structural" appears in at most one of them.
The FILES tab's filter and fold state are hoisted to the component that owns
the tabs, so switching tabs or folding the sidebar no longer resets them.

## Testing Decisions

Five seams, all pre-existing; V2 adds none.

1. **The map contract.** Scan a fixture tree, assert on the JSON. Home of:
   significance values (including the entry-point bonus and the
   zero-significance case), the layer-description fallback, C++
   qualified-name nodes and call edges. Prior art: the convention-checklist
   fixture table and the semantics tests. Scan is deterministic and free —
   nothing is mocked.
2. **The serve HTTP surface.** Real TCP against the real binary, scripted
   provider double behind it — never a live model, never the user's
   subscription. Home of: history clamping observable on the wire, usage
   passthrough and its absence, bare-question compatibility, the
   whole-request deadline (drive a connection that trickles header lines and
   require the server to give up), line/count caps, HEAD, 400-on-malformed,
   and the registry-versus-SECURITY.md drift test — proven able to fail by
   adding a throwaway route and watching it trip. Prior art: tickets 34/35's
   connection-driving tests and `routes.rs`.
3. **The pure projection functions.** The `toFlow()` family stays pure and
   synchronous (ADR-0011 preserves this seam deliberately). Home of: top-40
   selection and its tie-breaking, the revealed-state input, auto-reveal
   membership, anchor-point spreading as assertable geometry, and
   determinism — same map, same state, byte-identical positions. Prior art:
   the existing dashboard projection suite.
4. **Provider envelope fixtures.** Usage parsing asserted against recorded
   API and CLI response JSON behind the `EnrichmentProvider` trait; the
   citations-first slice rule unit-tested in the ask module beside the
   existing bounds, which are its prior art — the HTTP tests prove the
   plumbing, these prove the rule.
5. **The jsdom component seam.** Gesture→state only: conversation thread
   growth, the fresh-conversation reset, running usage total, FILES-tab
   state surviving tab switches and folds. Geometry stays with the
   stylesheet-contract pattern where needed. Prior art: the pass³ pattern
   from V1's walkthrough work.

A good test here asserts external behaviour — what the map says, what the
wire returns, what the projection draws — never internals. The share ceiling
rides the existing share-artifact tests. Every guard added must be proven
able to fail before its criterion is ticked; that rule has earned itself
ten times over in this repository.

## Out of Scope

- **Distribution** (binaries, release workflow) — out; its own lap after V2
  ships, per the standing polish-before-distribution decision.
- **Symbols on the canvas** — out; symbol expansion suits a 49-file
  reference repo, not 314 files, and drill, panels and search already cover
  symbols. Decided in the interview, 2026-08-13.
- **The other five parser gaps** (Go package-file anchoring, Rust
  macro-interior calls, C `static inline`, Python duplicate `def`, Markdown
  self-loop) — parked; C++ namespaced calls were judged the only one worth
  this lap.
- **Server-held conversation sessions** — rejected in
  [ADR-0012](../adr/0012-a-conversation-is-client-carried-bounded-input.md).
- **Cost-in-currency display** — rejected in ADR-0012; notional on
  subscription billing.
- **A layout library** — rejected in
  [ADR-0011](../adr/0011-no-layout-library-a-share-ceiling-enforces-it.md).
- **A magnify depth control** — out; direct neighbours only. Depth two
  reproduces the density the mode exists to escape, and magnifying a
  neighbour is the next hop. Added with stories 24–25 on 2026-08-13.
- **Semantic search / embeddings** — out; a different product decision
  entirely (carried forward from the V1 reference intake).
- **Share artifacts from the dashboard** — out; allowlist redaction in a
  second language is the copy that drifts and leaks.
- **Annotation-store reviewer machinery** — parked until a reviewer reports
  pain; provider, model and date fields already exist.
- **A thread cap for serve** — out; bounding the read stops one connection
  parking a thread forever, and a cap on concurrent connections is a larger
  change to the hand-rolled shape that deferred ticket 38 said should be
  argued on its own.
- **A Domain entity in the contract** — out; domains stay derived from
  flows, and only structural layers gain descriptions.

## Further Notes

- Source material: `docs/intake/2026-08-12-codeatlas-v1-next.md` (the
  harvested agenda), `docs/intake/2026-08-10-dashboard-ui-reference.md` (the
  reference the drill-view numbers trace to), ADRs 0010–0012, `CONTEXT.md`
  (all terms used here are its).
- The interview corrected the agenda's headline premise: the intake said "no
  layout algorithm exists", but V1's late tickets shipped a hand-rolled
  layered layout. V2's visualization work is disclosure, anchors and prose —
  not a layout engine.
- Memnoc noted at sign-off that a business deployment may reopen these
  decisions; the superseding mechanism in `docs/adr/` is the intended path.
- Open question, non-blocking: whether the top-40 constant should become
  user-adjustable if maps much larger than this repository's appear. Not
  now — a knob nobody asked for is speculative generality.
- Stories 24 and 25 were added on 2026-08-13, after the spec was approved and
  while the lap was in flight, at Memnoc's direction: the drill-view work made
  the seam cheap, and the defect was hit on the repository's own map. The
  decision to amend the spec rather than carry an orphan ticket is
  deliberate — `/harden` verifies stories, so work that is not a story is work
  nobody checks.
- Story 26 was added on 2026-08-14, the same way and for the same reason:
  Memnoc walked the shipped thread against the live map and the band layout
  failed the reader — the spec's own "in the existing answer panel" was the
  premise that aged. The column supersedes that clause; the thread's
  behaviour (stories 10, 12, 13) is unchanged.
- **Parked for `/next`, 2026-08-14, Memnoc's call:** the "open code" feature —
  opening the selected file or symbol as highlighted source, the way
  RepoAtlas does. Raised mid-lap, deliberately not squeezed in: it is a new
  class of serve surface (source-over-HTTP) that would falsify the
  never-serves-your-source sentence `docs/SECURITY.md` states and the drift
  test pins; it cannot exist in share mode under ADR-0011's two-megabyte
  ceiling; and syntax highlighting has no path under ADR-0006. If a next lap
  wants it, it starts as ADR-0013, not as a ticket.

## Verification

First harden walk, 2026-08-14, against `4c8a354` — the release binary built
that day (dashboard bundle embedded by the build script's own freshness
check), driven over real TCP with the `fake:` scripted double behind every
ask; no live model was called anywhere in the walk. Baseline at HEAD before
the walk: all three Rust configurations green (default / sealed / agent-cli,
including the 21.4 s serve suite and the fd-starvation backoff test in each),
`cargo fmt` + `clippy -D warnings` clean, dashboard 285 tests + typecheck
clean.

| story | verdict | how it was watched |
| --- | --- | --- |
| 1 top-40 opens readable | pass | projection draws the forty most significant of a wider region, path order deciding a significance-less map; forty-card default and reveal exercised in the drill suite |
| 2 one reveal affordance | pass | reveal gesture, its absence at ≤40, true-count reporting, and put-it-back all executed |
| 3 nothing points at a hidden file | pass | search, focus, tour stop and diff overlay each auto-reveal; a reader's collapse is never overridden |
| 4 revealed state is projection input | pass | revealed set drives `toFlow` as an argument; anchors identical however the map orders its edges |
| 5 edges land as a fan | pass | per-edge anchor geometry asserted in the projection suite; visible fan is a reader's-walk item below |
| 6 significance published once | pass | recomputed fan-in+fan-out+entry from the real map's own edges: 349/349 exact, 0 mismatches; the tour's 12 file stops are precisely ranks 0–11 of the stored ranking |
| 7 region card says what it is | pass | bought a description through the scripted double — `provenance: "llm"` in the served map; llm badge on that part alone executed in the render suite |
| 8 bought once, carried by hash | pass | second identical enrich run wanted one slot fewer and bought zero; the prose survived the full re-scan, which only the store can explain |
| 9 new fields optional | pass | neither field in any `required` list of the committed schema; a stripped V1-shape map served 200, dashboard loaded |
| 10 first question as before | pass | live wire: bare ask 200, canned citation dropped as the control demands |
| 11 follow-up understands "it" | pass | live wire: one carried turn citing the target put it in the slice — same question, citation survives |
| 12 measured usage shown | pass | live wire: `{"input_tokens":1207,"output_tokens":83}`, two counts and nothing else, no currency; per-turn display and running total executed |
| 13 no usage means no display | pass | live wire: reporting-nothing backend produced no `usage` key at all; dashboard absence and never-undercount executed |
| 14 clamp, never reject | pass | live wire: eight carried turns drew 200 with the newest turn still steering |
| 15 server holds nothing | pass | no cookie or session header on any response; identical repeats identical; the registry has no session route to document |
| 16 C++ namespaced calls resolve | pass | real binary on the fixture: `use_geo → geo::nsq` cross-file, `geo::twice → geo::nsq` same-file, `geo::inner::deep` walking outward, and the bare global `use_global → nsq` not suppressed |
| 17 routes named or the gate fails | pass | both drift directions green at HEAD; all four registry routes probed live and each is named in `docs/SECURITY.md` |
| 18 request read bounded | pass | watched live: 9 KB header line → 431, 70 lines → 431, and a trickling client held to exactly 20.0 s before `408` with the body naming the bound |
| 19 HEAD and 400 | pass | watched live: HEAD mirrors GET's status, headers and content-length with zero body on `/api/map`, `/` and a 404; versionless, one-token and non-UTF-8 heads each drew 400, never a silent close |
| 20 accept errors back off | pass | the fd-starvation test ran in all three configurations this walk — real child, real EMFILE, `/proc` CPU clock as witness |
| 21 tally says structural once | pass | grouping label and provenance count never share a word, across the mixes, matching the control's segment |
| 22 FILES tab remembers | pass | filter and folds survive the tab round trip and the sidebar fold, four ways |
| 23 the share ceiling is a committed test | pass | committed at HEAD, green this walk; its ability to fail is the dated red run in ticket 01; a real artifact built today from an enriched map came out 497 KB with `redacted LayerDescription.text ×1` printed by the build itself |
| 24 magnify draws the neighbourhood | pass | neighbourhood and nothing else, a hidden neighbour drawn without writing a reveal, a loner drawn alone and said so; Memnoc walked the live lens in the browser mid-lap |
| 25 leaving magnify restores | pass | back control returns exactly where the reader was; one Escape cascade, one step at a time |
| 26 the conversation beside the map | pass | column opens beside a canvas still drawn, single question gets the same column; geometry pinned by the stylesheet contract — bounded width as one named constant, internal scroll, its own track, no invented stacking context |

Between the slices, additionally watched: the purchased layer prose does
not cross the share boundary (the artifact names its own redaction); magnify
reveals through auto-reveal's mechanism and no other; the clamp is the
server's backstop behind a client that drops its own oldest turns first; and
the real repository's map serves at 0.5.0 with 349 significance values that
recompute exactly from its own edges.

**The reader's-walk layer.** Four spots where only eyes in a real browser
judge the last inch, per the pass³ pattern (gesture→state and
state→geometry are executed above; the CSS engine renders the rest):
the forty-card drill view reading as a picture (story 1), the fan visibly
spread on a well-connected card (story 5), the magnified neighbourhood
legible at real scale (story 24 — walked by Memnoc mid-lap, 2026-08-14),
and the conversation column beside a live map (story 26). None is a
correctness gap; all four are `cargo build --release`, `codeatlas serve`,
and a look.

Residuals accepted during the build are recorded in their tickets
(`.scratch/codeatlas-v2/`), none contradicting a story; the V-next
candidates among them are harvest material for `/next`.

Acceptance: **Memnoc, 2026-08-14** — walked all four reader's-walk spots
in the browser against the live server (the forty-card drill view, the
edge fan, magnify at scale, the conversation column) and accepted them.
Every story passes; **V2 is shipped.**
