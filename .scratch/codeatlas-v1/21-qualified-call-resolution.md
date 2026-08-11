# Ticket 21 — resolve qualified calls (`module::fn()`, `mod.fn()`)

**Status:** done
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

- [x] Every row in the table above produces a call edge to the same target a
      reader would name.
- [x] `src/lib.rs` has outgoing call edges to `scan::scan`, `enrich::run`,
      `diff::run` and `serve::serve` in CodeAtlas's own self-scan. All four,
      plus `share::run`, `scan::save` and `map::contract_schema` — seven where
      there were none.
- [x] Cross-file Rust call edges in the self-scan rise substantially from 7;
      record the number reached rather than asserting a threshold. **33**, of
      which 11 come from this ticket's own new fixtures — so **22** in
      CodeAtlas's real source, against 7 before. Cross-file call edges across
      all languages: 61 → 98. `No call flow` drops from 134 of 219 files to
      118 of 224.
- [x] A qualified call whose module is *external* (`serde::from_str`) still
      produces no edge — resolving more must not invent edges to things
      outside the tree. Each language's fixture now carries a **decoy**: a
      real export sharing the external callee's name (`from_str` in
      `rustroot/src/util.rs`, `greet` for `node:util`, `helper` for `os`).
      Without them the guard was vacuous — see Notes. `/crosscheck` then found
      a case the criterion's wording missed entirely: a receiver that is a
      plain **value** (`logger.info()`) resolving into a same-named module
      beside it. Fixed and guarded; see Notes.
- [x] A qualified call through an alias (`import * as u` / `use x as y`)
      resolves through the alias.
- [x] No measurable scan-time regression on the C family; measure before and
      after on a synthetic tree, as ticket 20 did. Interleaved A/B, 16 runs
      per arm on a 400-file / 160k-call-site C probe: **before 1.695s median,
      after 1.705s** — 0.6%, inside a 7% spread, and the two rounds disagree
      on the sign. Contrast ticket 20's regression, which was +40%.
- [x] Fixtures cover each form per language, not one instance standing in for
      the rest.

## Design note carried in from filing

`bindings` in `scan.rs` maps a local name to candidate targets. Qualified
calls need the *receiver* resolved to a module and then the member looked up
inside it, which is a different lookup from the current one — expect a new
step rather than a wider `bindings` map. (Borne out: it became a new step.)

## What was built

`Call` gained a `receiver` path and `Import` a `namespaces` list, so the two
questions stay apart: a *name* is something to look up inside a file, a
*namespace* is the file. `receiver_module` in `scan.rs` writes the receiver
back out the way source writes a module path — `["pkg","util"]` → `pkg.util`,
`["crate","util"]` → `crate::util`, via the new `module_path_separator` — and
looks it up among the modules this file's imports actually bound. A qualified
call never falls back to the unqualified paths: a same-named local function is
a different function.

**The load-bearing rule is `receiver_is_never_a_value`.** In Rust, `::` can
only separate path segments, so an unbound receiver can be resolved on sight
and `crate::util::helper()` needs no `use` anywhere. A dot promises nothing:
`logger.info()` is a value far more often than a module. So in dotted
languages a receiver is followed *only* when an import in the same file bound
that name — which is why `from pkg import util` binds `util` but leaves `pkg`
unbound, and `import pkg.util` binds the whole dotted path because that is
what a call site writes.

It cannot invent edges: it answers only with a file the map already contains,
reached by a name the file itself introduced, and the member must be an export
of that file.

## Notes

The pattern the spec already names for imports applies unchanged to calls: **a
language's call conventions are a checklist, and the fixture that exercises
one of them is not evidence for the rest.** Ticket 20's fixture set is the
model to copy.

**Two guards were vacuous and mutation testing caught both.** Seven mutations
were run; five failed the suite immediately. The two that survived:

1. *Over-eager resolution* (an unresolved receiver falling back to any file
   exporting the callee) passed all three negative assertions, because no
   fixture had a function whose name an external call also wrote. Decoys were
   added; the mutation now fails all three.
2. *`mod util;` binding a namespace* turned out to be dead weight — a bare
   receiver already falls through to the child-module lookup in
   `resolve_import`, which answers identically. Removed rather than kept on a
   hypothesis. The two differ only when a scanned crate shares the module's
   name, which real Rust calls ambiguous anyway.

**`/crosscheck` caught a fabricated-edge bug the criteria had not imagined.**
The first implementation rewrote *any* unbound receiver as a specifier and
resolved it. Python resolves bare names script-style beside the importer, so
this probe — a file importing nothing at all —

```python
# app/logger.py:  def info(msg): ...
# app/service.py:
def handle(logger):
    return logger.info("hi")
```

produced `app/service.py:handle -> app/logger.py:info`. `logger.`, `config.`,
`parser.`, `client.` over a same-named sibling module is an everyday shape, so
this would have fabricated edges across real repositories. Criterion 4 as
worded ("whose module is *external*") did not cover it, and every fixture
passed. Fixed by `receiver_is_never_a_value`; the probe is now
`pkg/uses_value.py` and the guard fails without the fix.

Two smaller findings from the same pass: the TypeScript nested-receiver branch
was dead (`a.b.c()` can never name a module in JavaScript) and is gone, and
three comments described behaviour the code did not have.

**Go and C++ are known remaining gaps, left deliberately.** `pkg.Func()` is
Go's dominant cross-package call, and it still produces no edge. A Go package
is a *directory* while `resolve_import` answers with one file, so binding the
package name would resolve members in whichever file the resolver picked and
silently miss the rest — right on a single-file package like the `goproj`
fixture, quietly wrong on a real one. Closing it means teaching the resolver
to answer with a package, which is its own change. C++ `util::helper()` is a
namespace or class qualifier, not a translation unit, so there is no module
for its receiver to name at all.

Neither is in the table this ticket was written from, and both should be
probed by a later `/harden` walk on its own axis rather than folded in here.
