# 01 — The source route behind the flag

**Spec:** `docs/specs/2026-08-16-codeatlas-v3.md` — a fresh session reads
this first, then ADR-0013.

**What to build:** With `serve --open-code`, a reader's dashboard can fetch
a mapped file's source over the loopback wire; without the flag the source
route does not exist — the `--ask` pattern. Only files that are nodes in
the map are servable, source is read live from disk, over-cap content
arrives truncated with the truncation disclosed, the capabilities route
says whether open code is on, and `docs/SECURITY.md` names the route
because the drift gates force it to.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] The source route is registered exactly when `--open-code` was given;
      without the flag a request draws the same refusal any unregistered
      route does, proven on the wire against the real binary (prior art:
      the `--ask` route-existence tests)
- [x] Only file nodes in the map resolve: an unmapped path, a symbol id,
      and a traversal-shaped request each draw 404 with no filesystem
      walk — the map is the allowlist, and there is no path-addressed
      serving to defend
- [x] Source is read live per request: a file edited after the scan serves
      its current contents; a deleted one draws a 404 naming the honest
      reason
- [x] The JSON envelope carries the source (plain text this ticket), the
      path, and a truncation flag; content past the named size cap arrives
      truncated with the flag set — the cap proven able to fail by
      lowering it and watching the test trip (lowered 512 KiB → 16 KiB,
      the exact-bytes assertion tripped, reverted)
- [x] The capabilities response carries the open-code boolean beside
      `ask`; true and false each observed on the wire, and each answer
      held to the route it describes (prior art: the capability-route
      test)
- [x] Both route drift gates green: `docs/SECURITY.md` names the route and
      its what-a-connection-can-be-told list is updated honestly — proven
      able to fail by un-naming the route (renamed every `/api/source`
      token in the document, both gates tripped naming their own
      direction, reverted)
- [x] HEAD mirrors GET on the source route through the registry's existing
      derivation
- [x] Full suite green in all three feature configurations (default,
      `--no-default-features`, `--no-default-features --features
      agent-cli` — 15 test binaries each, zero failures)
