# Ticket 44 — serve says how to turn questions on

**Status:** done — 2026-08-12, `25e12a1`
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 23 — added 2026-08-12, after the work shipped
**Blocks:** none
**Blocked by:** none
**Filed:** 2026-08-12, **retrospectively** — see below

## Filed after the fact, and that is the point of it

This ticket describes a change that was already committed. It is written now
because `25e12a1` shipped a user-facing feature with **no ticket and no
story**, which is the same gap ticket 41 exists for, and I did not notice at
the time. A change nobody filed is a change `/harden` cannot walk, and a
feature that is merely unmentioned looks exactly like a feature that was
verified.

Recording it late is worse than recording it first and better than not
recording it. Story 23 was added to the spec in the same breath.

## Problem

Asked on 2026-08-12, looking at the running dashboard: *"shouldn't we hint in
the search bar subtext that you can in fact ask a question?"*

The dashboard already does — `MapExplorer.tsx` switches the placeholder to
"…or ask a question and press Enter" and renders an Ask button — but only when
`GET /api/capabilities` reports `ask: true`. Without `--ask` the whole feature
is absent: no hint, no button, no walkthrough step.

That is correct in the browser. Advertising a question this server cannot
answer would be worse than silence. But it left the feature undiscoverable:
`serve` announced the question route when it was on and said nothing when it
was off, so a reader who has only ever run plain `serve` could never learn the
flag exists.

## What was built

One line on stderr when `--ask` was not given:

```
questions are off; restart with --ask to ask this map questions in prose
```

Guarded on `enrich::recognised_specs()` rather than on a feature name. A
sealed build has no backend for `--ask` to resolve, so pointing at it would
send the reader straight into the startup refusal printed two lines above —
a new false claim of the species this project has been bitten by three times.

## Acceptance criteria

- [x] A build with a backend prints the pointer when `--ask` is absent.
- [x] A server that already answers does **not** also ask to be restarted.
- [x] A sealed build prints nothing of the kind. Unreachable from any
      `cargo test` — every one of them carries `test-provider`, so
      `recognised_specs()` is never empty there — so it is checked in
      `scripts/sealed-probe.sh` against a real `--no-default-features`
      binary, with a control check on the default binary so the sealed
      assertion cannot pass by finding nothing.
- [x] Every guard tampered and seen to fail, including rebuilding a sealed
      binary with the guard removed.

## Notes

**The banner is read by quiet, not by a line count.** The test helper counts
nothing: a line-count read *hangs* when the line under test stops being
printed, which is the regression it exists to catch. The version before that
killed the child mid-`eprintln!` and asserted on a truncated port. Both are
recorded in the helper's own doc comment.

**No confirmation prompt and no dashboard change.** The dashboard's silence is
correct and must stay; this is the terminal's job because the terminal is
where `--ask` is something a reader can act on.
