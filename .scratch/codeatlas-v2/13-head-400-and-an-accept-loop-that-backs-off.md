# Ticket 13 — HEAD, 400, and an accept loop that backs off

**Status:** ready
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 19 — HEAD is answered wherever GET is, and a malformed request
draws a 400 instead of a silent close; 20 — accept errors back off rather than
spin
**Blocks:** none
**Blocked by:** 11 — "wherever GET is" is defined by the route registry

## Problem

Three places the server does not behave the way HTTP promises. A HEAD request
gets the non-GET refusal, so an ordinary client checking whether a route
exists is told it does not. A request the parser cannot make sense of gets the
connection closed with no status at all, which the client can only report as
a network failure. And an accept error spins the loop at full speed, so
file-descriptor exhaustion burns a core instead of degrading service.

## What to build

HEAD answered wherever GET is, 400 on a malformed request, and an accept loop
that backs off.

## Acceptance criteria

- [ ] HEAD is answered for every route that answers GET, with the same status
      and headers and no body — derived from the registry, never from a
      second hand-maintained list.
- [ ] A request that cannot be parsed draws a 400 with a short body instead of
      a silent close.
- [ ] Accept errors back off rather than spin; the loop yields under a
      sustained error condition instead of consuming a core.
- [ ] Existing refusals are unchanged: non-GET methods still get their 405
      with the message that names what is actually served, and unknown paths
      still get their 404.
- [ ] Each behaviour is tested over real TCP against the real binary, and
      proven able to fail.

## Notes

The 405 message already distinguishes the two server shapes — with and
without a question backend. HEAD must not disturb that: it is a method the
server *answers*, not a method it refuses more politely.

Back-off means a bounded pause, not a retry budget and not a shutdown. The
server keeps serving; it just stops spinning.
