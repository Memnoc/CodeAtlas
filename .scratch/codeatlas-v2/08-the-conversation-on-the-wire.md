# Ticket 08 — the conversation on the wire

**Status:** done
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

- [x] A request may carry previous turns — question, answer, citations — and
      a bare question remains a valid request, answered exactly as today.
- [x] History beyond 6 turns is clamped oldest-first. Over-bound input is
      never rejected: the reader typed the question, the dashboard assembled
      the history, and a 400 would punish the wrong party.
- [x] Per-field character bounds clamp rather than error, alongside the
      existing question bound.
- [x] The slice is built citations-first from the carried turns, then
      current-question term scoring fills the remainder; the existing
      40-node bound is never exceeded.
- [x] Only citations naming real nodes survive — the existing validation
      covers carried ones, so a client cannot smuggle a node ID into the
      slice by inventing it.
- [x] The ask path retains nothing between requests: no session, no cache, no
      per-client identity. Two conversations interleaved on one server never
      see each other.
- [x] Wire tests drive real TCP against the real binary with a scripted
      provider double behind it — never a live model, never the user's
      subscription.
- [x] The slice rule is unit-tested in the ask module beside the existing
      bounds; the wire tests prove the plumbing, the unit tests prove the
      rule.
- [x] Each clamp is proven able to fail before its criterion is ticked.

## Notes

The `Content-Type: application/json` gate on the ask route is a deliberate
guard against a cross-origin form post. Growing the body's shape must not
weaken it.

Citations-first was chosen over folding previous questions' terms into
scoring precisely because citations are already-validated node IDs
(ADR-0012). If the implementation drifts toward term overlap for continuity,
it has drifted out of the decision.

## What was built

- `crates/codeatlas/src/enrich/ask.rs` — `Turn` (question, answer,
  citations; deserialized straight off the wire), `MAX_TURNS = 6`,
  `MAX_TURN_ANSWER_CHARS = 2000`, `clamped_turns` (newest 6 kept, carried
  question clamped to `MAX_QUESTION_CHARS`, carried answer to
  `MAX_TURN_ANSWER_CHARS`), and `select_context` grown citations-first:
  carried citations fill seats inside `CONTEXT_NODES` (newest turn first,
  repeats once, IDs naming no node select nothing), term scoring fills the
  remainder exactly as a bare question is served.
- `crates/codeatlas/src/serve.rs` — `AskBody` gains `turns`
  (`#[serde(default)]`), handed to `ask::build`. Nothing else on the route
  changed: malformed JSON, blank/over-long question, `Content-Type` gate,
  `MAX_BODY` all keep their existing refusals.
- `crates/codeatlas/src/enrich/prompt.rs` — `ask_user_message` renders the
  clamped turns as a Q/A transcript, oldest first, between the project and
  the question; a bare question's message is byte-identical to before
  (the existing whole-message test holds it there).
- `dashboard/src/app/ask.ts` — `Turn` type, `MAX_TURNS = 6` (pinned to the
  Rust constant by `tests/routes.rs`), `askServer(question, turns = [])`;
  without turns the body is byte-identical to the old one. No UI — that is
  ticket 09's.
- `docs/SECURITY.md` — the question-path section now names the carried
  conversation and states five bounds instead of three, with the enforcing
  tests listed.

The wire shape:

```json
{ "question": "what calls it?",
  "turns": [ { "question": "where does the program start?",
               "answer": "In src/main.ts.",
               "citations": ["file:src/main.ts"] } ] }
```

**`MAX_TURN_ANSWER_CHARS = 2000`**, chosen 2026-08-13: the ask system
prompt demands "a short paragraph", so 2000 characters holds any answer
this route produces with room to spare, while capping what six carried
answers add to the prompt at 12,000 characters — smaller than the slice
itself (40 nodes × 400-character summaries). Clamped rather than refused,
like a summary and unlike the current question: the reader can rephrase a
question, and can do nothing about what an earlier answer said.

**Tests added.** Seam 4 (the rule, `src/enrich/ask.rs` +
`src/enrich/prompt.rs`):
`a_carried_citation_selects_a_node_the_question_alone_never_would`,
`the_newest_turns_citations_lead_and_a_repeat_enters_once`,
`citations_alone_cannot_widen_the_slice_past_the_bound`,
`an_invented_carried_citation_selects_nothing`,
`history_beyond_the_turn_bound_is_dropped_oldest_first`,
`carried_fields_are_clamped_rather_than_refused`,
`carried_turns_ride_between_the_project_and_the_question_oldest_first`.
Seam 2 (the plumbing, `tests/serve.rs`, real TCP against the real binary,
`fake:`/stand-in-CLI doubles only):
`a_carried_turn_steers_the_slice_the_next_answer_is_drawn_from` (with the
bare-question control in the same test),
`history_beyond_the_bound_is_clamped_oldest_first_never_rejected`,
`two_conversations_interleaved_on_one_server_never_see_each_other`,
`a_request_carrying_turns_still_faces_the_content_type_gate`,
`carried_turns_reach_the_model_clamped_and_in_order` (agent-cli, on the
recorded argv). Constants: `the_dashboard_carries_the_turn_bound_this_
binary_clamps_to` (`tests/routes.rs`, born red 2026-08-13 before ask.ts
declared the constant). Dashboard (`tests/ask.test.tsx`): `carries previous
turns on the wire, and no turns field without them`; the existing exact-body
test pins the bare case.

The conversation-state observable on the wire is the citation validator
itself: the canned answer cites `file:src/zzz/target.ts` in a 61-file repo
where that node is outside every bare top-40, so the citation survives
`verified` exactly when carried turns put the node in the slice.

## Proved able to fail (recorded 2026-08-13)

Each guard was broken one at a time, its test went red with the output
quoted, and the source was restored byte-identical (diff-verified against
pre-mutation snapshots). The main cycles were also born red in TDD order:
citations-first (unit, then wire), both build clamps, the prompt transcript.

- Turn clamp kept the oldest six instead of the newest →
  `history_beyond_the_turn_bound_is_dropped_oldest_first` failed: "the
  oldest turn is the one dropped: left: Some(\"turn 0?\") right:
  Some(\"turn 1?\")".
- Turn clamp removed → the same test failed ("left: 7, right: 6") and the
  wire test `history_beyond_the_bound_is_clamped_oldest_first_never_rejected`
  failed: "the oldest turn must be the one dropped: … citations
  [\"file:src/zzz/target.ts\"], expected []".
- Carried-question clamp removed →
  `carried_fields_are_clamped_rather_than_refused` failed: "clamped to the
  bound plus the ellipsis: left: 1050, right: 1001".
- Carried-answer clamp removed → the same test failed ("left: 2050, right:
  2001") and `carried_turns_reach_the_model_clamped_and_in_order` failed on
  the child's argv: "the over-long answer must arrive clamped".
- Citation loop skipped →
  `a_carried_citation_selects_a_node_the_question_alone_never_would` failed
  (a fallback file led the slice instead of the cited function) and the wire
  test `a_carried_turn_steers_the_slice…` failed: "the carried citation must
  steer the slice: citations []".
- Citation order inverted (oldest turn first) →
  `the_newest_turns_citations_lead_and_a_repeat_enters_once` failed (run3
  led where run7 must) and `citations_alone_cannot_widen_the_slice_past_the_
  bound` failed: "a newest-turn citation was cut:
  file:src/module40/widget40.ts".
- Bound check removed from the citation loop → `citations_alone_cannot_
  widen_the_slice_past_the_bound` failed ("attempt to subtract with
  overflow" at the fill's truncate — the fill cannot even be computed once
  citations overrun the bound).
- Invented citations admitted as placeholder nodes →
  `an_invented_carried_citation_selects_nothing` failed: "an invented node
  ID must never enter the slice".
- The server given a session (a `static Mutex` accumulating turns across
  requests) → `two_conversations_interleaved_on_one_server_never_see_each_
  other` failed on round 1: "a bare conversation asked between two carried
  ones must inherit nothing from them: citations
  [\"file:src/zzz/target.ts\"]".
- Transcript dropped from the prompt → `carried_turns_ride_between_the_
  project_and_the_question_oldest_first` failed whole-message, and
  `carried_turns_reach_the_model_clamped_and_in_order` failed: "turn 1 must
  survive the clamp" (the recorded argv carried no turn at all).
- `askServer` dropped turns from the body → the dashboard test failed:
  "expected { question: 'what calls it?' } to deeply equal { question:
  'what calls it?', …(1) }".

Suites, measured 2026-08-13 after the mutations were restored: default
`cargo test --workspace` 243 passed / 0 failed; sealed
`--no-default-features` 210 / 0; `--no-default-features --features
agent-cli` 233 / 0; `cargo fmt --all --check` clean;
`cargo clippy --all-targets -- -D warnings` clean in all three feature
configurations; dashboard `npm test` 252 passed (18 files) and
`npm run typecheck` clean.
