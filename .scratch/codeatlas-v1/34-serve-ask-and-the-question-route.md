# Ticket 34 — the serving binary answers questions about the map

**Status:** ready
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

- [ ] `serve --ask` enables `POST /api/ask`; without the flag the route is
      absent, so a plain `serve` stays provably egress-free and the existing
      netns test keeps a real subject.
- [ ] The server reads a request body. This is its first non-GET verb — the
      module currently rejects every method but GET and has never parsed a
      content length.
- [ ] The provider trait gains a question method with a **default
      implementation**, so no existing provider — real, fake, or failing —
      breaks by not having one.
- [ ] The model receives a **bounded slice of the map alone**, never file
      contents, selected mechanically from the question, with the bound
      stated in the code that enforces it.
- [ ] That bound holds however the question is phrased — asserted at seam 2,
      on what reaches the provider, including for a question crafted to match
      everything.
- [ ] Answers cite node IDs, and every cited ID exists in the map.
- [ ] It works on an unenriched map, answering from mechanical summaries.
      Gating on enrichment would add a way to fail for a reason the reader
      cannot see.
- [ ] A provider failure returns a clean error response and leaves the server
      running — the same degradation rule as story 14.
- [ ] In the sealed build, `--ask` explains that no provider exists rather
      than failing obscurely.
- [ ] Tested at **seam 4**: run the real binary, speak HTTP/1.1 to it over
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
