# Ticket 12 — a bounded request read

**Status:** ready
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

- [ ] A deadline bounds the entire request read; a client that trickles
      header lines is dropped when it expires, however well each individual
      read behaves.
- [ ] Header line length is capped: an over-long line ends the request rather
      than growing a buffer.
- [ ] Header count is capped.
- [ ] The existing per-read timeout stays exactly as it is. It is not the
      bound, and removing it would remove a guard that still does its job.
- [ ] Tests drive a real connection that trickles, and require the server to
      give up within a stated margin of the deadline.
- [ ] Each cap is proven able to fail before its criterion is ticked.
- [ ] The availability sentence in `docs/SECURITY.md` becomes true, and the
      document names the bound that makes it so.
- [ ] No cap on concurrent connections — explicitly out of scope; bounding
      the read stops one connection parking a thread forever, and a
      connection cap is a larger change to the hand-rolled shape that must be
      argued on its own.

## Notes

Measure what the kernel does, not only what the test asserts. V1's lesson
from `TCPAbortOnClose` was to count kernel state rather than green tests; a
"dropped" connection that leaves a thread blocked is the same defect wearing
a passing test.

Three separate bounds, three separate failures. Do not merge them into one
"request too large" condition — the operator reading a log wants to know
which one tripped.
