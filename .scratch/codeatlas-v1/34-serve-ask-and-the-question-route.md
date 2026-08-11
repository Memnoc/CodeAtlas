# Ticket 34 — the serving binary answers questions about the map

**Status:** done
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 21 — ask the map a question in my own words and be shown which
nodes answer it (the binary half; ticket 27 is the dashboard half)
**Blocks:** 27, 32
**Blocked by:** none

## Problem

A reader who does not know a codebase does not know what to search *for*,
which is exactly when name-matching search is least useful. The map already
holds the structure and, once enriched, the prose an answer would need.

[ADR-0009](../../docs/adr/0009-codebase-questions-are-answered-by-the-serving-binary.md)
settles who does the asking: the serving binary, not the dashboard. Egress
stays inside the binary where ADR-0006's feature gates and egress suite
already live — and because the dashboard's request is same-origin, spec story
17 needs no rewrite at all.

## What to build

`codeatlas serve --ask` exposes `POST /api/ask`. A question in, an answer out,
citing the node IDs it was drawn from. Demoable with `curl` alone — no
dashboard involved.

## Acceptance criteria

- [x] `serve --ask` enables `POST /api/ask`; without the flag the route is
      absent, so a plain `serve` stays provably egress-free and the existing
      netns test keeps a real subject.
- [x] The server reads a request body. This is its first non-GET verb — the
      module currently rejects every method but GET and has never parsed a
      content length.
- [x] The provider trait gains a question method with a **default
      implementation**, so no existing provider — real, fake, or failing —
      breaks by not having one.
- [x] The model receives a **bounded slice of the map alone**, never file
      contents, selected mechanically from the question, with the bound
      stated in the code that enforces it.
- [x] That bound holds however the question is phrased — asserted at seam 2,
      on what reaches the provider, including for a question crafted to match
      everything.
- [x] Answers cite node IDs, and every cited ID exists in the map.
- [x] It works on an unenriched map, answering from mechanical summaries.
      Gating on enrichment would add a way to fail for a reason the reader
      cannot see.
- [x] A provider failure returns a clean error response and leaves the server
      running — the same degradation rule as story 14.
- [x] In the sealed build, `--ask` explains that no provider exists rather
      than failing obscurely.
- [x] Tested at **seam 4**: run the real binary, speak HTTP/1.1 to it over
      127.0.0.1, assert on the response — the shape the serve suite already
      uses.

## Notes

**The bound is the part worth being careful about.** ADR-0004's standing
promise is that the model never receives the whole serialized graph, and an
unbounded question feeding an unbounded retrieval step is the obvious way to
lose that by accident. The spec deliberately leaves the number and the ranking
rule open, to be settled by measurement in the same spirit as the enrichment
batch size — but *stated*, and enforced somewhere a test can point at.

Questions go through the same provider trait as enrichment, so both credential
paths work here with no second integration. That is the reason ADR-0008 and
ADR-0009 were decided in one session.

`serve` is deliberately a hand-rolled server rather than a framework — "one
verb, three routes". Adding POST is a real change to that premise and the
module comment needs to stop saying one verb. Keep the hand-rolled shape; the
alternative is pulling in a server crate for one route, which would widen the
dependency audit surface ADR-0006 exists to keep narrow.

## What the work found

**The bound is two bounds, and the second one was not in the ticket.** The
context slice is the obvious one — `ask::CONTEXT_NODES`, enforced by an
unconditional `truncate` in `select_context`, so a question engineered to
match the whole repository produces exactly as much context as one that
matches nothing. The other is the question itself: a reader's prose is
unbounded input that reaches a prompt verbatim, so `MAX_QUESTION_CHARS`
refuses an over-long one rather than truncating it. A truncated question is a
different question, answered without saying so.

Selection is mechanical: distinct terms of three characters or more, minus a
short list of English function words, scored by substring against a node's
name (×3), path (×2) and summary (×1), then ordered by score, node kind, and
ID. The kind rank is what makes a question that matches nothing fall back to
the map's file skeleton rather than an arbitrary handful of symbols.

**Statuses distinguish who can fix the problem.** A blank, over-long or
unparseable question is a 400; a backend that would not answer is a 502.
That split is why `ask::build` and `ask::answer` are two functions rather
than one `run` — the route reads the distinction off the call site instead of
matching on an error string.

**Both backends were refactored rather than copied.** Enrichment and
questions differ in three values — schema, system prompt, message — and in
nothing about transport, so `claude::structured_output` and
`agent_cli::structured_output` now return the payload and each caller pairs it
with its own parser. In the CLI backend that mattered for a specific reason:
a second copy of the argv construction would be a second place for ticket
31's swallowed-prompt bug to return, and it would return silently.

### Five mutations survived the first pass

Twenty-nine mutations were run. The five that survived were all the project's
recurring failure mode — a criterion ticked against an assertion that could
not fail:

- **Stopword removal.** `function_words_do_not_rank_nodes` asserted a
  stopword question scores zero, against a fixture whose terse summaries
  contained none of the words in question. It passed with the filter deleted.
  It now runs against prose that genuinely contains all of them, and asserts
  the premise before the conclusion.
- **The kind tiebreak.** The fallback test asserted the slice was all files —
  true either way, because node IDs are kind-prefixed and `file:` sorts
  before `function:`. The fixture gained class nodes, since `class:` sorts
  *before* `file:` and the alphabet therefore stops rescuing the test.
- **`MIN_TERM_CHARS` and the name/path/summary weights** were untested
  outright. Both are load-bearing: `in` is inside `thing`, `defines`, and
  half the paths in a repository, and a file named for the term should beat
  one that mentions it. The weights test puts its decoy *earlier* in ID order
  on purpose, so a tie cannot hand it the right answer for the wrong reason.
- **Which schema `CliProvider::ask` passes.** The lockdown test asserted the
  answer schema — on a *test helper* that constructed the argv itself, not on
  the provider. Exactly the trap ticket 31 recorded, reproduced one ticket
  later. It is now asserted on the argv a real spawned child received,
  driven all the way from an HTTP request through seam 4 into seam 3.

### Two things fixed on the way past

**Four error messages ended with "the structural map was written without
enrichment".** True for `scan --enrich`, false for `serve --ask`, which has
written nothing. The clause was also already redundant — `lib.rs` appends
"(the structural map is intact)" at the scan call site. Removed from the
resolver and from the CLI backend; the caller that has written a map is the
caller that says so.

**The fake-executable recorder truncated every multi-line argument.** It
recorded `arg=<first line>` and left the rest untagged, so every assertion
about a prompt's body was reading only its first line — including the ones
ticket 31 added. It now folds newlines to carriage returns before recording,
which is what let the question's text be asserted at all.

**Out of scope, folded in deliberately:** `lib.rs` printed `mapped N files`
from `nodes.len()`, roughly four times the truth. The spec had carried it
through five harden walks as "the only place the CLI states a number to the
user that is not true, and a one-word fix". This ticket was the next work to
touch that function, so it is fixed here with a test that fails if it reverts,
and the spec's limitations entry is struck through with a pointer here.

### The ranking rule was settled by measurement, and the first attempt was wrong

The ticket left the number and the ranking rule "to be settled by
measurement". Measuring meant scanning this repository (47 files, 488 nodes),
serving it with `--ask` behind a stand-in executable, and reading the prompt
the child actually received. Two things came out of that:

**On an unenriched map, only names and paths carry vocabulary.** A mechanical
summary is `"Rust file, 378 lines: 7 functions, 3 classes"`. The summary
weight is therefore dead until enrichment has run, which is worth knowing
when judging answer quality — and is an argument for ticket 30's committed
store rather than a defect here.

**Summing the per-field weights double-counted names.** A file's name is
always inside its path, and a symbol's mechanical summary is literally
`"Function <name>, lines a-b"`, so the three fields are not independent
evidence. "How does the sealed build stop network egress?" returned six
`build_*` functions ahead of the file whose enriched summary discussed sealed
builds, egress *and* the network feature — one term scoring 4 against three
terms scoring 3. A term now scores where it appears most strongly, once. The
same question then ranks that file first, and "where is the dashboard search
overlay handled?" returns `overlay.ts` and `searchNodes`.

## What `/crosscheck` found

**A third bound was missing.** `CONTEXT_NODES` bounds the slice in *nodes*,
not in bytes, and every summary CodeAtlas writes is a sentence — but a map
from another producer (story 16) may carry any string the schema allows. The
module claimed "the prompt stays a few KB"; that is now true, enforced by
`MAX_SUMMARY_CHARS`.

**`POST /api/ask` had no defence against another origin.** While
`serve --ask` runs, any page open in the reader's browser could post to
127.0.0.1 and spend their model budget — it could not read the reply, but the
spend is the damage. Requiring `application/json` fixes it: a cross-origin
`fetch` or form post can only set the three "simple" content types without a
CORS preflight, and this server answers no `OPTIONS`. Same-origin callers
(ticket 27) are unaffected and `curl` sets the header anyway.

**The body cap changed behaviour on routes that never wanted a body.** It
sat before routing, so a plain `serve` — no `--ask`, sealed build — answered
413 to a GET with a large `Content-Length`. Reading a body is now confined to
the question route, so everything else behaves exactly as it did before
ADR-0009 rather than merely similarly.

**`"only GET is served"` is false with `--ask`.** Now build-aware, and it
names the route that does exist, which is what a reader who mistyped it
needs.

**`answer_schema`/`parse_answer` sat one character from
`answers_schema`/`parse_answers`.** That naming *is* the hazard the survived
mutation above came from, so they are `ask_answer_schema`/`parse_ask_answer`,
and the trio `(schema, system_prompt, user_message)` that both backends
passed around became `prompt::Completion` with two constructors. Each backend
now has exactly one `complete`, and the API backend's duplicated body builder
is gone — it had been left half-refactored while the CLI backend was done.

**`--model` and `--provider` were declared twice** in `lib.rs`, once per
subcommand, with help text built from the compiled-in backends — two places
for it to drift. They are one flattened `BackendArgs`. Both flags depend on a
different enabling flag (`--enrich`, `--ask`), which is why the two carry the
same clap **id** `backend`: one `requires` then resolves correctly in either
subcommand, and each error still names the right flag.

### Deferred, and genuinely owned

`docs/SECURITY.md` is now false in a default build and says nothing about the
question path; `README.md` has the same gap; CI has no
`agent-cli`-without-`network` job; and the netns suite does not yet pin
`serve --ask` as egress-capable. All four are ticket 32's acceptance
criteria, which is why it is blocked on this ticket. A criterion was added
there for the question path's payload specifically, because
`enrich::ask::NodeContext` now carries a comment pointing at a paragraph that
does not exist yet.
