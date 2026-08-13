# Ticket 09 — what the exchange spent, and the thread that shows it

**Status:** done
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

- [x] Both provider backends parse input and output token counts from the
      response envelopes they already receive.
- [x] The ask response carries usage as an optional field. A backend that
      reports none produces no field — never a zero, never an estimate.
- [x] No currency anywhere. The CLI envelope's cost figure is deliberately
      not surfaced: on subscription billing it is notional, and a wrong price
      is worse than no price (ADR-0012).
- [x] Recorded-envelope fixtures cover both backends, including the
      reports-nothing case.
- [x] The dashboard renders the exchange as a thread in the existing answer
      panel: previous turns stay visible and a follow-up carries them.
- [x] The client enforces the same 6-turn bound the server does, so the
      server's clamp is a backstop rather than the mechanism.
- [x] Per-turn usage is shown, plus a running conversation total; when a
      backend reports nothing, the display is absent rather than zero.
- [x] One control starts a fresh conversation, clearing the carried turns and
      the running total.
- [x] A reader who asks one question and stops sees what they see today.
- [x] jsdom gesture→state tests cover thread growth, the reset, and the
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

## What was built

**The wire half.**

- `crates/codeatlas/src/enrich/ask.rs` — `Usage { input_tokens,
  output_tokens }` (Serialize; deliberately no field a price could ride) and
  `Usage::from_envelope`, the reading rule both backends share: two measured
  counts or `None` — an absent object, a missing field, or a count that is
  not an unsigned integer all read as absence, never as zero. `Answer` gains
  `usage: Option<Usage>`, carried untouched through `verified`.
- `crates/codeatlas/src/enrich/claude.rs` — `ApiMessage` reads the Messages
  API envelope's top-level `usage` (as a raw value, so a malformed one costs
  the reader nothing but the display); `structured_output` and `complete`
  return it beside the payload; `ask` attaches it. `enrich` discards it —
  enrichment reports spend nowhere today, unchanged.
- `crates/codeatlas/src/enrich/agent_cli.rs` — `CliResult` reads the result
  envelope's `usage` the same way. `total_cost_usd`, one field over, is
  deliberately never deserialized (ADR-0012), and the doc comment on the
  field says why.
- `crates/codeatlas/src/enrich/prompt.rs` — `parse_ask_answer` sets
  `usage: None`: usage lives on the envelope, not in the structured output.
- `crates/codeatlas/src/serve.rs` — the ask response gains `"usage":
  {"input_tokens", "output_tokens"}` exactly when the backend reported one;
  no field otherwise. The inherited stale 400 hint now describes the whole
  accepted shape, optional `turns` included (ticket 08's residual, closed).
- `crates/codeatlas/src/enrich.rs` — the `fake:` backend gains reserved
  `ask:input_tokens` / `ask:output_tokens` keys; a canned file without them
  scripts the reports-nothing backend, which every pre-existing file already
  is by construction.

The response shape, when the backend measured (and no field at all when it
did not):

```json
{ "answer": "…", "citations": ["…"],
  "usage": { "input_tokens": 1207, "output_tokens": 83 } }
```

What each backend reports: the API backend reads `usage.input_tokens` /
`usage.output_tokens` from the Messages API response; the CLI backend reads
the same two counts from the result envelope and never its `total_cost_usd`;
the `fake:` double reports whatever its canned keys script, and nothing when
they are absent.

**The panel half.**

- `dashboard/src/app/ask.ts` — `Usage` (wire field names), `Answer.usage?`
  read by `askServer` under the same two-counts-or-nothing rule;
  `CompletedTurn`; `AskState` phases gain `turns` (the exchanges completed
  before the current question); `useAsk` keeps the conversation in a ref,
  sends `prior.slice(-MAX_TURNS)` so the server clamp is a backstop, appends
  the completed turn when an answer lands (behind the `latest` guard, so a
  reply landing after dismissal resurrects nothing), and `dismiss` clears the
  conversation — the fresh-conversation control, so a hidden panel never
  holds history that would silently steer the next question.
- `dashboard/src/app/AnswerPanel.tsx` — the thread: every completed turn
  renders question, answer, citations (still one control per cited node) and
  its usage line when measured; the current question renders below with the
  same asking/failed/answered states as before; the running total renders
  after the last turn, only when the thread holds more than one exchange
  (a total of one number would repeat the line above it) and only when every
  exchange was measured — a total missing a turn is an undercount presented
  as a measurement, so it is absent instead. The dismiss control is now
  labelled "Dismiss conversation" and titled for what it does.
- `dashboard/src/app/styles.css` — `.answer-turn` seams, `.answer-usage`,
  `.answer-total`.
- A reader who asks one question and stops: one question block, answer,
  citations, dismiss — plus the usage line exactly when the backend measured
  (story 12); no total, no thread chrome.

**Tests.** Seam 4 (envelope fixtures + the rule):
`envelope_usage_is_two_measured_counts_or_nothing`,
`usage_survives_citation_verification` (ask.rs);
`measured_usage_is_read_from_the_response_envelope`,
`an_envelope_without_usage_yields_an_answer_without_usage` (claude.rs);
`measured_usage_is_read_from_the_result_envelope_and_the_price_is_not`,
`an_envelope_without_usage_yields_an_answer_without_usage` (agent_cli.rs).
Seam 2 (real TCP, scripted doubles, `tests/serve.rs`):
`usage_rides_the_answer_exactly_as_the_backend_reported_it` (also pins the
usage object to exactly two keys),
`a_backend_reporting_no_usage_produces_no_usage_field`,
`the_cli_envelopes_counts_reach_the_wire_and_its_price_never_does`
(agent-cli, stand-in CLI envelope carrying both counts and a cost figure),
and `an_unusable_question_is_refused_with_a_reason` now pins the 400 hint
naming `turns`. Seam 5 (`dashboard/tests/conversation.test.tsx`, jsdom
gesture→state): thread growth and wire carriage, thread visible around an
in-flight follow-up, the client's own 6-turn bound (eight exchanges, the
eighth request carries exactly questions 2–7), per-turn usage plus the
running total, absence (no line, no total, no zero), the mixed-thread total
absenting itself, the fresh-conversation control (next request carries no
turns), the single-question view, and `askServer`'s two-counts-or-nothing
wire reading.

## Proved able to fail (recorded 2026-08-13)

The panel half was born red first: `conversation.test.tsx` ran 7 failed /
2 passed against the pre-ticket sources at 19:43 (the two passes were the
absence tests, vacuous until usage rendering existed — both covered by
mutations 11–12 below). Every guard was then broken one at a time, its test
went red with the output quoted, and the source was restored byte-identical
(diff-verified against pre-mutation snapshots).

1. API backend fed `None` instead of the envelope's usage →
   `measured_usage_is_read_from_the_response_envelope` failed: "left: None,
   right: Some(Usage { input_tokens: 1200, output_tokens: 80 })".
2. CLI backend fed `None` the same way → its unit fixture failed ("left:
   None, right: Some(Usage { input_tokens: 31, output_tokens: 9 })") and the
   wire test `the_cli_envelopes_counts_reach_the_wire_and_its_price_never_
   does` failed: "left: Null, right: Object {\"input_tokens\": Number(4213),
   \"output_tokens\": Number(57)}".
3. `Usage::from_envelope` made to fabricate zeros on anything missing →
   three tests failed at once, all "left: Some(Usage { input_tokens: 0,
   output_tokens: 0 }), right: None" (the rule test and both backends'
   absence fixtures).
4. serve.rs passthrough removed → `usage_rides_the_answer_exactly_as_the_
   backend_reported_it` failed: "left: Null, right: Object
   {\"input_tokens\": Number(1207), \"output_tokens\": Number(83)}".
5. serve.rs made to emit `{"input_tokens":0,"output_tokens":0}` when the
   backend reported none → `a_backend_reporting_no_usage_produces_no_usage_
   field` failed: "no measurement means no field — not null, not zero:
   …\"usage\":{\"input_tokens\":0,\"output_tokens\":0}".
6. A `total_cost_usd` added beside the counts on the wire → both wire usage
   tests failed on the exact-object assertion ("only them: …
   \"total_cost_usd\": Number(0.0731)").
7. `verified` made to drop usage → `usage_survives_citation_verification`
   failed: "the measured counts must survive verification: left: None".
8. The 400 hint reverted to the stale bare form →
   `an_unusable_question_is_refused_with_a_reason` failed: "the refusal must
   mention the optional turns the body accepts".
9. The client's `slice(-MAX_TURNS)` removed → `drops its own oldest turns at
   the bound` failed: "expected [ …(7) ] to have a length of 6 but got 7".
10. `dismiss` made to keep the conversation → `starts a fresh conversation
    from one control` failed: the third question carried both dismissed
    turns ("expected { question: 'third?', turns: [ …(2) ] } to deeply equal
    { question: 'third?' }").
11. The panel made to render zeros when usage is absent → `shows no usage at
    all when the backend reports none` failed: "Found multiple elements with
    the text: /tokens in/ … 0 tokens in · 0 tokens out".
12. The total made to sum only the measured turns → `absents the total
    rather than undercounting` failed: "expected document not to contain
    element, found … Conversation total: 1207 tokens in · 83 tokens out".
13. `readUsage` made to fabricate zeros for missing counts → the `askServer`
    wire-reading test failed: "reply 2 reports nothing usable and must read
    as absent: … Received: { input_tokens: 1207, output_tokens: 0 }".

Suites, measured 2026-08-13 after the mutations were restored: default
`cargo test --workspace` 252 passed / 0 failed; sealed
`--no-default-features` 214 / 0; `--no-default-features --features
agent-cli` 240 / 0; `cargo fmt --all --check` clean; `cargo clippy
--all-targets -- -D warnings` clean in all three feature configurations;
dashboard `npm test` 261 passed (19 files) and `npm run typecheck` clean.
Share ceiling: the artifact weighed 1,507,645 bytes against the 2,097,152
byte ceiling on this repository's own map (measured 2026-08-13, after the
panel work rode into the embedded dashboard).

**Residual, stated:** the running total is deliberately absent while any
turn in the thread is unmeasured — the honest reading of "measured or
absent" for a sum — and absent for a single exchange, whose own line already
is the total. Escape and the × are one control: dismissing the thread is
starting fresh, so no invisible history survives a closed panel.
