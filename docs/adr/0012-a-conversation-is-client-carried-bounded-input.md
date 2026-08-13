---
status: accepted
date: 2026-08-13
proposed-by: Claude Fable 5
approved-by: Memnoc
---

# ADR-0012: A conversation is client-carried, bounded input

## Context

ADR-0009 answers one question from a bounded, mechanically selected slice of
the map; V2 adds multi-turn conversation, which needs somewhere for state to
live and a way for a follow-up like "what calls it?" — which carries no
searchable terms — to retrieve the right nodes.

## Decision

The dashboard carries the conversation: a request may include the previous
turns (question, answer, citations), and the server stays stateless —
clamping history to the last 6 turns with stated per-field character bounds,
and building each slice from the carried citations first, current-question
term scoring filling the remainder within the same 40-node bound. Each
answer returns measured token usage when the backend reports it. In plain
terms: the conversation lives in the reader's browser tab, the server never
remembers anyone, and every bound ADR-0009 promised still holds.

## Considered options

- **Client-carried history, server clamps** — chosen because history is just
  more input, bounded where every other bound lives; there is no session
  lifecycle, eviction, or cross-tab identity for the security posture to
  cover, and a re-scan keeps being answered against the newest map on every
  request.
- **Server-held sessions** — rejected: a second kind of state in the one
  binary the security review covers, buying no capability the client cannot
  carry.
- **Folding previous questions' terms into slice scoring** — rejected in
  favour of citations-first: citations are already-validated node IDs, so
  continuity comes from the nodes the conversation is provably about rather
  than from fuzzy term overlap.
- **Surfacing the CLI's `total_cost_usd`** — rejected: on subscription
  billing that number is notional, and a wrong price is worse than no price.
  Usage is tokens, measured or absent — never estimated.

## Consequences

`AskBody` grows compatibly: history is optional, and a bare question stays a
valid request. Over-bound history is clamped oldest-first rather than
rejected — the reader types the question but the dashboard assembles the
history, and a 400 would punish the wrong party. Both provider backends gain
usage parsing from response envelopes they already receive and currently
discard.
