# Ticket 35 — a refusal the client sometimes never receives

**Status:** ready
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 21 — the question route (the refusal half), under story 14's rule
that a path which cannot serve a request must still say so
**Blocks:** none
**Blocked by:** none
**Filed:** 2026-08-11, from a flake surfaced during ticket 32's review

## Problem

`the_question_route_does_not_exist_without_the_flag` fails roughly once in
twenty-five runs of the `serve` suite:

```
thread 'the_question_route_does_not_exist_without_the_flag' panicked at
crates/codeatlas/tests/serve.rs:173:39:
called `Result::unwrap()` on an `Err` value:
Os { code: 104, kind: ConnectionReset, message: "Connection reset by peer" }
```

Measured at 2 failures in 40 filtered runs and 1 in 25 full-suite runs. It is
not the test's fault, and fixing the test would hide the defect.

**What is actually happening.** Ticket 34 confined request-body reading to the
question route, deliberately — so that every other route behaves exactly as it
did before ADR-0009 rather than merely similarly. The consequence is that the
405 branch writes its refusal and closes the socket without draining the body
the client already sent. Closing a TCP socket with unread data in the receive
queue sends an RST rather than a FIN, and an RST discards the send buffer at
the peer. The client then reads a reset instead of the response that was
genuinely written to the wire a moment earlier.

So this is a product defect, not a test defect: a reader who POSTs to a server
started without `--ask` sometimes gets a connection reset instead of
`only GET is served`. The message ticket 34's crosscheck made build-aware —
precisely so a reader who mistyped a route learns which routes exist — is the
message most likely to be lost.

The same mechanism was met during ticket 34 and misdiagnosed as a
large-body-only effect. The test
`a_plain_serve_still_ignores_a_body_it_was_never_going_to_read` was written
around it by declaring a large `Content-Length` and sending two bytes. It
bites at thirty bytes too; it is just rarer, which is worse.

## What to build

Every response this server writes reaches the client that asked for it,
including the ones that refuse.

## Acceptance criteria

- [ ] A POST to a route the build does not serve receives its status line and
      body, not a reset — asserted under repetition, not once. A test that
      passes twenty-four times in twenty-five is what filed this ticket.
- [ ] The fix drains or shuts down in a way that is bounded. `MAX_DRAIN`
      already exists for the question route; whatever this does must not give
      an unauthenticated local caller a way to make the server read without
      limit.
- [ ] Plain `serve` — no `--ask`, sealed build — still never allocates a body
      buffer for a request it was never going to read. That property is why
      ticket 34 confined body reading in the first place, and it should
      survive.
- [ ] The 415 and 413 refusals on the question route get the same treatment,
      since both write a response and close while a body is in flight.
- [ ] Tested at seam 4: the real binary, real HTTP/1.1 over 127.0.0.1.

## Notes

**Do not fix this by making the test tolerate a reset.** The reset is the
symptom of a response the reader does not receive, and a test that accepts
either outcome is a guard that cannot fail — the failure mode this project has
now hit six times.

Worth knowing: the conventional fix is a half-close (`shutdown(Write)`) after
writing, or a bounded read-until-EOF before closing. Which one is right
depends on whether the server should wait on a client that never finishes
sending, and that is a decision this ticket should make explicitly rather than
inherit from whichever example was nearest.
