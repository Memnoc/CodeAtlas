# Ticket 08 — the conversation on the wire

**Status:** ready
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 11 — a follow-up that says "it" is answered from the nodes the
conversation is already about; 14 — over-bound history is clamped
mechanically rather than rejected; 15 — the serving binary holds no
conversation state
**Blocks:** 09
**Blocked by:** none — can start immediately

## Problem

Every question starts from zero. A reader asks a good question, gets a good
answer, and then types the natural follow-up — "what calls it?" — which
carries no searchable terms at all, so the mechanical slice selects on the
words "what", "calls" and "it" and the answer is nonsense.

[ADR-0012](../../docs/adr/0012-a-conversation-is-client-carried-bounded-input.md)
decided the shape: history is client-carried bounded input, and continuity
comes from the citations the conversation has already earned.

## What to build

The ask route accepts previous turns alongside a question, clamps them, and
builds each slice citations-first — while the server keeps remembering no one.

## Acceptance criteria

- [ ] A request may carry previous turns — question, answer, citations — and
      a bare question remains a valid request, answered exactly as today.
- [ ] History beyond 6 turns is clamped oldest-first. Over-bound input is
      never rejected: the reader typed the question, the dashboard assembled
      the history, and a 400 would punish the wrong party.
- [ ] Per-field character bounds clamp rather than error, alongside the
      existing question bound.
- [ ] The slice is built citations-first from the carried turns, then
      current-question term scoring fills the remainder; the existing
      40-node bound is never exceeded.
- [ ] Only citations naming real nodes survive — the existing validation
      covers carried ones, so a client cannot smuggle a node ID into the
      slice by inventing it.
- [ ] The ask path retains nothing between requests: no session, no cache, no
      per-client identity. Two conversations interleaved on one server never
      see each other.
- [ ] Wire tests drive real TCP against the real binary with a scripted
      provider double behind it — never a live model, never the user's
      subscription.
- [ ] The slice rule is unit-tested in the ask module beside the existing
      bounds; the wire tests prove the plumbing, the unit tests prove the
      rule.
- [ ] Each clamp is proven able to fail before its criterion is ticked.

## Notes

The `Content-Type: application/json` gate on the ask route is a deliberate
guard against a cross-origin form post. Growing the body's shape must not
weaken it.

Citations-first was chosen over folding previous questions' terms into
scoring precisely because citations are already-validated node IDs
(ADR-0012). If the implementation drifts toward term overlap for continuity,
it has drifted out of the decision.
