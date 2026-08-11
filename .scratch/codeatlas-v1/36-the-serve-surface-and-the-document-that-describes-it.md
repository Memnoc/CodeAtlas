# Ticket 36 — the serve surface, and the document that describes it

**Status:** ready
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 9 — approving CodeAtlas is a code review rather than a trust
exercise (the document half: an audit entry point that cannot silently fall
behind the code)
**Blocks:** none
**Blocked by:** none
**Filed:** 2026-08-11, from a pattern across tickets 32 and 27

## Problem

`docs/SECURITY.md` opens by holding itself to a standard: *"Every claim below
names the code and the committed test that enforces it — the posture is
tested, not documented."* The document's own **completeness** is the one claim
it does not hold to that standard, and the gap is not hypothetical.

Ticket 32 existed because `README.md` and `docs/SECURITY.md` had gone false
about the serving binary: they described one egress route after two had
shipped. It corrected them. One commit later, ticket 27 added
`GET /api/capabilities` and made §1 false again —

> `serve` — the flag absent — holds no provider at all and does not route
> `POST /api/ask`, so it is the same program it was before ADR-0009 rather
> than a similar one.

— because a plain `serve` now routes something that exists only to advertise
ADR-0009's feature. The clauses were still true; the conclusion was not. The
same commit left the "Honest limitations" enumeration (*"the server reads just
the map and overlay from disk and serves embedded assets"*) describing three
of four things the server answers with.

Both were caught by review rather than by a test. Nothing makes a new route
meet the document: adding one compiles, ships, passes every suite, and leaves
the security document quietly stale. That is the project's recurring failure
mode — a claim nothing can falsify — operating one level up, on the document
instead of on the code.

## What to build

A committed test that fails when the serve surface and the documents that
describe it disagree. After this ticket, the fifth route cannot ship
undocumented.

## Acceptance criteria

- [ ] Every route the server answers is named in `docs/SECURITY.md`. A route
      present in the code and absent from the document fails a test.
- [ ] The test derives the route set from the code, not from a second
      hand-maintained list — a list that must be updated alongside the
      document is the same problem with an extra step.
- [ ] It fails for the right reason: adding a route without documenting it
      trips it, and the failure message says which route and which document.
      Prove this by adding a throwaway route, watching it trip, and removing
      it.
- [ ] Story 9's sentence is pinned verbatim in both `README.md` and
      `docs/SECURITY.md`, against the spec as the authority. This was
      explicitly deferred by ticket 32 — *"a Markdown-scanning test would be a
      new pattern here, and that decision belongs to a ticket that wants
      it"* — and this is that ticket.
- [ ] The check runs in CI in at least one configuration, and its cost is
      near zero: it reads files, builds nothing.

## Notes

**How to derive the route set is the real decision, and it should be made
explicitly.** Two shapes, neither obviously right:

- *Scan `serve.rs` for route literals.* No production change, but it is a
  guard that depends on a spelling convention. A route added in a shape the
  scanner does not recognise passes silently — a guard that cannot fail,
  which is the thing this ticket exists to prevent, reintroduced inside the
  fix.
- *Give the module a route registry the responder itself uses* — a
  `pub const` slice that `handle` dispatches through, so a route that is not
  in the registry is not served at all. Costs a small refactor of a
  deliberately hand-rolled file, and buys a derivation that cannot drift
  because the code and the list are the same thing.

The second is more likely right for the same reason `serve.rs` is hand-rolled
at all: ADR-0006 requires this file to survive an audit by being read. Pick
one and record why.

**The precedent for reading a non-Rust file from a Rust test already exists.**
Ticket 27's `crates/codeatlas/tests/routes.rs` reads
`dashboard/src/app/ask.ts` and asserts its route constants match the Rust
ones, after the crosscheck found that a typo there would make the question
feature permanently and silently absent while every dashboard test passed.
Reading a Markdown file is the same shape, and this ticket is plausibly an
extension of that file rather than a new one.

**Scope discipline.** This is about the serve surface and story 9's sentence,
not about validating every claim in `docs/SECURITY.md`. A test that tried to
verify the whole document would be unbounded and would rot. If the work
suggests further claims worth pinning, file them rather than absorbing them.
