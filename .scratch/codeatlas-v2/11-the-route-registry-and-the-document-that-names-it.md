# Ticket 11 — the route registry, and the document that must name it

**Status:** ready
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 17 — every route the server answers is named in `docs/SECURITY.md`,
enforced by a test that derives the route set from the code itself
**Blocks:** 13 (HEAD is answered "wherever GET is", and the registry defines
wherever)
**Blocked by:** none — can start immediately

## Problem

`docs/SECURITY.md` opens by holding itself to a standard: every claim names
the code and the committed test that enforces it. The document's own
completeness is the one claim nothing enforces, and it has gone false three
times — twice in consecutive commits during V1, each caught by human review
rather than by a test. Adding a route today compiles, ships, passes every
suite, and leaves the security document quietly stale.

This is deferred V1 ticket 36, absorbed into the V2 spec. Its open question —
how to derive the route set — was answered in the 2026-08-13 interview: the
registry, not a source scanner.

## What to build

A route registry the request handler itself dispatches through, and a
committed test that fails when the security document does not name every
route in it.

## Acceptance criteria

- [ ] The request handler dispatches through a route registry: a route absent
      from the registry is not served at all, so the code and the route list
      are the same thing.
- [ ] A committed test derives the route set from the registry and fails when
      `docs/SECURITY.md` does not name every route; the failure names the
      route and the document.
- [ ] Proven able to fail: add a throwaway route, watch it trip, remove it.
- [ ] The optional ask route is in the registry only when a backend stands
      behind it, and the test is correct in both configurations.
- [ ] V1 story 9's sentence is pinned verbatim in both `README.md` and
      `docs/SECURITY.md`, against the spec as the authority.
- [ ] `docs/SECURITY.md`'s limitations gain the DNS line: the netns tests
      prove no TCP egress, not the absence of a DNS channel, and the sealed
      tree probe is the complementary guarantee.
- [ ] The test reads files and builds nothing, and runs in CI in at least one
      configuration.

## Notes

The source-scanning alternative was rejected in the interview for the reason
this ticket exists: a scanner that recognises a spelling convention cannot
fail for a route spelled unexpectedly — a guard that cannot fail,
reintroduced inside the fix.

`serve.rs` is hand-rolled so that an audit can read it (ADR-0006). The
registry must leave it readable: a `const` slice the handler walks, not a
dispatch framework.

Scope discipline: this is the serve surface and story 9's sentence, not a
verification of every claim in the security document. Anything else worth
pinning gets filed, not absorbed.
