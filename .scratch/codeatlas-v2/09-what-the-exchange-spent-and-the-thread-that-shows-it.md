# Ticket 09 — what the exchange spent, and the thread that shows it

**Status:** ready
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 10 — the first question works exactly as before; 12 — each answer
shows measured token usage and a running conversation total; 13 — on a backend
that reports no usage, the display is absent rather than fabricated
**Blocks:** none
**Blocked by:** 08 — the thread has nothing to carry until the wire accepts it

## Problem

Two halves of one reader-facing change. The answer panel shows one question
and one answer, so ticket 08's carried history has no interface to come from;
and while each question spends real model tokens, nothing tells the reader
what the exchange is costing. Both provider envelopes already carry the token
counts and both backends currently discard them.

## What to build

Usage parsed from the envelopes both backends already receive, surfaced
through the ask response, and an answer panel that is a conversation: prior
turns visible, follow-ups carrying them, per-turn usage and a running total.

## Acceptance criteria

- [ ] Both provider backends parse input and output token counts from the
      response envelopes they already receive.
- [ ] The ask response carries usage as an optional field. A backend that
      reports none produces no field — never a zero, never an estimate.
- [ ] No currency anywhere. The CLI envelope's cost figure is deliberately
      not surfaced: on subscription billing it is notional, and a wrong price
      is worse than no price (ADR-0012).
- [ ] Recorded-envelope fixtures cover both backends, including the
      reports-nothing case.
- [ ] The dashboard renders the exchange as a thread in the existing answer
      panel: previous turns stay visible and a follow-up carries them.
- [ ] The client enforces the same 6-turn bound the server does, so the
      server's clamp is a backstop rather than the mechanism.
- [ ] Per-turn usage is shown, plus a running conversation total; when a
      backend reports nothing, the display is absent rather than zero.
- [ ] One control starts a fresh conversation, clearing the carried turns and
      the running total.
- [ ] A reader who asks one question and stops sees what they see today.
- [ ] jsdom gesture→state tests cover thread growth, the reset, and the
      running total. Wire behaviour is ticket 08's and the usage passthrough
      is proven there.

## Notes

**This is the largest slice in the set** — a merge made deliberately at
breakdown review. If it will not fit one session, the split line is the wire
half (envelope parsing and the response field) against the panel half; split
there and nowhere else, so neither half ships a number the other cannot show.

Usage is *measured or absent*. That phrasing is the glossary's, and it is the
whole point: the reader must be able to trust every number on screen as
something a provider actually reported.
