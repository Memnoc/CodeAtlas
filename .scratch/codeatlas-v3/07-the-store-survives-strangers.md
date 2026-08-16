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

**Status:** done

- [x] Read → rewrite preserves an unknown top-level section, its content
      byte-for-byte
- [x] Proven able to fail by tamper — drop the preservation, watch the
      test name the lost section — before the criterion above is ticked
- [x] The store version stays 2; a store of another version still reads
      as empty, exactly as before
- [x] This repository's own committed store round-trips unchanged through
      a plain scan (never re-bought, never modified — the standing rule)
- [x] Full suite green in all three feature configurations

**What "byte-for-byte" was honestly proven as:** unknown sections ride
through `serde(flatten)` into a `BTreeMap<String, serde_json::Value>`,
so the section's JSON *content* is preserved exactly (every key, string,
and number — asserted by value equality against the fixture), while the
first rewrite re-normalises formatting with the store's own pretty
printer; from then on every rewrite is byte-identical, which the same
test pins with a second save. The repo's serializer cannot guarantee
byte-identity of a foreign section's source formatting, so content
equivalence is the criterion the test asserts — recorded here per the
spec's instruction to prove whichever the test can honestly claim.

The tamper log (all reverted before ticking): (1) preservation dropped in
`save_store` — the test fails naming the lost `symbol_docs` section;
(2) the flatten capture narrowed to strings — the reattach-beside-
strangers guard fails; (3) the version gate removed from `load` — the
not-mined-for-salvage guard fails; (4) a nondeterministic entry injected —
the byte-identical-rewrites guard fails.
