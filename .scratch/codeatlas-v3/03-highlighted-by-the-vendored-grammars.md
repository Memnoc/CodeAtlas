# 03 — Highlighted by the vendored grammars

**Spec:** `docs/specs/2026-08-16-codeatlas-v3.md` — a fresh session reads
this first, then ADR-0013.

**What to build:** Opened source arrives highlighted for the seven
grammar-covered languages and readable as plain text for everything else;
the envelope names the language; the dashboard renders the spans with its
own styles in both themes. Zero egress, zero dashboard bundle growth.

**Blocked by:** 01 — The source route behind the flag; 02 — The dashboard
opens code.

**Status:** done

- [x] Each of the seven vendored grammars produces highlight spans at the
      module seam — the rule beside the wire, the division ask's slice
      tests already use; an uncovered language falls back to plain text,
      stated in the envelope
- [x] The spec's open question resolves and is recorded here on
      completion: `tree-sitter-highlight` against the pinned tree-sitter
      version, or the grammars' own highlight queries driven directly —
      either way vendored, either way zero egress
- [x] The envelope carries highlighted HTML plus the language; the
      dashboard renders it with its own styles, legible in both themes
- [x] No new grammar dependencies and no dashboard bundle growth;
      highlighting is not egress code and compiles in the sealed build
- [x] A truncated file highlights exactly what is served, notice intact
- [x] Full suite green in all three feature configurations; dashboard
      suite and typecheck green

## The open question, resolved: `tree-sitter-highlight`, first try

The crate pairing worked without a fallback. `tree-sitter-highlight` is
released in lockstep with `tree-sitter` itself, and `0.26.12` exists on
crates.io beside the repository's pinned `tree-sitter = "0.26.12"` — the
lock resolves to **one** shared `tree-sitter`, no duplicate, no version
negotiation. So the chosen path is the ADR's first preference: the one new
crate on the existing grammar family, driving the seven grammars' own
bundled `HIGHLIGHT(S)_QUERY` constants — which is to say both halves of the
open question turned out to be the same decision, the crate being nothing
but the driver for those queries. Vendored like everything else, zero
egress (its dependencies are `regex`, `streaming-iterator`, `thiserror` —
no networking family member, and `tests/sealed.rs`'s probes stay green),
compiled unconditionally so the sealed build highlights like the default
one.

Two upstream details the implementation leans on, verified in its source
before leaning: the HTML renderer escapes `<`, `>`, `&`, `'`, `"` (the
envelope's safety), and it closes every open span at each newline and
reopens it after (the dashboard's line-by-line rendering). One detail it
corrects: the renderer appends a trailing newline the input may not have
had; `highlight.rs` takes it back off, because a truncated file's cut must
stay exactly the cut.

Layered languages concatenate their queries base-first — C++ over C,
TypeScript (and TSX) over JavaScript, JSX over JavaScript — because in
this crate the last pattern matching a node wins, so the specific layer
must ride last to override its base.

## Where things landed

- `crates/codeatlas/src/highlight.rs` — the module seam: extension → one
  of the seven grammars (the scanner's own families) or the stated
  `plain text` fallback; unit tests for all seven, the fallback, the
  escaping, the exact-cut composition, per-line span balance, and a drift
  guard reading the dashboard stylesheet so every emitted `hl-…` class
  stays bound to a colour.
- `crates/codeatlas/src/serve.rs` — `source_envelope` clips first, then
  highlights the clipped text; the envelope now carries `html` +
  `language` beside `path` + `truncated`.
- `dashboard/src/app/source.ts`, `SourcePanel.tsx`, `styles.css` — the
  envelope type, the per-line rendering of the server's spans (the lit
  and truncation mechanics of ticket 02 unchanged), the language chip,
  and the `--code-*` token palette defined in all three theme blocks
  (Rosé Pine Dawn and Moon values).
- Wire tests updated to read the text under the markup; new wire test for
  the language statement, the plain fallback, and `<script>`-in-source
  arriving as entities. Dashboard tests for spans-as-markup,
  entities-as-text, line identity, and the language statement; a
  stylesheet-contract describe holds the both-themes legibility
  mechanically.

Bundle, measured against the pre-ticket build: JS 431.51 → 431.69 kB,
CSS 44.35 → 45.45 kB — the growth is the stylesheet, as the ADR allowed,
and no client-side highlight code exists to grow it further.
