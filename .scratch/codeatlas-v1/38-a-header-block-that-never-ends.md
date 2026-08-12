# Ticket 38 — a header block that never ends

**Status:** deferred — after V1, decided 2026-08-12
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 9 — the security posture is tested, not documented; specifically
`docs/SECURITY.md`'s availability bullet, which currently claims a bound the
code does not have
**Blocks:** none
**Blocked by:** none
**Filed:** 2026-08-12, from ticket 35's `/crosscheck`

## Problem

`serve::read_headers` reads header lines in an unbounded loop:

```rust
fn read_headers(reader: &mut BufReader<TcpStream>) -> std::io::Result<Headers> {
    let mut headers = Headers::default();
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 || line == "\r\n" || line == "\n" {
            return Ok(headers);
        }
        …
    }
}
```

There is no cap on the number of header lines, no cap on the length of one
line, and no deadline across the block. The only limit is `READ_TIMEOUT`, ten
seconds, and it applies **per read**. A client that sends one header line every
nine seconds holds a handler thread indefinitely. A client that sends one very
long line without a newline grows a `String` without bound.

`serve` is thread-per-connection, so a handful of such connections is a handful
of parked threads, and nothing caps how many exist.

**This makes a committed sentence false.** `docs/SECURITY.md`'s availability
bullet says *"half-open requests cannot park threads forever"*. They can, and
this is how. The file holds every claim to the code and the committed test that
enforces it; this claim has neither.

## Why it is deferred, and what ships instead

The hole predates V1's recent work — it has been in `serve` since ticket 09 —
and it is reachable only from loopback, by someone already running code on the
machine. That is a real limitation and not a release blocker.

What is **not** acceptable is the false sentence, and that does not wait.
Ticket 35's crosscheck amendment moves *"half-open requests cannot park threads
forever"* out of the guarantees and into `docs/SECURITY.md`'s Honest
limitations section, stating the actual behaviour and pointing here. A
documented limitation an auditor can read is a different thing from a
guarantee that is not true, and this project has spent three tickets learning
the difference.

## Why it is filed separately

Ticket 35 fixed a different defect in the same file — responses discarded by an
RST — and its review surfaced this while checking whether the new drain was
bounded. The drain bug was 35's to fix because 35 introduced it. This one
predates 35 and is independent of it: it is on the request path, not the
response path, and it would still be here if ADR-0009 had never happened.

## What to build

Reading a request is bounded in every dimension a client controls, and the
sentence in `docs/SECURITY.md` becomes true.

## Acceptance criteria

- [ ] A client that trickles header lines slower than the per-read timeout is
      disconnected after a bounded total time, not held indefinitely. Assert it
      against the real binary — drive a connection that sends a line, waits,
      sends another, and require the server to give up.
- [ ] A single header line cannot grow memory without bound.
- [ ] The number of header lines is bounded.
- [ ] The bound is a **deadline across the whole request read**, not another
      per-read timeout. A per-read timeout is what created this: any number of
      reads that each individually beat it add up to no limit at all. Ticket
      35's `hang_up` has the same shape and is being corrected in the same
      spirit — check whether one mechanism should serve both.
- [ ] `docs/SECURITY.md`'s availability bullet is true afterwards and names
      the test that makes it so.
- [ ] Plain `serve` keeps every property ticket 34 and ticket 35 established:
      no body buffer for a request it was never going to read, and every
      response it writes still reaches the client. Ticket 35's repetition
      tests must still pass, unchanged.

## Notes

**Do not solve this by lowering `READ_TIMEOUT`.** Ten seconds is generous for
loopback but the problem is not the per-read value; it is that there is no
total. Lowering it narrows the window without closing it, and makes a legitimate
slow client fail sooner.

**A thread cap is a different ticket, and possibly a better one.** Bounding the
read stops one connection holding a thread forever; it does not stop a thousand
connections holding a thousand threads. That is a larger change to the
hand-rolled shape and should be argued on its own rather than smuggled in here.
Note it in the write-up if the work makes the case.

`serve` is deliberately a screenful of `std` so an auditor can read it
(ADR-0006). Whatever this adds must survive that standard — a deadline and two
counters, not a state machine.
