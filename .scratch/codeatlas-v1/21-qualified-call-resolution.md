# Ticket 21 — resolve qualified calls (`module::fn()`, `mod.fn()`)

**Status:** ready
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 2 — relationships: imports, calls, containment, exports
**Blocks:** none
**Blocked by:** none

## Problem

A call edge is only produced when the callee is an **unqualified name bound
directly by an import**. Every *qualified* call form — reaching a function
through the module that holds it — resolves to nothing, in every language.

Found by the fifth `/harden` walk (2026-08-10, baseline `e68c184`) by probing
**call** conventions. The four previous walks probed *import* conventions, so
this axis had never been exercised.

Measured, one probe tree per form:

| Language | Form | Edge? |
|---|---|---|
| Python | `from pkg.util import helper` → `helper()` | yes |
| Python | `from pkg import util` → `util.helper()` | **no** |
| Python | `import pkg.util` → `pkg.util.other()` | **no** |
| TypeScript | `import { helper }` → `helper()` | yes |
| TypeScript | `import * as util` → `util.helper()` | **no** |
| Rust | `use crate::util::helper;` → `helper()` | yes |
| Rust | `use util::helper;` → `helper()` | **no** |
| Rust | `crate::util::helper()` | **no** |
| Rust | `util::helper()` | **no** |

The import edges are all present and correct in every one of these cases —
this is purely the call-binding step.

Note the Rust `use util::helper;` row: a bare local-module path in a `use`
is not resolved even though `mod util;` already produced the file edge, so
that row is an *import-convention* gap feeding a call-binding gap.

## Why it matters

On CodeAtlas's own source the map holds **7** cross-file Rust call edges
against **447** same-file ones, for a 134-file crate. `src/lib.rs` — the CLI's
whole command dispatch — has **zero** outgoing call edges, though it calls
`scan::scan`, `enrich::run`, `diff::run` and `serve::serve`. The map cannot
trace the program's own top-level control flow, which is the first thing story
2 promises.

There are 21 qualified cross-module call occurrences (16 distinct) in the Rust
source, none of them edges.

It is visible in the product, not only in the data: domain flows are projected
from call chains, so the dashboard's Domain grouping puts **134 of 219** files
in `No call flow`.

## Acceptance criteria

- [ ] Every row in the table above produces a call edge to the same target a
      reader would name.
- [ ] `src/lib.rs` has outgoing call edges to `scan::scan`, `enrich::run`,
      `diff::run` and `serve::serve` in CodeAtlas's own self-scan.
- [ ] Cross-file Rust call edges in the self-scan rise substantially from 7;
      record the number reached rather than asserting a threshold.
- [ ] A qualified call whose module is *external* (`serde::from_str`) still
      produces no edge — resolving more must not invent edges to things
      outside the tree.
- [ ] A qualified call through an alias (`import * as u` / `use x as y`)
      resolves through the alias.
- [ ] No measurable scan-time regression on the C family; measure before and
      after on a synthetic tree, as ticket 20 did.
- [ ] Fixtures cover each form per language, not one instance standing in for
      the rest.

## Notes

The pattern the spec already names for imports applies unchanged to calls: **a
language's call conventions are a checklist, and the fixture that exercises
one of them is not evidence for the rest.** Ticket 20's fixture set is the
model to copy.

`bindings` in `scan.rs` maps a local name to candidate targets. Qualified
calls need the *receiver* resolved to a module and then the member looked up
inside it, which is a different lookup from the current one — expect a new
step rather than a wider `bindings` map.
