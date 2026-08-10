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

**Status:** ready

- [ ] A TypeScript file importing `./x.js` resolves to `x.ts` when that file
      is in the scanned set, and `./x.jsx` resolves to `x.tsx`
- [ ] The literal path still wins: a genuine `x.js` sitting beside an `x.ts`
      resolves to `x.js`, so JavaScript projects are unaffected
- [ ] Candidate order is fixed and documented, so resolution stays
      deterministic when several candidates exist
- [ ] The `simple` fixture gains a NodeNext-style specifier — the convention
      that was missing is now covered by the suite, not just by this fix
- [ ] Existing resolution is unregressed: extensionless specifiers, index
      files, and bare package names (which must still resolve to nothing)
- [ ] Referential integrity holds — no edge references a missing node
- [ ] Scanning this repository produces import edges throughout
      `dashboard/src/`, asserted at a floor well above today's single edge

**Worth deciding while in here, not necessarily doing:**

- `.mts` and `.cts` are not scanned at all — the TypeScript parser claims
  only `["ts"]` and the Tsx parser `["tsx"]` (`ts_js.rs:22,27`). So the
  `./x.mjs → x.mts` half of the NodeNext convention has nothing to resolve
  to. Widening the extension list is a separate, smaller slice.
- `./x.js` may also legitimately resolve to `x.d.ts`. Declaration files are
  scanned today (they end in `.ts`), so decide whether they should be import
  targets or ignored as non-source.
- The other parsers deserve the same question asked of them: each one's
  `resolve_import` was verified against a fixture written by the same author
  as the resolver, which is precisely the arrangement that produced this bug.
