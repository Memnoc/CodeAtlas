# 05 — HTTP keeps its promises

**Spec:** `docs/specs/2026-08-16-codeatlas-v3.md` — a fresh session reads
this first. This ticket is V2 ticket 13's residual list, promoted.

**What to build:** The serve surface stops cutting the three corners a
stranger's tooling will notice: every 405 names what is allowed, the
request line is exactly three tokens or nothing, and a method the server
does not implement draws 501 rather than a 405 that claims the wrong
problem.

**Blocked by:** None — can start immediately.

**Status:** ready

- [ ] Every 405 carries `Allow` naming exactly the methods served at that
      path (GET and HEAD everywhere; POST too where ask is registered),
      observed on the wire — V2 ticket 13's byte-identical-405s pin is
      superseded deliberately and its test updated to assert the new shape
- [ ] A request line of anything other than exactly three tokens draws
      400; `GET / HTTP/1.1 junk` no longer passes
- [ ] An unrecognised method (`FROB`) draws 501; a recognised method not
      served at the path stays 405-with-`Allow` — the taxonomy stated in
      the tests so the next reader knows which refusal means what
- [ ] `docs/SECURITY.md`'s served-surface prose still true; both drift
      gates green
- [ ] Full serve suite green in all three feature configurations
