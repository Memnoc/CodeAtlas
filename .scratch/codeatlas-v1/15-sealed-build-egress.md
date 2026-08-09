# 15 — Sealed build, egress suite, dual-config CI

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** The auditor's deliverable (ADR-0006): a sealed
`--no-default-features` build that contains no networking code at all — where
sending data anywhere is a compile error, not a forbidden action — plus an
egress test suite proving the standard binary's default path opens no
non-loopback sockets. CI builds and tests both configurations on every
change.

**Blocked by:** 09 — CLI serves the embedded dashboard; 12 — Claude API
provider.

**Status:** ready

- [ ] The sealed build compiles with the network feature off and links no
      networking dependencies (verified mechanically, e.g. against the
      dependency tree)
- [ ] Every command works in the sealed build; `--enrich` fails with a clear
      "not in this build" message
- [ ] Egress tests assert the default path (scan, serve, diff, share) opens
      no non-loopback sockets in the standard build
- [ ] CI builds and runs the test suite in both feature configurations
- [ ] The redaction exhaustiveness test (ticket 14) and the contract drift
      check (ticket 07) run in the same CI gate
- [ ] A short security document states the guarantee and points at the tests
      and build config that enforce it — the audit entry point
