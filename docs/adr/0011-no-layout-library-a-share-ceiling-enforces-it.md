---
status: accepted
date: 2026-08-13
proposed-by: Claude Fable 5
approved-by: Memnoc
---

# ADR-0011: No layout library; a two-megabyte share ceiling enforces it

## Context

The V2 sketch assumed adopting dagre, but V1 shipped a hand-rolled layered
layout — pure, synchronous, deterministic, dependency-free — and the
remaining visual noise traces to every edge anchoring at one centre point
per card, not to layer assignment. The share artifact has meanwhile grown
from 849 KB to 1.35 MB, and nothing enforces a ceiling.

## Decision

The dashboard keeps the hand-rolled layout; edge quality is fixed inside the
pure projection — per-edge anchor spreading and curve styling — and a
committed test fails when `share.html` exceeds 2 MB. In plain terms: the map
gets cleaner lines without new third-party code, and the single-file share
export is guaranteed to stay small enough to hand to anyone.

## Considered options

- **Keep the hand-rolled layout and fix anchors in the projection** — chosen
  because the diagnosed defect is anchor convergence, which no layout engine
  fixes (React Flow draws the edges either way); it costs no dependency, and
  the `toFlow()` determinism seam and the auditable surface stay as they are.
- **`@dagrejs/dagre`** — rejected: better rank assignment than
  depth-relaxation, but it bundles into the binary and the share artifact —
  roughly a third of the remaining ceiling headroom — to solve a problem V2
  does not have.
- **`elkjs`** — rejected: asynchronous layout would force `toFlow()` async
  and reshape the entire dashboard test seam.

## Consequences

Future layout ambitions must argue against the ceiling test in the open
rather than drift in through `package.json`. The ceiling also caps
everything else the share artifact embeds — the dashboard bundle, styles,
and any future panel ships inside it.
