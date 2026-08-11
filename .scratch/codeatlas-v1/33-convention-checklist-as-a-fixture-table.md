# Ticket 33 — story 2's convention checklist becomes a fixture table

**Status:** done
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 2 — the map captures relationships, not just a file tree (as
rewritten 2026-08-11 with a finite convention checklist)
**Blocks:** none
**Blocked by:** none

## Problem

Story 2 failed three consecutive `/harden` walks — TypeScript (ticket 17),
Rust (18), Python (20) — and each failure was found by widening the
per-language probe rather than by changing the standard. That is not bad luck.
Its scope was open-ended by construction: "every import convention in six
languages" has no bottom, so there is always a seventh convention and the
story can never be finished.

The spec rewrote it on 2026-08-11 as a closed checklist. This ticket makes
that checklist real, so that coverage is something a reader can see rather
than something a walk discovers.

## What to build

A fixture table with one row per convention per language, where a gap reads as
a failing row rather than as an absence nobody notices. After this ticket,
"does Go resolve an aliased import?" is answered by looking, not by writing a
new probe.

## Acceptance criteria

- [x] Every row of the spec's checklist has a named test, in each of the six
      V1 languages where that convention exists. `tests/conventions.rs`, one
      `#[test]` per convention over all six languages; 84 cells, none absent,
      asserted by `the_table_holds_one_reasoned_cell_for_every_convention_in_every_language`.
- [x] Rows for conventions a language does not have are marked as
      not-applicable rather than silently missing, so the table's shape shows
      coverage at a glance. 23 of the 84 cells; each carries a written reason,
      and the empty reason is itself a test failure.
- [x] The three **non-edge** rows are covered as carefully as the positive
      ones: a call whose receiver is a value rather than a module, a call into
      a package outside the repository, and an import resolving to no file in
      the repository. These are where the resolver invents edges, and ticket
      21 shipped a fabricated-edge bug that seven mutations missed because no
      fixture had a decoy. All 18 non-edge cells now name a decoy that the map
      really contains, and all 18 were made to fail by tampering — see below.
- [x] Existing fixtures are reused where they already cover a row; only the
      gaps get new ones. No new fixture *repository*; 15 files added and 8
      edited across the five that already existed, each for a cell nothing
      else could assert.
- [x] Every row either passes, or is **filed as its own ticket** and listed
      here with its number. This ticket is complete when the table is complete
      and every failing row has somewhere to live — not when every convention
      in six languages works. Two cells fail — Go's two qualified-call
      conventions — and both are **ticket 37**. The table asserts the gap is
      still there, so closing 37 fails this file rather than letting the table
      go stale, and it asserts the ticket exists on disk, so the hatch cannot
      be used without using it.

## Notes

**The escape hatch in the last criterion is deliberate and was agreed
explicitly.** Three walks found one gap each; this table will probably find
more, and some may be as large as ticket 21 was. Without the hatch this ticket
is unbounded and cannot fit a session. With it, the *table* is the deliverable
and the fixes are sequenced honestly.

Fourteen rows across six languages is up to eighty-four cells, which sounds
enormous and is not: most rows already have a test somewhere in
`crates/codeatlas/tests/scan.rs` from tickets 17, 18, 20 and 21. The work is
mostly finding them, naming them consistently, and writing down which cells
are empty.

The reason to do this at all, rather than accepting story 2 as perpetually
open: `/harden` walks the numbered story list, and a story that cannot be
finished means the release cannot be finished either.

## What the work found

**Fifty-nine cells hold, twenty-three do not apply, and two are broken — but
almost none of the fifty-nine were already *asserted*.** That is the finding,
and it is the one the ticket predicted in a milder form. The resolver has been
quietly correct about conventions no fixture ever wrote — a plain
`import "./side"`, a renamed `import { greet as hello }`,
`use super::util::helper`, an aliased Go import — and correct-but-unasserted
is exactly the state three `/harden` walks kept converting into a defect
report. Fifteen fixture files closed that, and only two cells turned out to be
genuinely broken.

### The table

`pass` / `n/a` / `ticket NN`, rows × languages. The live copy is `CHECKLIST`
in `crates/codeatlas/tests/conventions.rs`; this is what it renders.

| Convention | TypeScript | Rust | Python | Go | C | C++ |
|---|---|---|---|---|---|---|
| a plain module import | pass | pass | pass | pass | pass | pass |
| a named/member import | pass | pass | pass | n/a | n/a | n/a |
| an aliased import | pass | pass | pass | pass | n/a | n/a |
| a namespace or whole-module import | pass | pass | pass | pass | n/a | n/a |
| a relative import | pass | pass | pass | n/a | pass | pass |
| a package/directory import through an initialiser or index | pass | pass | pass | pass | n/a | n/a |
| a header/source pairing (C/C++ only) | n/a | n/a | n/a | n/a | pass | pass |
| an unqualified call to an imported name | pass | pass | pass | n/a | pass | pass |
| a qualified call through an imported module | pass | pass | pass | **37** | n/a | n/a |
| a qualified call through an aliased module | pass | pass | pass | **37** | n/a | n/a |
| a qualified call through a nested module path | n/a | pass | pass | n/a | n/a | n/a |
| NON-EDGE: receiver is a value, not a module | pass | pass | pass | pass | pass | pass |
| NON-EDGE: a call into a package outside the repo | pass | pass | pass | pass | pass | pass |
| NON-EDGE: an import resolving to no file in the repo | pass | pass | pass | pass | pass | pass |

59 pass, 23 not-applicable, 2 filed.

### The rows that failed, and where they live

**Go's two qualified-call cells — ticket 37.** `util.Format(…)` is not a style
Go code can avoid: the language has no member import, so *every* cross-package
call is package-qualified. The parser records no selector call at all, which
means the Go cross-package call graph is empty by construction rather than
sparse. In `goproj` the entry point calls `util.Format`, and `Format` is
reported as a domain-flow root — a function nothing calls. Ticket 21 left this
deliberately and recorded why: a Go package is a directory while
`resolve_import` answers with one file. That reason still holds, so the fix is
a ticket rather than a line. `goproj/util/` now holds a second file precisely
so the wrong fix can be *seen* to be wrong.

### Five cells were vacuous where I first wrote them, and the tamper is what said so

The rule the ticket set — a non-edge row must name a decoy the map really
holds — is not sufficient on its own. A decoy is only a temptation if the
resolver could actually reach it, and in three places it could not:

- **The TypeScript value-receiver row did not trip** when TypeScript was made
  to follow any dotted receiver as a module path. `resolve_import` declines
  bare specifiers outright, so `logger` reached nothing whatever the receiver
  rule said. The guard is real, but proving it takes the *pair* of mutations —
  follow the receiver *and* resolve bare specifiers beside the importer, which
  is precisely the shape of the bug ticket 21 shipped in Python.
- **The Rust value-receiver row was in the wrong file.** I first put
  `call_on_a_value` in a new `src/value.rs`, where a bare `util::` receiver
  resolves against `src/value/`, which does not exist — so the decoy was
  unreachable and the cell asserted nothing. It moved into `src/lib.rs`, the
  file that declares `mod util;`, where a bare `util::` genuinely does resolve
  to `src/util.rs`. Only then did the mutation fabricate the edge.
- **The three resolves-nowhere rows for TypeScript, C and C++ had no decoy at
  all** — `./missing`, `<stdio.h>` and `<iostream>` name nothing the fixture
  contains, so an over-eager resolver had nothing to find. Each fixture gained
  a same-named file one directory down (`src/lib/missing.ts`,
  `app/config.h`, `detail/shapes.hpp`) and each importer a specifier that
  would reach it if resolution went by name rather than by path.

That is the same failure the ticket was written to prevent, met three times
inside the work meant to prevent it. It is worth saying plainly: writing the
decoy is not the check. Running the mutation is the check.

**A latent hazard the Rust cell exposed.** `receiver_is_never_a_value` is
`true` for Rust, so an unbound receiver is resolved on sight — which is safe
only because the Rust parser never records `x.f()` at all. The two facts sit
in different files and neither mentions the other. Teach `rust.rs` to record
field expressions for any reason and `util.helper()` on a local binding
silently becomes an edge into `src/util.rs`. There is now a fixture that fails
the moment that happens.

**Go's two non-edge cells pass vacuously today**, and honestly so: nothing
records a Go selector call, so no edge can be fabricated whatever the decoys
say. They were proved failable only by first implementing a sketch of ticket
37. Ticket 37 carries them as acceptance criteria for exactly this reason —
they are the rows most likely to break when it lands, and they are the rows
that look green while it is open.

### Choices the table makes that a reader should be able to argue with

**Go's whole-module import shares its edge with the plain import.** Go has one
import statement, and it is both: it makes the file edge and binds the package
qualifier. Rather than invent a second assertion or mark a convention Go
plainly has as not-applicable, the cell passes on the same edge and says so in
its own text. What the *binding* is worth is the qualified-call row, which is
filed.

**Go's relative import is not-applicable, not untested.** `import "./util"`
is illegal inside a module, so the row does not exist for V1 Go. The parser
does carry a legacy relative branch for GOPATH-era layouts; it is untested,
and it is not a checklist row. Recorded here rather than smuggled in as one —
adding a convention is a spec change.

**The nested-path call row is not-applicable for TypeScript** on ticket 21's
finding that `a.b.c()` can never name a module in JavaScript, and for C and
C++ because `::` qualifies a namespace or a class, never a translation unit.
Four of the fourteen rows are therefore Rust-and-Python-only, which is a fact
about the languages rather than a gap.

### How every cell was proved able to fail

Eighteen mutations, run in fifteen combinations, each applied to the resolver
or a parser, the suite run, then reverted. Every one of the 59 passing cells
and both filed cells is tripped by at least one:

| Mutation | Cells it trips |
|---|---|
| TypeScript follows a dotted receiver **and** resolves bare specifiers | TS receiver-is-a-value |
| Rust records `field_expression` calls | Rust receiver-is-a-value |
| Python follows any dotted receiver | Python receiver-is-a-value |
| the C family reads a member call's field as a plain callee | C, C++ receiver-is-a-value |
| an unresolved *receiver* falls back to any file exporting the callee | TS/Rust/Python outside-package, TS/Python receiver-is-a-value |
| an unresolved *callee* falls back to any file exporting it | C, C++ outside-package |
| a specifier that resolved nowhere falls back to any file of that name | TS/Python/Go/C/C++ resolves-nowhere |
| the furthest crate of a name wins instead of the nearest | Rust resolves-nowhere |
| Go binds package names and records selector calls | both filed Go cells; with the receiver fallback, both Go non-edge cells |
| a statement binding no name makes no edge | 6 rows: plain/aliased/whole-module/relative/package/pairing |
| an imported name binds nothing | 6 rows, 14 cells across all five languages that have them |
| a qualified receiver never names a module | 6 rows, 14 cells |
| Rust declines crate-name paths / stops walking `super::` | Rust named import, Rust relative import |
| TypeScript forgets the index-file convention | TS package/index import |
| the C family drops quoted includes / stops pairing headers | C and C++ plain, relative, pairing, unqualified call |
| a decoy node is renamed out of the fixture | the preflight, on every cell that names one |

The last one is the guard on the guards. `Cell::preflight` asserts every node
a cell names is really in the map, so a decoy cannot quietly disappear and
leave "no edge to it" trivially true. Renaming two decoys produced *the
fixture no longer holds …, so this cell asserts nothing* rather than a pass.

### What the table is worth beyond today

The two things that make it more than a pretty listing are both assertions.
`the_table_holds_one_reasoned_cell_for_every_convention_in_every_language`
fails if a cell is missing, duplicated, or not-applicable without a reason —
so the shape cannot rot. And a `Filed` cell asserts its ticket **exists on
disk** and that the gap is **still present**, so the escape hatch is a
commitment in both directions: a row cannot be filed against a ticket nobody
wrote, and a fix cannot land while the table still calls the row broken.
