# Ticket 12 — a bounded request read

**Status:** done
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 18 — reading a request is bounded in total time, line length and
line count, so a slow or hostile loopback client cannot park a handler thread
forever
**Blocks:** none
**Blocked by:** none — can start immediately

## Problem

A per-read timeout is not a bound. A client that sends one header byte every
few seconds beats the timeout on every individual read and holds the handler
thread indefinitely; a client that sends one enormous header line grows a
buffer with no ceiling; a client that sends header lines forever is never
told to stop. One committed sentence in `docs/SECURITY.md` about availability
is untrue today because of this.

This is deferred V1 ticket 38, absorbed into the V2 spec.

## What to build

A deadline across the whole read, plus a cap on header-line length and a cap
on header count. A deadline and two counters — not a state machine.

## Acceptance criteria

- [x] A deadline bounds the entire request read; a client that trickles
      header lines is dropped when it expires, however well each individual
      read behaves. `REQUEST_DEADLINE`, 20 seconds, spanning request line,
      header block and — on the one route that reads one — the body: the
      clock starts before the first byte and every socket wait is armed with
      `min(time left, READ_TIMEOUT)`, the same clamp `hang_up` already uses,
      so the give-up is tight rather than deadline-plus-one-stale-timeout.
- [x] Header line length is capped: `MAX_HEADER_LINE`, 8 KiB (the head-line
      ceiling Apache and nginx default to). Enforced one `fill_buf` at a
      time, so the line is refused *while* it arrives — `read_line` grew its
      `String` for as long as the peer withheld the newline, which was the
      defect.
- [x] Header count is capped: `MAX_HEADER_LINES`, 64 — an order of magnitude
      above what a browser sends.
- [x] The existing per-read timeout stays exactly as it is: `READ_TIMEOUT`
      is still 10 seconds, still set by `prepare` on every accepted stream
      (`accepted_connections_carry_a_read_timeout` unchanged), and still
      does its own job — a client that goes fully quiet is dropped by it at
      ten seconds, half the deadline, with the silent-drop failure it has
      always had. The deadline being twice the timeout is what keeps both
      guards employed.
- [x] Tests drive a real connection against the real binary
      (`crates/codeatlas/tests/serve.rs`):
      `a_client_that_trickles_header_lines_is_dropped_at_the_request_deadline`
      trickles one header line per 600 ms — inside every per-read timeout,
      forever — and requires the 408 in the window 19–26 s from the request
      line. Measured 2026-08-14: refused at 20.001 s against the 20-second
      deadline; the trickler's write failed at 21.008 s (the one-second
      drain behind the refusal). Kernel state counted, not green tests
      (the V1 `TCPAbortOnClose` lesson): the child's `/proc/<pid>/status`
      thread count shows a handler thread parked while the trickle runs and
      released back to baseline after the refusal; the trickler's own write
      failing is the client-side proof the socket is gone; and a fresh
      `GET /api/map` answers promptly afterwards.
- [x] Each bound proven able to fail, 2026-08-14, by raising its constant
      and watching its test fail (short client-side read timeout, so a hang
      is a failure and not a stuck suite; serve.rs restored byte-identical,
      sha256 `79670e0e…ca6acd`, after each run):
      - `REQUEST_DEADLINE` 20 s → 3600 s: the trickle test failed at 32.7 s
        with "the 408 never arrived: nothing bounds the whole request read".
      - `MAX_HEADER_LINE` 8 KiB → 1 GiB: its test failed instantly —
        "status: HTTP/1.1 200 OK" where the 431 belonged.
      - `MAX_HEADER_LINES` 64 → 1 000 000 000: its test failed instantly —
        "status: HTTP/1.1 200 OK" where the 431 belonged.
- [x] The availability sentence in `docs/SECURITY.md` is true again:
      "Half-open requests cannot park threads forever" moved back out of the
      limitations and into the loopback-DoS bullet as a claim, naming all
      three constants, the three refusals, and the three enforcing tests.
      The stale "slow header block" limitation bullet is gone; what remains
      in its place is the residual truth below.
- [x] No cap on concurrent connections — untouched, and now stated honestly
      in `docs/SECURITY.md` as its own limitation bullet: the bounds change
      each thread's tenure, not how many threads a client opening connection
      after connection can hold; the cap remains a decision to be argued on
      its own (V2 spec, Out of Scope).

## Notes

Measure what the kernel does, not only what the test asserts. V1's lesson
from `TCPAbortOnClose` was to count kernel state rather than green tests; a
"dropped" connection that leaves a thread blocked is the same defect wearing
a passing test.

Three separate bounds, three separate failures. Do not merge them into one
"request too large" condition — the operator reading a log wants to know
which one tripped.

## Decisions (2026-08-14)

- **What each failure sends.** The deadline draws a real response, `408
  Request Timeout` — HTTP's own status for "no complete request within the
  time the server was prepared to wait" — naming the twenty seconds; a
  bare close would be cheaper but the response costs one write, and
  `respond`'s hang-up already bounds a refused trickler's remaining hold
  (measured: write dead one second after the 408). Both caps draw `431
  Request Header Fields Too Large`, each with its own sentence — "a header
  line may be at most 8192 bytes" versus "a request may carry at most 64
  header lines" — because RFC 6585 defines 431 for exactly those two cases
  and asks the response to say which; the failures stay separate in the
  sentence an operator reads, and the status is HTTP's one word for header
  bounds. Not a 400: malformed is ticket 13's lane, and none of these
  requests is malformed — they are refused for their size or their pace.
- **The per-read timeout's failure is untouched.** A wait that ran out the
  full `READ_TIMEOUT` still propagates as an I/O error and a silent drop, as
  it always has; only a wait clamped *below* it by the deadline reads as the
  deadline's own refusal. A client that went quiet is not reading refusals.
- **The request line rides `MAX_HEADER_LINE`** and draws the same 431: it is
  the first line of the head, and a 414 carved out for the same bound on a
  different line would be a fourth failure for three bounds.
- **The body is inside the deadline.** "The entire request read" includes
  the one body this server reads: `Content-Length` under `MAX_BODY` says
  nothing about when the bytes come, and a body dribbled one read at a time
  beats the per-read timeout exactly as a trickled header block does —
  64 KiB at a byte per nine seconds is a week of thread life. `read_body`
  arms each read with the same clamp; expiry there answers 408 in the ask
  route's JSON error shape. Same deadline, same clock, no fourth constant.
- **The deadline is tested at its real value, not through an injection
  seam.** A `#[cfg(test)]` override cannot reach a spawned binary, an env
  knob would be new machinery on an audited surface, and the spec asked for
  a deadline and two counters. The price is wall time and it is recorded
  below: the suite went from 4.81 s to 25.79 s, all of it the trickle test
  waiting out the real 20-second constant. Accepted deliberately.

## Verification record (2026-08-14)

- `cargo test --workspace` — 270 passed, 0 failed (was 267 + 3 new),
  25.79 s wall against the 4.81 s baseline measured the same day — the
  +20.98 s is `tests/serve.rs` (2.02 s → 21.43 s) waiting out the real
  deadline, per the decision above.
- `cargo test --workspace --no-default-features` — 231 passed, 0 failed
  (was 228 + 3).
- `cargo test --workspace --no-default-features --features agent-cli` —
  258 passed, 0 failed (was 255 + 3).
- `cargo fmt --all --check` clean; `cargo clippy --all-targets -D warnings`
  clean in all three configurations.
- Mutation proofs and the measured give-up margin are dated in the criteria
  above; `docs/SECURITY.md` also gained the ticket 07 crosscheck residual
  (`a_layer_description_slot_carries_exactly_the_documented_fields` added to
  the enrichment path's Enforced-by list) while this ticket was in the
  document.
