# Ticket 27 — ask the codebase a question from the search bar

**Status:** blocked
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 21 — ask the map a question in my own words and be shown which
nodes answer it (**the dashboard half**; ticket 34 is the binary half)
**Blocks:** none
**Blocked by:** 34 — `serve --ask` and `POST /api/ask`
**Scope:** V1 — decided 2026-08-11, against a recommendation to defer it

## Problem

Requested 2026-08-11: when a map is enriched, let the search bar take a
*question* rather than a name — "how does this function do X", "where does Y
business logic happen" — and answer it from the map.

The appeal is obvious: the map already holds the structure and, once
enriched, the prose. A reader who does not know the codebase does not know
what to search *for*, which is precisely when a name-matching search is least
useful.

## Scope: the dashboard half only

**Split 2026-08-11 during `/to-tickets`.** As originally filed this ticket
covered the route, the flag, the retrieval, the bounding, the UI, share-mode
absence and the sealed refusal — more than one session, and split at the
boundary the spec's own seams already draw. Ticket 34 delivers everything
below HTTP and is verifiable with `curl` alone; this ticket is the interface
on top of it.

## What to build

The search bar takes a question as well as a name. The answer appears with the
nodes it cites, and clicking one selects it on the canvas — so an answer is a
way into the map rather than a wall of prose beside it.

## Acceptance criteria

- [ ] The search bar accepts a question and shows the answer, without losing
      the name-matching search it already does — a reader typing a filename
      must still get the filename.
- [ ] Cited nodes are rendered as controls that select the node on the canvas,
      so the answer is navigable.
- [ ] A citation naming a node absent from the map degrades visibly rather
      than rendering a control that does nothing.
- [ ] While an answer is in flight the UI says so, and a failed request says
      what failed without discarding the question the reader typed.
- [ ] The feature is **absent in a share artifact**, which has no server —
      driven through `<App/>` with an embedded payload, the way ticket 28's
      share-mode assertions are.
- [ ] Absent when the served map's binary was started without `--ask`, without
      the dashboard needing to be rebuilt to notice.
- [ ] `Escape` closes the answer through the explorer's one cascade, not a
      second handler — see ticket 22, where a split Escape left a dead zone.
- [ ] Driven by real user events in the dashboard suite, with the route
      stubbed at the fetch boundary; no test performs real network I/O.

## Notes

**The Escape criterion is not boilerplate.** `MapExplorer` owns one
document-level cascade — search overlay, share/export menu, path panel, one
step back — because ticket 22 shipped a version where Escape lived in two
places and tabbing once put focus where neither listened. A third handler in a
third component is how that hole reopens. Add a layer to the cascade.

**How the dashboard learns whether `--ask` is on** is unsettled and worth
deciding early rather than discovering: the map payload does not carry server
capabilities today, and probing the route to find out is a request made to
answer a question nobody asked. Ticket 34 may be the better place to expose
it.

## How it was unblocked

**All five questions below were settled on 2026-08-11 by
[ADR-0009](../../docs/adr/0009-codebase-questions-are-answered-by-the-serving-binary.md),
and story 21 now exists for `/harden` to walk.** The answers, in the order the
questions are asked:

- **The serving binary calls the model**, over a new `POST /api/ask` route
  behind an explicit `serve --ask` — so egress stays where ADR-0006's
  machinery already lives.
- **Story 17 needed no rewrite at all.** The dashboard's request is
  same-origin to 127.0.0.1, the same category as the `/api/map` request it
  already makes; "zero external requests" has always meant off-origin.
- **The sealed build refuses**, as it refuses every provider; without `--ask`
  plain `serve` stays provably egress-free, so the existing netns test keeps a
  real subject.
- **The share artifact does not have the feature.** It has no server, and
  giving a double-clickable `file://` page a network path would change what
  "share" means more than the feature is worth.
- **The model sees a bounded slice of the map alone** — never file contents —
  selected mechanically, with the bound stated; answers cite node IDs so a
  reader can check them.

Questions travel through the same provider trait as enrichment, so both
credential paths (API key, and the `cli:claude` provider of ADR-0008) work
here without a second integration. The section below is kept as the record of
why this was filed blocked.

### Why this was blocked rather than ready

It is not a dashboard feature with an LLM bolted on; it changes what the
product *is*, in three ways that the existing ADRs answer differently:

1. **The dashboard would have to reach the network.** Today it consumes only
   local files and makes zero external requests — that is story 17, and
   ADR-0006 makes it a compile-time guarantee with an egress suite behind it.
   A question box that calls an API breaks that guarantee for the dashboard,
   and the share artifact inherits the same renderer.
2. **Enrichment today is a batch that fills typed slots** (ADR-0004): bounded
   prompts, structured output, no free text, no output-repair machinery. A
   question is unbounded input and its answer is prose. That is a second,
   differently-shaped path to the model.
3. **It needs credentials at view time, not scan time**, which is the exact
   friction the enrichment-credential work exists to remove. A feature that
   reintroduces per-reader credentials wants deciding alongside that, not
   after it.

None of these is a reason not to build it. They are reasons the decision
belongs in an `/adr` rather than in a ticket.

### Questions the ADR had to settle

- Does the *dashboard* call the model, or does the CLI serving it do so —
  keeping egress in the binary where ADR-0006's guarantee already lives?
- What does the sealed build do here? Refuse, presumably — and the egress
  suite must prove it.
- What does the share artifact do? It is a single file with no server; either
  the feature is absent there or the artifact gains a network path, which
  would be a much larger change to what "share" means.
- What does the model see? Bounded slots are what keeps ADR-0004's promise
  that the model never receives the whole serialized graph. A free question
  needs a retrieval step with a stated bound.
- Is it answerable from the map alone, or does it need file contents? Those
  are very different privacy postures.

### What that prediction got wrong

The note filed with this ticket said: *"Story 17 currently says the dashboard
makes zero external requests; this feature contradicts that story as written,
so the story changes or a new one carves out an exception."*

It does not, and the reasoning was worth catching. The dashboard already
fetches `/api/map` from the local server, so "zero external requests" has
always meant *off-origin* — a same-origin POST to 127.0.0.1 is the same
category as what it does today. Once ADR-0009 put the model call in the
serving binary, story 17 needed no amendment at all. The assumption that a
question box must mean dashboard egress is what made this look like a larger
decision than it was.
