# Ticket 13 — HEAD, 400, and an accept loop that backs off

**Status:** done
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

- [x] HEAD is answered for every route that answers GET, with the same status
      and headers and no body — derived from the registry, never from a
      second hand-maintained list. `handle` routes a HEAD through the
      registry's GET entries (`routes.route("GET", path)`) and withholds
      the body in `respond_head`, which shares `transmit` with `respond`
      so "same status and headers" is a property of the code. The asset
      fallback — GET's own remainder — answers HEAD the same way.
      `head_is_answered_wherever_get_is_with_gets_headers_and_no_body`
      walks the REGISTRY's GET entries itself, plus the fallback's 200 and
      404. Proven able to fail 2026-08-14: red against the pre-ticket
      server ("HEAD /api/map must carry GET's status: left 405, right
      200"), and the no-body arm by mutation (`respond_head` made to send
      the body → "HEAD /api/map must send no body, got 35270 bytes";
      restored).
- [x] A request that cannot be parsed draws a 400 with a short body instead of
      a silent close. The malformed taxonomy: a head that is not UTF-8
      (was an I/O error and a dropped connection), a request line short of
      its parts, and a request line with no `HTTP/` version (was silently
      served). Bodies name the fault: "the request head is not valid
      UTF-8" / "malformed request line: a request is `<method> <target>
      HTTP/<version>`". Size and pace stay in their own lanes — the 408
      and both 431s byte-identical.
      `a_request_that_cannot_be_parsed_draws_a_400_instead_of_a_silent_close`
      proven able to fail 2026-08-14: red against the pre-ticket server —
      "a malformed request must draw a response, not a closed connection:
      no header/body separator in response".
- [x] Accept errors back off rather than spin: `ACCEPT_BACKOFF`, 100 ms of
      `thread::sleep` on any `accept` error before the next attempt — a
      bounded pause, no retry budget, no shutdown.
      `accept_errors_back_off_instead_of_burning_a_core` forces the real
      error on the real binary: `serve_starved` lowers the child's fd
      budget to 40 via the shell's own `ulimit` (`exec` keeps the PID),
      80 connections are parked so every accept past the budget is EMFILE
      with the queue never emptying, and the child's own `/proc` CPU clock
      is read across a 3-second window; then the pressure is lifted and
      the server must answer again. TESTED, not reasoned, 2026-08-14: red
      against the spinning loop burned 300 clock ticks over the 3 s window
      (a full core); green with the backoff, 0 ticks; recovery answered
      within the 10 s patience.
- [x] Existing refusals are unchanged: the two 405 sentences byte-identical
      ("only GET is served" / "only GET, and POST /api/ask, are served"),
      404s unchanged, every pre-ticket serve test passing unedited.
      `other_methods_still_draw_the_405_that_names_the_served_surface`
      pins PUT/DELETE/OPTIONS/PATCH to the exact plain-shape sentence;
      proven able to fail 2026-08-14 by disabling the 405 branch
      (`if false` mutation → "PUT status: HTTP/1.1 404 Not Found";
      restored). HEAD's move out of the 405 lane is pinned by the two HEAD
      tests above.
- [x] Each behaviour is tested over real TCP against the real binary, and
      proven able to fail — dated outputs above; five tests added to
      `crates/codeatlas/tests/serve.rs`.

## Notes

The 405 message already distinguishes the two server shapes — with and
without a question backend. HEAD must not disturb that: it is a method the
server *answers*, not a method it refuses more politely.

Back-off means a bounded pause, not a retry budget and not a shutdown. The
server keeps serving; it just stops spinning.

## Decisions (2026-08-14)

- **Content-Length on a HEAD is GET's, computed from GET's actual body.**
  RFC 9110 §9.3.2 says the header SHOULD match what GET would send; the
  implementation builds the full GET response and withholds the bytes in
  `respond_head`, so the promised length can never be a different body's.
  The cost is a disk read the server already pays on every GET of the same
  route — accepted for a loopback dashboard over inventing lengths.
- **HEAD /api/ask draws the 404 GET /api/ask has always drawn**, on both
  server shapes. The ask route is POST-only; HEAD mirrors GET everywhere,
  including where GET is not served — `GET /api/ask` falls to the asset
  lookup and 404s, so HEAD does identically. Answering 405 instead would
  take exactly the second method-aware list the criterion forbids, and
  would refuse a method this server answers. Pinned by
  `head_of_the_post_only_question_route_mirrors_gets_404`, which also
  proves POST beside it still answers 200.
- **The 405 sentences stay byte-identical** even though "only GET is
  served" now has HEAD answered beside it: HEAD is GET's metadata twin,
  the sentence is still the truth every *refused* method needs, and the
  brief said the two shapes' messages are not to be disturbed.
- **A versionless request line is malformed, not tolerated.** HTTP/1.1's
  request-line grammar is three parts; before this ticket `GET /api/map`
  (no version) was silently served, which was an accident of
  `split_whitespace`, not a decision. HTTP/0.9 clients are not a
  constituency on a loopback dashboard port.
- **The accept backoff is flat, and errors are not inspected.** 100 ms on
  any accept error: distinguishing ECONNABORTED from EMFILE would be
  cleverness on an audited surface for at most 100 ms of latency on a
  connection that already aborted. Flat rather than exponential for the
  same reason — a bounded pause was asked for, and a constant is the most
  auditable bound there is (ADR-0006).
- **The backoff is tested at its real value through real starvation** —
  no `#[cfg(test)]` seam, per the brief: the shell's `ulimit` starves the
  spawned binary, `/proc/<pid>/stat` (utime+stime) is the witness, and
  both sides of the bound were measured (300 ticks spinning, 0 backed
  off). The test costs ~4 s of wall time in sleeps; recorded in the
  verification section.

## Test-file edits beyond the five new tests (named per the brief)

- `serve_with`'s spawn/URL-parse tail factored into `adopt(child)` so
  `serve_starved` shares the startup contract instead of duplicating its
  parsing; no existing test or assertion touched, and all 35 pre-ticket
  serve tests pass unedited.
- New helpers only: `http_head`, `raw_request`, `serve_starved`,
  `cpu_ticks`, and one permanent `eprintln!` of the measured window in the
  backoff test (the same pattern the trickle test uses).

## Verification record (2026-08-14)

- Baseline before any change: `cargo test --workspace` exit 0, every suite
  ok (tail capture; `tests/serve.rs` 35 passed in 21.43 s).
- After: `cargo test --workspace` — 277 passed, 0 failed (272 + the 5 new;
  `tests/serve.rs` 40 passed, 21.43 s — the backoff test's ~4 s of sleeps
  rides inside the deadline test's existing wall time).
- `cargo test --workspace --no-default-features` — 238 passed, 0 failed
  (serve 37: the sealed build compiles three fewer serve tests).
- `cargo test --workspace --no-default-features --features agent-cli` —
  265 passed, 0 failed (serve 40).
- Both SECURITY.md drift directions green after the document edits:
  `every_registered_route_is_named_in_the_security_document` and
  `every_route_the_security_document_names_is_still_registered`.
- `cargo fmt --all --check` clean; `cargo clippy --all-targets
  -D warnings` clean in all three configurations.
- Backoff measurements, same day: spinning loop (pre-ticket server, the
  red run) 300 clock ticks over the 3 s window — a full core; backed-off
  loop 0 ticks; recovery after the pressure lifted, inside the 10 s
  patience. Recorded in `docs/SECURITY.md`'s loopback-DoS bullet with the
  date.

**Residuals from the crosscheck (2026-08-14), accepted — V-next harvest
material, none a fault of this ticket:**

- The 405 still omits the `Allow` header RFC 9110 §15.5.6 makes a MUST.
  Pre-existing, and criterion 4's byte-identical-405s constraint forbade
  touching it here; story 19's "the way HTTP promises" is otherwise served.
- `starts_with("HTTP/")` accepts a four-token request line
  (`GET / HTTP/1.1 junk`); strict grammar is exactly three parts.
- `FROB /x HTTP/1.1` draws the unchanged 405; purist HTTP wants 501 for an
  unrecognised method. Pinned as-is by this ticket's tests.
- The backoff test is Linux-only (`/proc`), matching the thread-count
  precedent from ticket 12; the reviewer ran it and measured 0 ticks in
  3.62 s with a wide margin, and verified failure paths leak neither the
  parked connections nor the starved child.
