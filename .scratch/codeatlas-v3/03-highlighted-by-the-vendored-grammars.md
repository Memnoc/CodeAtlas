# 03 — Highlighted by the vendored grammars

**Spec:** `docs/specs/2026-08-16-codeatlas-v3.md` — a fresh session reads
this first, then ADR-0013.

**What to build:** Opened source arrives highlighted for the seven
grammar-covered languages and readable as plain text for everything else;
the envelope names the language; the dashboard renders the spans with its
own styles in both themes. Zero egress, zero dashboard bundle growth.

**Blocked by:** 01 — The source route behind the flag; 02 — The dashboard
opens code.

**Status:** ready

- [ ] Each of the seven vendored grammars produces highlight spans at the
      module seam — the rule beside the wire, the division ask's slice
      tests already use; an uncovered language falls back to plain text,
      stated in the envelope
- [ ] The spec's open question resolves and is recorded here on
      completion: `tree-sitter-highlight` against the pinned tree-sitter
      version, or the grammars' own highlight queries driven directly —
      either way vendored, either way zero egress
- [ ] The envelope carries highlighted HTML plus the language; the
      dashboard renders it with its own styles, legible in both themes
- [ ] No new grammar dependencies and no dashboard bundle growth;
      highlighting is not egress code and compiles in the sealed build
- [ ] A truncated file highlights exactly what is served, notice intact
- [ ] Full suite green in all three feature configurations; dashboard
      suite and typecheck green
