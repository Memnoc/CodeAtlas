---
status: accepted
date: 2026-08-11
proposed-by: Claude Opus 5
approved-by: Memnoc
---

# ADR-0009: Codebase questions are answered by the serving binary, not the dashboard

## Context

A reader who does not know a codebase does not know what to search *for*,
which is exactly when the dashboard's name-matching search is least useful,
and an enriched map already holds both the structure and the prose an answer
would need. But the dashboard is the one component the security posture
promises makes no external requests (spec story 17), and the share artifact
inherits its renderer.

## Decision

Free-text questions about a map are answered by the serving binary over a new
`POST /api/ask` route, enabled by an explicit `serve --ask` opt-in, from a
bounded slice of the map alone; the dashboard only ever makes the same-origin
request it already makes for `/api/map`. In plain terms: the reader types a
question into the dashboard, but the program that talks to the model is the
one on their own machine, which is the program the security review already
covers.

## Considered options

- **The serving binary calls the model** — chosen because egress stays inside
  the binary where ADR-0006's feature gates and egress suite already live, and
  because it leaves spec story 17 true word for word: the dashboard's requests
  remain same-origin, which is what "zero external requests" has always meant
  there.
- **The dashboard calls the model directly** — rejected; it would put egress
  in the one component with no compile-time gate over it, and the share
  artifact renders from the same code, so a redacted file handed to an
  outsider would gain a network path.
- **Answers that may read file contents** — rejected for V1. Answering from
  the map alone keeps ADR-0004's bound exactly as it stands and makes this
  feature's privacy posture identical to enrichment's. Thin answers are
  evidence for V2, not an assumption to make now.
- **Available in the share artifact** — rejected; it has no server, and giving
  a double-clickable `file://` page a network path would change what "share"
  means far more than the feature is worth.

## Consequences

Questions reach the model through the same provider trait as enrichment
(ADR-0004, ADR-0008), so both the API and CLI credential paths work here
without a second integration — which is the reason to settle this alongside
ADR-0008 rather than after it.

`serve` gains its first non-GET verb; its module comment currently reads
"exactly one verb (GET), three routes". Without `--ask` it stays provably
egress-free, so the netns test
`serve_binds_loopback_and_answers_with_no_network_beyond_loopback` keeps a
real subject.

The feature works on an unenriched map too, answering from mechanical
summaries. Gating it on enrichment would add a second way to fail for a reason
the reader cannot see.

What the model receives must be a bounded slice with a stated bound, selected
mechanically from the question, and answers cite node IDs so a reader can
check them. ADR-0004's promise is that the model never receives the whole
serialized graph, and an unbounded question with an unbounded retrieval step
is the obvious way to lose it by accident.
