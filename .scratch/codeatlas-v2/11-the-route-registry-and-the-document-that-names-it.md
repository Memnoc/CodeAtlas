# Ticket 11 — the route registry, and the document that must name it

**Status:** done
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

- [x] The request handler dispatches through a route registry: a route absent
      from the registry is not served at all, so the code and the route list
      are the same thing. `serve::REGISTRY`, a `const` slice of four entries;
      `handle` finds the entry via `Routes::route` and matches on its
      endpoint — the if-cascade became one lookup and one `match`, and the
      embedded-asset fallback stays what it was: every remaining GET, stated
      as not-a-route in the registry's doc comment and in `docs/SECURITY.md`.
- [x] A committed test derives the route set from the registry and fails when
      `docs/SECURITY.md` does not name every route; the failure names the
      route and the document.
      `every_registered_route_is_named_in_the_security_document`
      (`crates/codeatlas/tests/routes.rs`). On its very first run,
      2026-08-14, it failed for real: the document had never named
      `GET /api/diff` — a fourth instance of exactly the drift the ticket's
      problem statement counted three of. The served-surface section added to
      guarantee 1 is what made it pass.
- [x] Proven able to fail, 2026-08-14: a throwaway `GET /api/throwaway`
      entry added to `REGISTRY` tripped it with —
      "docs/SECURITY.md does not name `/api/throwaway` — the server answers
      `GET /api/throwaway` (serve::REGISTRY, crates/codeatlas/src/serve.rs),
      and every route the registry holds must be named in the security
      document; a route shipping undocumented is the drift this test exists
      to catch" — then removed; the test is green again.
- [x] The optional ask route is in the registry only when a backend stands
      behind it: `Routes::route` registers the `POST /api/ask` entry only
      while `Routes::ask` is `Some`, so without `--ask` the route does not
      exist and the 405 is byte-identical to before. The test checks the
      entry unconditionally — the document describes both shapes — and the
      `const` is the same slice in every build, so the test is correct in
      both configurations; all three feature configurations ran it green
      (2026-08-14, suite results below).
- [x] V1 story 9's sentence is pinned verbatim in both `README.md` and
      `docs/SECURITY.md`, against the spec as the authority:
      `story_9s_sentence_is_pinned_verbatim_in_readme_and_security_document`
      lifts the sentence from between the V1 spec's emphasis markers and
      requires both documents to carry it word for word. Proven able to fail
      both ways, 2026-08-14: "two ways" mutated to "three ways" in README.md
      tripped it naming README.md; "has neither" mutated to "has none" in
      docs/SECURITY.md tripped it naming docs/SECURITY.md; both restored,
      README.md byte-identical to its pre-mutation snapshot.
- [x] `docs/SECURITY.md`'s limitations gain the DNS line, beside the netns
      skip-conditions bullet: the netns tests prove no TCP egress, not the
      absence of a DNS channel — a binary that fired resolver queries and
      shrugged off their failure would pass them — and the sealed
      dependency-tree probe is the complementary guarantee.
- [x] The test reads files and builds nothing — the registry is a `const`
      the test binary already links; everything else is `fs::read_to_string`
      — and runs in CI in three configurations: `.github/workflows/ci.yml`,
      the `rust` job's Test step (`cargo test --workspace`) and both
      `feature-configuration` matrix legs, which run the whole suite with
      their feature strings.

## Verification record (2026-08-14)

- `cargo test --workspace` — 254 passed, 0 failed
- `cargo test --workspace --no-default-features` — 216 passed, 0 failed
- `cargo test --workspace --no-default-features --features agent-cli` —
  242 passed, 0 failed
- `cargo fmt --all --check` — clean;
  `cargo clippy --all-targets -- -D warnings` clean in all three
  configurations
- `tests/serve.rs` (32 real-TCP tests) needed no edits: refusal messages and
  status codes are byte-identical through the registry dispatch.

## Verification record — crosscheck fix (2026-08-14)

The crosscheck found the "more" direction unenforced: a route removed from
`REGISTRY` but still described in `docs/SECURITY.md` failed nothing. Added
`every_route_the_security_document_names_is_still_registered`
(`crates/codeatlas/tests/routes.rs`): it scans the whole document for
`/api/`-prefixed path tokens — each `/api/` occurrence extended through
ASCII alphanumerics, `-` and `_`, stopped by any other character — and
fails when the registry no longer holds one. The substring limitation of
the naming check is now stated in its doc comment, and the document's
"cannot be more or less" sentence was retired for one that claims exactly
the two enforced directions.

- Proven able to fail (stale direction), 2026-08-14: a fake
  `GET /api/ghost` bullet added to the served-surface list tripped it
  with — "docs/SECURITY.md names `/api/ghost`, but serve::REGISTRY
  (crates/codeatlas/src/serve.rs) no longer holds it — a route still
  described in the security document after leaving the registry is stale,
  the same false claim as an undocumented route in the other direction" —
  then removed; docs/SECURITY.md byte-identical to its pre-mutation
  snapshot.
- Missing direction re-proven not weakened, 2026-08-14: the throwaway
  `GET /api/throwaway` registry entry tripped
  `every_registered_route_is_named_in_the_security_document` again with —
  "docs/SECURITY.md does not name `/api/throwaway` — the server answers
  `GET /api/throwaway` (serve::REGISTRY, crates/codeatlas/src/serve.rs),
  and every route the registry holds must be named in the security
  document; a route shipping undocumented is the drift this test exists to
  catch" — then removed; serve.rs byte-identical to its pre-mutation
  snapshot.
- `cargo test --workspace` — 255 passed, 0 failed
- `cargo test --workspace --no-default-features` — 217 passed, 0 failed
- `cargo test --workspace --no-default-features --features agent-cli` —
  243 passed, 0 failed (each one more than above: the new test)
- `cargo fmt --all --check` — clean;
  `cargo clippy --all-targets -- -D warnings` clean in all three
  configurations

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
