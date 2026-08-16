# 05 — HTTP keeps its promises

**Spec:** `docs/specs/2026-08-16-codeatlas-v3.md` — a fresh session reads
this first. This ticket is V2 ticket 13's residual list, promoted.

**What to build:** The serve surface stops cutting the three corners a
stranger's tooling will notice: every 405 names what is allowed, the
request line is exactly three tokens or nothing, and a method the server
does not implement draws 501 rather than a 405 that claims the wrong
problem.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] Every 405 carries `Allow` naming exactly the methods served at that
      path (GET and HEAD everywhere; POST too where ask is registered),
      observed on the wire — V2 ticket 13's byte-identical-405s pin is
      superseded deliberately and its test updated to assert the new shape.
      `Routes::allow` derives the list from `REGISTRY` through
      `Routes::route`, so the flag conditions apply and no second list
      exists; `transmit` grew the one optional header, so the 405 still
      cannot be written without the hang-up. The updated pin —
      `other_methods_draw_a_405_whose_allow_header_names_the_served_surface`
      — states the supersession in its doc comment, citing the V3 spec's
      own sentence; `allow_names_post_exactly_where_ask_is_registered`
      asserts the flags-included exactness with equality, never `contains`,
      on both server shapes. Proven able to fail 2026-08-16: red before the
      header existed (`left: None, right: Some("GET, HEAD")`), and by
      tamper — POST dropped from the registry walk →
      `left: Some("GET, HEAD"), right: Some("GET, HEAD, POST")`; reverted.
- [x] A request line of anything other than exactly three tokens draws
      400; `GET / HTTP/1.1 junk` no longer passes. A fourth
      `parts.next()` must be `None` — the whole line parses or none of it
      does. Fewer than three was already V2's 400 and its test rides
      unchanged. `a_request_line_of_more_than_three_tokens_is_refused_not_tolerated`
      proven able to fail 2026-08-16: red against the pre-ticket server
      (`four-token status: HTTP/1.1 200 OK` — the residual verbatim), and
      again by tamper re-tolerating the fourth token; reverted. The
      three-token control alongside proves the refusal is about the count.
- [x] An unrecognised method (`FROB`) draws 501; a recognised method not
      served at the path stays 405-with-`Allow` — the taxonomy stated in
      the tests so the next reader knows which refusal means what.
      `RECOGNISED_METHODS` is RFC 9110 §9.3's eight plus PATCH (RFC 5789);
      the doc comment of
      `an_unrecognised_method_draws_501_and_a_recognised_one_stays_405`
      states both sides in words — 501 = "this server does not implement
      the method at all" (no `Allow`: it makes no claim about the path),
      405 = "the method exists here, just not at this path". TRACE and
      CONNECT ride the 405 side as the methods most tempting to lump in
      with FROB. Proven able to fail 2026-08-16: red against the
      pre-ticket server (`FROB / status: HTTP/1.1 405 Method Not
      Allowed`), and again by tamper disabling the 501 lane; reverted.
- [x] `docs/SECURITY.md`'s served-surface prose still true; both drift
      gates green. The "Any other method is a 405 naming what is served"
      sentence rewrote into the two-refusal taxonomy with the `Allow`
      contents, the flags-included clause, and the exactly-three-tokens
      grammar, naming all five enforcing tests; the only routes the
      paragraph mentions are registry routes, and
      `every_registered_route_is_named_in_the_security_document` and
      `every_route_the_security_document_names_is_still_registered` both
      pass after the edit.
- [x] Full serve suite green in all three feature configurations
      (2026-08-16): default 296 passed 0 failed across 15 suites
      (`tests/serve.rs` 50); sealed `--no-default-features` 257 passed
      0 failed (serve 47 — the three agent-cli-gated tests compile out,
      as before); `--no-default-features --features agent-cli` 284 passed
      0 failed (serve 50). `cargo fmt --all --check` clean;
      `cargo clippy --all-targets -- -D warnings` clean in all three
      configurations. Dashboard untouched, as the slice promised.

## Decisions (2026-08-16)

- **`Allow` is derived, not listed.** GET and HEAD open every list —
  every path answers GET through the registry or the asset fallback, and
  HEAD wherever GET is — and the walk over `REGISTRY` adds each remaining
  method through `Routes::route`, so the same conditions that keep an
  unflagged route out of dispatch keep its method out of `Allow`. A route
  added later is covered without anyone remembering the header.
- **The 405 sentences stay.** "only GET is served" / "only GET, and POST
  /api/ask, are served" remain the human half and remain true; the
  supersession is the header's arrival, not the sentences' departure, so
  every existing body assertion rides unchanged.
- **"Recognised" means the methods HTTP's own core standards define** —
  RFC 9110's eight plus PATCH. The IANA extension registry (WebDAV and
  kin) is deliberately out: on a loopback dashboard serving two methods,
  anything beyond the core set is honestly not implemented, which is what
  501 says.
- **A 501 carries no `Allow`.** That header answers "what does this path
  serve", and the 501's point is that no path on this server could serve
  the method — pinned by the test's `None` assertion.

## Test-file edits beyond the three new/updated tests

- `http_method(port, method, path)` helper added — the refusal lanes'
  bodyless arbitrary-method request, built on the existing `raw_request`.
- `other_methods_still_draw_the_405_that_names_the_served_surface` renamed
  to `other_methods_draw_a_405_whose_allow_header_names_the_served_surface`
  and moved onto the helper; body assertion kept verbatim, `Allow`
  equality added, supersession documented in the doc comment.
- No other existing test or assertion touched; all pre-ticket serve tests
  pass unedited.

## Verification record (2026-08-16)

- Baseline before any change: `tests/serve.rs` 47 passed, `tests/routes.rs`
  5 passed, both 0 failed.
- Red runs, in ticket order: `Allow` `left: None`; four tokens
  `HTTP/1.1 200 OK`; `FROB` `405 Method Not Allowed` — each the pre-ticket
  behaviour named by V2 ticket 13's residual list.
- Tamper rounds after green, each tripping the guard it aimed at, each
  reverted and re-verified: POST dropped from the `Allow` walk (exactness
  assertion), fourth token re-tolerated (400 test), 501 lane disabled
  (taxonomy test). Working tree grepped clean of tamper markers before
  the suites ran.
- Suites: default 296/0, sealed 257/0, agent-cli 284/0; fmt clean, clippy
  `-D warnings` clean in all three.
