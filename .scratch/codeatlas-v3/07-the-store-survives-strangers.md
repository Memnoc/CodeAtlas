# 07 — The store survives strangers

**Spec:** `docs/specs/2026-08-16-codeatlas-v3.md` — a fresh session reads
this first, then ADR-0014's forward-compat consequence. This ticket is V2
ticket 07's residual, promoted because distribution multiplies old
binaries.

**What to build:** Reading and rewriting an annotation store preserves the
top-level sections this binary does not understand, so a binary released
today can never silently drop prose a newer binary's owner paid for. The
first distributed binary is the last moment this fix is free.

**Blocked by:** None — can start immediately. Must complete before ticket
09 (the edge is recorded there).

**Status:** ready

- [ ] Read → rewrite preserves an unknown top-level section, its content
      byte-for-byte
- [ ] Proven able to fail by tamper — drop the preservation, watch the
      test name the lost section — before the criterion above is ticked
- [ ] The store version stays 2; a store of another version still reads
      as empty, exactly as before
- [ ] This repository's own committed store round-trips unchanged through
      a plain scan (never re-bought, never modified — the standing rule)
- [ ] Full suite green in all three feature configurations
