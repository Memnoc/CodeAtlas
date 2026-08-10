# 17 — Resolve TypeScript's `.js`-for-`.ts` import specifiers

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** Spec story 2 — "I want the map to capture relationships —
imports, calls, containment, exports — not just a file tree, so that I can
trace how components actually connect". For TypeScript under `NodeNext`
module resolution, it currently does not: the compiler requires source to
write `import … from "./MapExplorer.js"` for a file that is `MapExplorer.tsx`
on disk, and `TsJs::resolve_import` drops every such specifier.

`crates/codeatlas/src/parsers/ts_js.rs:104` tries the literal path, then
appends each of `RESOLVE_EXTENSIONS` to it (line 107), then tries
`<path>/index.<ext>` (line 113). For a specifier of `./MapExplorer.js` that
means testing `MapExplorer.js`, then `MapExplorer.js.ts`,
`MapExplorer.js.tsx`, … then `MapExplorer.js/index.ts` — none of which can
exist. The edge is dropped and nothing reports it.

Minimal reproduction — two files importing the same helper, one edge emitted:

```ts
// util.ts
export function helper(): number { return 1; }

// nodenext.ts — the TypeScript convention
import { helper } from "./util.js";

// classic.ts — extensionless
import { helper } from "./util";
```

```
$ codeatlas scan . && jq '[.edges[]|select(.kind=="imports")]' …
classic.ts -> util.ts        # resolved
                             # nodenext.ts -> util.ts is missing
```

**Why the suite never caught it:** every relative specifier in
`crates/codeatlas/tests/fixtures/simple/` is extensionless — `./util`,
`./barrel`, `./lib`, `./alias`. Ticket 03's acceptance criteria were met
against a fixture that does not use the convention real TypeScript projects
are obliged to use. The fixture gap is as much the defect as the resolver is.

**What it costs today**, measured against CodeAtlas's own repository:

- **38 of the dashboard's 46 relative import specifiers end in `.js`**, and
  not one of them resolves. The 8 that do are `.json` and `.css`, which match
  the literal path.
- `dashboard/src/app` holds 11 source files and the map gives it **one**
  import edge (`MapExplorer.tsx -> styles.css`).
- The whole `dashboard/` subtree therefore looks unconnected, and so does
  every region summary computed over it.
- The 12-step guided tour of CodeAtlas contains **zero** of the 30 dashboard
  files, because `semantics::build_tour` ranks by import fan-in plus fan-out
  and every dashboard file scores at or near nothing. The tour of a
  Rust-plus-TypeScript project is silently all Rust.

That last one is why this is worth its own ticket rather than a footnote: the
defect is invisible in the map (no error, no warning, just absent edges) and
it degrades a headline feature in a way that reads as a design choice.

Found on 2026-08-10 while spiking the V2 visualization: region summaries
kept reporting "nothing outside it reaches in, and it reaches nothing out"
for directories that plainly do. The summaries were right; the map was wrong.

**Blocked by:** none — 02 and 03 are done, this corrects them.

**Status:** done

- [x] A TypeScript file importing `./x.js` resolves to `x.ts` when that file
      is in the scanned set, and `./x.jsx` resolves to `x.tsx`
- [x] The literal path still wins: a genuine `x.js` sitting beside an `x.ts`
      resolves to `x.js`, so JavaScript projects are unaffected
- [x] Candidate order is fixed and documented, so resolution stays
      deterministic when several candidates exist
- [x] The `simple` fixture gains a NodeNext-style specifier — the convention
      that was missing is now covered by the suite, not just by this fix
- [x] Existing resolution is unregressed: extensionless specifiers, index
      files, and bare package names (which must still resolve to nothing)
- [x] Referential integrity holds — no edge references a missing node
- [x] Scanning this repository produces import edges throughout
      `dashboard/src/`, asserted at a floor well above today's single edge

**How it landed.** `resolve_import` gained one step between the literal path
and extension inference: if the specifier ends in an extension TypeScript
emits, try the source extensions it stands for (`NODENEXT_SOURCES` — `js` to
`ts`/`tsx`, `jsx` to `tsx`). Placing it *after* the literal check is what
keeps a genuine `x.js` beside an `x.ts` resolving to itself, and the whole
four-step order is now documented on the function. Four files joined the
`simple` fixture: `nodenext.ts` reaching all three cases, `widget.tsx`, and a
`twin.js`/`twin.ts` decoy pair whose only job is to prove the rewrite never
shadows a file that exists.

Measured on this repository: import edges **88 → 129**, `dashboard/src/app`
from 1 edge to 33, every one of the 14 files under `dashboard/src/` now
carrying at least one, and the 12-step guided tour going from **zero**
dashboard files to five.

`/crosscheck` findings folded back in. The first version pinned
`enrich.rs` to `fan-in 4` — an exact count on a fixture 35 tests share, so
every future fixture file would have re-broken an enrichment test over
arithmetic it does not care about. That assertion is now shape-only, and the
arithmetic moved to where it belongs: `tests/scan.rs` parses each mechanical
tour label and checks the fan-in and fan-out it cites against the edges
actually emitted, which is a truer invariant than any constant and cannot
rot as the fixture grows. The dashboard assertion was a bare count over
`src/app` — a count is not "throughout", so it now asserts zero orphans
across all of `dashboard/src/` plus the tour consequence. The resolver's
three hand-rolled candidate loops collapsed into three iterator expressions
reading straight off the documented order, and `NODENEXT_SOURCES` lost its
dotted-versus-undotted mismatch with `RESOLVE_EXTENSIONS`.

Declined: hoisting a shared `byId` in the dashboard test (one line, two
independent tests) and trimming the fixture comments further — for the
`twin.js`/`twin.ts` pair the reason they exist is unrecoverable from the code.

**Decisions taken on the open questions:**

- `.mts`/`.cts` — **not now.** Those extensions are not scanned at all
  (`ts_js.rs` claims `["ts"]`, `["tsx"]`, `["js","jsx","mjs","cjs"]`), so a
  `./x.mjs` rewrite would have nothing to resolve to. Recorded in the
  `NODENEXT_SOURCES` doc comment; widening the extension list is its own
  slice.
- `./x.js` → `x.d.ts` — **no.** A declaration file describes an
  implementation rather than being one, and wherever both exist the literal
  path already wins. A repo holding only `x.d.ts` genuinely does not contain
  the module being imported, so no edge is the honest answer.
- **The other parsers do have the same gap, and it is now evidenced rather
  than suspected.** `crates/codeatlas/tests/*.rs` write
  `use codeatlas::map::MAP_CONTRACT_VERSION`, and this repository's map holds
  **zero** import edges from `crates/codeatlas/tests/` to
  `crates/codeatlas/src/`: the Rust parser does not resolve own-crate paths
  to the files behind them. Same shape of defect, different parser, and it
  needs its own decisions (own crate versus workspace siblings versus
  external crates), so it wants its own ticket rather than a widening of
  this one.
