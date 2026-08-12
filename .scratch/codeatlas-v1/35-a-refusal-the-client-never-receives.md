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

## What /crosscheck found

**The drain was bounded in bytes and in silence, and neither of those bounds
a loop.** `LINGER_TIMEOUT` is 500 ms *per read*, so a client that dribbles one
byte every 450 ms makes every read return `Ok(1)` before it expires and the
only cumulative limit left is `MAX_DRAIN`: 1,048,576 reads at 0.45 s each is
about five days of thread life, per connection, unauthenticated, on every
route — `respond` calls `hang_up` unconditionally, and `serve` is
thread-per-connection with no cap on how many exist. This ticket introduced
it. The paragraph above congratulated itself on two bounds for two reasons
without noticing that one of them cannot bound the thing it was chosen to
bound. Measured before fixing: a client dribbling a byte every 200 ms was
still being drained after six seconds, and would have been after five days.

`DRAIN_DEADLINE` is one second across the whole loop, and each read's timeout
is now `min(LINGER_TIMEOUT, what is left of it)` rather than a flat 500 ms, so
the sum of the reads cannot exceed the deadline no matter how the client
paces them. `LINGER_TIMEOUT` stays and still earns its place — it is what
makes the ordinary case, a client that has stopped, cost 500 ms instead of a
second — but it is no longer load-bearing, and the module now says so where
the constant is defined. Worst case past the response is `DRAIN_DEADLINE`
plus one read's copy: one second, from five days.

`a_client_that_keeps_sending_is_hung_up_on_rather_than_drained_forever` holds
the real binary to it. It dribbles one byte every 200 ms — comfortably inside
the per-read bound, so every read succeeds and only a deadline can end the
loop — and requires the write to start failing within four seconds. The
trickle has to run in a second thread and continue *through* the response
being read, because a client that pauses to read is a client that has gone
quiet, and going quiet is precisely the case the old per-read timeout already
handled: pause anywhere and the test passes either way. With the deadline
removed and the per-read timeout restored, it fails with *the server was still
draining a trickling client after 6s*.

**`docs/SECURITY.md` said two things that were not true, and only one of them
was this ticket's.** "A handler thread also outlives its own response by up to
half a second" was false the day it was written, for the reason above; the
bullet now states `DRAIN_DEADLINE` by name, gives all three bounds and what
each is for, and names the test. The second was there before this ticket
touched the file, and the Documentation paragraph directly above repeats it:
"half-open requests cannot park threads forever" — they can,
because `read_headers` loops on `read_line` with no line cap, no length cap
and no deadline, and `READ_TIMEOUT` is also per read. That is ticket 38, filed
from this review and deferred past V1, and it is not fixed here. What could
not wait is the sentence, which has moved out of the guarantee and into
`docs/SECURITY.md`'s Honest limitations as a bullet stating the actual
behaviour and pointing at the ticket. A limitation an auditor can read is a
different thing from a guarantee that is not true, and this file keeps
learning it: the sentence that was this ticket's own fault was written in the
same paragraph as the one it inherited, and neither was checked against the
loop it described.

**Two tests were cited for a claim neither of them makes.** The
non-retention sentence — nothing is kept from the drain, which is what keeps a
route that reads no body from allocating one — cited the two repetition tests,
which assert that refusals arrive and say nothing about allocation. The guard
for the property is `a_plain_serve_still_ignores_a_body_it_was_never_going_to_read`,
which asks a plain `serve` for the map behind a declared 200 KB body and
requires the map back at once; the file names it now, and the repetition tests
are cited for the delivery claim that is theirs.

**A response above `MAX_DRAIN` still does not arrive, and the document read as
though all of them do.** The drain stops at a megabyte and drops the socket on
the remainder, which is the original RST for exactly the requests that overrun
it. The bound is right and criterion 2 requires it; what was missing is that
the residual was neither stated nor tested.
`a_body_far_past_the_drain_bound_costs_the_client_its_refusal` sends 16 MiB and
requires the client's own write to fail — measured, a client gets about 2.8 MB
out before the pipe breaks, so the margin is not a coin flip. The response is
usually still readable out of the client's receive buffer afterwards, and the
test deliberately does not assert that: it is the kernel's ordering of a FIN
and an RST, which is the accident this whole ticket exists to stop depending
on. A client that treats a failed write as fatal, `curl` among them, never
looks. Raising `MAX_DRAIN` past the body makes the test fail with *the whole
body was accepted, so the drain read past its bound*.

**One test's docstring claimed a guard it is not.**
`the_question_routes_refusals_reach_the_client_too` said it covered "the same
close-on-unread-bytes race as the 405 above". Against `107ea88` the 413 and
415 branches read the body before deciding, so the receive queue was empty at
close and the test passes on the unfixed server — the write-up above is honest
about this and the docstring was not. What it actually guards is the pairing:
the reorder is only safe because the hang-up is there, so it fails if the
hang-up is dropped while the reorder stays, which is the combination a later
change is most likely to reach for since the reorder is the half that looks
like an optimisation. Confirmed by removing the hang-up: five tests fail, this
one on round 1 with `os error 104`.

**Four smaller things.** A docstring pointed at `post_body_late`, a helper that
does not exist under that name. `LARGER_THAN_ONE_READ` was described as "the
smallest body that is certain to outlast the server's first read" while being
32 KiB against an 8 KiB `BufReader` — the value is right and the sentence was
not, so it now says margin rather than minimum. `MAX_DRAIN` was `16 * MAX_BODY`,
which coupled a close-time courtesy bound to a request-body cap it no longer
relates to now that the body read is bounded by the 413 check above it; it is
its own megabyte, with a comment saying which of the two caps it is. And
`let _ = stream.set_read_timeout(...)` would, on the one failure it could
have, leave the drain on the ten-second `READ_TIMEOUT` and degrade the
documented bound with no trace — it now breaks out of the loop instead, on the
grounds that a read it cannot bound is a read it should not make.

**The hand-rolled client was written three times.** `http_get`, `http_post_as`
and `http_post_still_arriving` each connected, wrote, `read_to_end`, found
`\r\n\r\n` with `windows(4).position` and split off the status line. One
`read_response` does it for all three, returning a `Result` because a response
that never arrives is the thing under test — the two unwrapping callers unwrap
it. Removing the hang-up still fails all four of the original guards through
the shared helper, so the deduplication did not soften anything.

### Measured, after the amendment

| | runs | failures |
| --- | --- | --- |
| default features | 40 | 0 |
| default features, `--test-threads=4` | 40 | 0 |
| `--no-default-features` | 20 | 0 |

211 / 182 / 201 became 213 / 184 / 203 — the two new tests, in every
configuration. Both new guards were tampered and both tripped; so did the
hang-up removal, against the shared helper.
