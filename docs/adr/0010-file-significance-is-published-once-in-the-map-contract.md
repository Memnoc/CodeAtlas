---
status: accepted
date: 2026-08-13
proposed-by: Claude Fable 5
approved-by: Memnoc
---

# ADR-0010: File significance is published once, in the map contract

## Context

The tour's selection rule, the dashboard's default drill view (new in V2),
and the info panel's rankings all answer "which files matter", and V1
answered it in two languages — the tour in Rust, the rest in TypeScript —
agreeing by coincidence rather than by contract. V2's progressive disclosure
makes any disagreement visible: a default view that hides a file the tour
stops at feels incoherent.

## Decision

The map publishes an optional `significance` integer on every file node —
import fan-in + import fan-out + 1 if the file hosts an entry point, the
formula the tour already uses — computed at scan, and every consumer reads
the published number instead of re-deriving its own. In plain terms: the map
itself now says which files matter, so the tour, the default view, and every
panel agree by construction.

## Considered options

- **Publish the raw score** — chosen because ordering survives: top-N tour
  selection and top-K default disclosure both need it, and a consumer that
  only wants a word can band the number itself.
- **Publish a band (`simple`/`moderate`/`complex`)** — rejected: banding
  destroys the ordering that top-K selection needs, and the region
  complexity word answers a different question (how busy a region is) that
  stays where it is.
- **Keep per-consumer derivation in the dashboard** — rejected: no contract
  churn, but it is exactly the three-competing-heuristics failure this
  decision exists to prevent, and the rule stays invisible to every other
  consumer of the map.

## Consequences

The formula is public API now: changing it is a contract event, carrying
schema and generated-TypeScript regeneration under the existing drift gate
(ADR-0003). The field is optional, so every existing map still validates.
Tour selection switches to reading the stored number; tour *ordering*
remains the tour's own affair.
