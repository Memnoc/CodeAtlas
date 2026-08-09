# 14 — Share artifact

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** `codeatlas share` produces one self-contained HTML file a
colleague opens by double-click — no server, no token, nothing installed. Its
content passes through allowlist redaction, an exhaustiveness test derived
from the contract schema guarantees no field ships unclassified, and the
artifact tells its reader what was redacted (ADR-0006, spec stories 8 and 10).

**Blocked by:** 07 — Published contract; 08 — Dashboard renders a map.

**Status:** done

- [x] The share command emits a single self-contained HTML file (renderer +
      redacted map inlined) that opens from the filesystem
- [x] Redaction is an allowlist: every schema field is classified share-safe
      or redacted
- [x] A schema-derived exhaustiveness test fails the build when a field is
      unclassified — a new field cannot ship silently
- [x] The artifact displays a disclosure of what was redacted
- [x] The artifact makes zero external requests when opened
- [x] Test: generate from a fixture map, assert redacted fields absent from
      the file's bytes and disclosure present
