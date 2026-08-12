# Ticket 35 — a refusal the client sometimes never receives

**Status:** done
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

- [x] A POST to a route the build does not serve receives its status line and
      body, not a reset — asserted under repetition, not once. A test that
      passes twenty-four times in twenty-five is what filed this ticket.
- [x] The fix drains or shuts down in a way that is bounded. `MAX_DRAIN`
      already exists for the question route; whatever this does must not give
      an unauthenticated local caller a way to make the server read without
      limit.
- [x] Plain `serve` — no `--ask`, sealed build — still never allocates a body
      buffer for a request it was never going to read. That property is why
      ticket 34 confined body reading in the first place, and it should
      survive.
- [x] The 415 and 413 refusals on the question route get the same treatment,
      since both write a response and close while a body is in flight.
- [x] Tested at seam 4: the real binary, real HTTP/1.1 over 127.0.0.1.

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

## What the work found

**The answer to the ticket's question is "both", and the order is the whole
decision.** A half-close and a bounded drain do different jobs, and neither
alone is the fix. `respond` now shuts the write side down, drains what the
client is still sending up to `MAX_DRAIN` bytes or `LINGER_TIMEOUT` of
silence, and only then lets the socket drop.

The half-close is what delivers the response. Sending the FIN first ends the
client's read, so a client waiting on the answer has it before the server
waits on anything at all; the client then closes, and the drain reaches EOF
immediately rather than sitting out its timeout. That ordering is also the
answer to "should the server wait on a client that never finishes sending?" —
no, and the way to not wait is to put the waiting behind the response instead
of in front of it. Draining *before* replying, the other conventional shape,
gets the ordering exactly backwards: it makes every refusal contingent on the
client finishing a body the server has already decided not to read, and it
would have re-broken `a_plain_serve_still_ignores_a_body_it_was_never_going_to_read`,
which exists because ticket 34's crosscheck found a body cap sitting in front
of routing.

The drain is what stops the connection being aborted. It is tempting to stop
at the half-close, because on Linux that alone makes the tests pass — the FIN
is processed before the RST that `close()` then sends, and `tcp_recvmsg`
reports the clean EOF in preference to the later `ECONNRESET`. Counting
kernel resets rather than reading test output shows what that is worth. Fifty
POSTs with an unread tail, against `TcpExtTCPAbortOnClose`:

| | responses delivered | resets emitted |
| --- | --- | --- |
| before the fix | 0 / 50 | 50 |
| half-close only | 50 / 50 | 50 |
| half-close then bounded drain | 50 / 50 | 0 |

The middle row is a server that aborts every connection it serves and is
rescued by the order in which the peer's kernel happens to process two
segments. That is the same species of accidental dependence that filed this
ticket, so it is not the fix; it is the fix's first half.

**Both bounds are needed for different reasons and neither can be dropped.**
`MAX_DRAIN` stops an unauthenticated local caller making the server read
without limit. `LINGER_TIMEOUT` stops it holding a thread by going quiet
mid-body — the ten-second `READ_TIMEOUT` would be a poor bound here, and it
can be far shorter precisely because the response has already been flushed,
so no reader is behind it. Nothing is retained: the bytes pass through one
4 KiB stack array and are dropped, which is how a route that reads no body
still allocates none.

**The 413 and 415 refusals were reordered, not just protected.** They were
decided *after* reading up to `MAX_DRAIN` bytes into a `Vec`, so refusing a
1 MB request meant first allocating a megabyte for it and refusing a
cross-origin post meant reading the whole thing. Both are settled by the
header block alone, so both now answer before a byte of the body is read.
That is only safe because `respond` hangs up rather than dropping the socket
on the remainder — which is why removing the hang-up now fails four tests
rather than two: `a_question_sent_as_a_browser_simple_request_is_refused` and
`an_oversized_request_body_is_refused` became guards on this ticket's fix as
a side effect of no longer being guards on a wasteful implementation. The
body read that survives is bounded by the 413 check above it rather than by a
`min` of its own, so there is one cap on a request body and not two.

### Reproducing it deterministically, and why the first attempt did not

The defect needs unread bytes in the receive queue *at the moment of close*.
The first version of the test wrote the header block, paused ten
milliseconds, then wrote the body — and passed twenty-five times out of
twenty-five, because the pause put the body **after** the close rather than
before it. Data arriving at an already-closed socket does provoke an RST, but
by then the client has its response, and Linux hands it over.

What works is arithmetic rather than timing. The server reads the header
block through a `BufReader`, whose buffer is 8 KiB, so a body larger than
that cannot be swallowed by the read that collects the headers no matter how
the segments fall. `LARGER_THAN_ONE_READ` is 32 KiB and the helper asserts
its body exceeds it, so the test cannot quietly degrade into one that proves
nothing. Failure went from occasional to certain: **40 failures in 40 runs**
against the unfixed server, on round one every time, with the ticket's own
`os error 104`.

### Measured

The full `serve` suite, run repeatedly, killing the child server each time:

| | runs | failures |
| --- | --- | --- |
| unfixed, suite as it stood | 40 | 4 (`--test-threads=4`: 9 in 40) |
| unfixed, with this ticket's tests | 40 | 40 |
| fixed, default features | 90 | 0 |
| fixed, `--no-default-features` | 75 | 0 |
| fixed, `--no-default-features --features agent-cli` | 75 | 0 |

The ticket reported one in twenty-five; unforced it measured four in forty
here, and raising contention with `--test-threads=4` took it to nine in
forty. `the_method_refusal_describes_the_server_it_came_from` turned out to
be a second victim the ticket had not named — the same 405 branch, the same
reset.

Three tampers, each reverting one half:

- **No hang-up at all**, reorder kept: four tests fail, two of them
  pre-existing, all with `os error 104` on round one.
- **Half-close, no drain**: every test passes, and the server emits fifty
  resets per fifty requests. Measured, not reasoned about; the table above.
- **Drain, no half-close**: every test passes and the suite goes from 0.04s
  to **13.13s**, because each response now waits out `LINGER_TIMEOUT` on a
  client that is waiting for the FIN before it will close. The order is worth
  a factor of three hundred.

### One workaround reversed, and one left alone

Ticket 27 wrote a test around this defect: the capability route's
cross-check POSTed an **empty** body specifically so nothing would be left
unread, with a comment explaining the reset it was avoiding. Both the
workaround and the comment are now false, so the test sends the question a
real caller would send. It is a shape that flakes on the unfixed server, so
it is a guard again rather than a detour around one.

`a_plain_serve_still_ignores_a_body_it_was_never_going_to_read` was left
exactly as it is, though this ticket's problem statement calls it written
around the defect too. Its declared-large / sent-small shape is load-bearing
for what it actually guards — that no body cap sits in front of routing —
and a version that sent the whole body could no longer tell the two servers
apart. The property this ticket cares about is covered by the new tests
instead.

**Deliberately not fixed:** a client that declares a `Content-Length` larger
than it sends still parks the question route's handler for the full ten-second
`READ_TIMEOUT` and then closes with no response at all. That is a request the
client never finished, so there is nothing to answer yet, and HTTP has no
better move than waiting; it is bounded, and it is the pre-existing behaviour
of the one route that reads bodies.

### Documentation

`docs/SECURITY.md`'s "`serve` DoS surface is loopback-local" bullet was not
made false by this change — half-open requests still cannot park threads
forever — but it became the incomplete half of a sentence, because a handler
thread now outlives its own response by up to `LINGER_TIMEOUT`. Availability
is exactly what that bullet is for, so it now states the second bound, names
both constants and the two tests that assert them under repetition, and says
that nothing is retained from the drain. Nothing else in that file or in
`README.md` needed changing: the enumeration of what a connection can be told
is unaffected, since this ticket discloses nothing new and adds no route.
