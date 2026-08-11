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
      in six languages works. Six Go cells are **ticket 37** after
      `/crosscheck`: three filed outright (both qualified-call rows and the
      unqualified-call row, which is a dot import) and three vacuous (the
      whole-module row and the two non-edge rows, which hold but cannot fail
      until 37 lands). The table asserts the gap is still there, so closing 37
      fails this file rather than letting the table go stale, and it asserts
      the ticket exists on disk, so the hatch cannot be used without using it.

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

**Superseded.** `/crosscheck` reclassified four of these cells and the counts
with them; everything below this line in this section is the record of what
the original work found, not of what the table says now. The current table,
and the mutations that prove the amended cells can fail, are at the end of
this file.

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

## What `/crosscheck` found

**Two of the eighteen non-edge cells rendered as `pass` while asserting
nothing, and the ticket said so only in prose.** `go.rs` keeps a call only
when the callee node is an `identifier`, so `util.Format(…)` — a
`selector_expression` — records no call at all, and the two Go non-edge cells
forbade an edge nothing in the system could produce. The section above admits
that. The table is the deliverable, though, and a reader looking at the table
saw green beside sixteen cells that mean something. Prose in the ticket is not
a substitute for the cell being honest.

`Verdict` gained a fourth arm, `Vacuous`, for exactly this: the convention
exists in this language, the guard really does hold, and nothing could make it
fail. It carries the guard — still asserted, because a cell that runs nothing
is worth less than one that runs something unfalsifiable — plus
`vacuous_until`, an expectation that does *not* hold today and whose arrival
is what gives the row something to say. The runner asserts both directions, so
the day ticket 37 records selector calls, all three vacuous cells fail with
*this cell is no longer vacuous — ticket 37 looks to have landed* rather than
sliding into a silent pass. That is the property the escape hatch already had
for `Filed` cells, extended to the cells that were quietly worse than filed.

**The value-receiver decoy was unreachable, and fixing the verdict would not
have fixed that.** `goproj/value.go` contained no import statement, so even
after ticket 37 binds package names that file's module map would be empty and
the decoy would stay out of reach. It now imports the `util` package and then
shadows the name with the `Logger` parameter, which is the shape that actually
tempts a resolver: the name really is bound to a module, and only the call
site's shadowing says otherwise. Measured against a sketch of ticket 37 the
guard fabricates `function:value.go:onValue -> function:util/util.go:Format`,
which it could not have done before. The external-package decoy needs the
sketch plus an unresolved-receiver fallback; with both, it fabricates the same
edge. Ticket 37 carries the measurements.

**Go's unqualified-call cell was not-applicable on a false reason.** It said a
package member is always written package-qualified. `import . "pkg"` binds
every exported name unqualified and is legal Go, which the file already knew
two rows above. A not-applicable cell contradicted by its own table is a
wrongly classified cell, not an inapplicable one, so it is now filed against
ticket 37 on a new fixture, `goproj/dot.go`. Ticket 37's dot-import criterion
names the cell and says what closing it owes: if the decision is to decline
dot imports, the decision goes in the cell's text and the cell is
reclassified — a row cannot stay filed against a closed ticket.

**Go's whole-module import passed on the plain-import cell's evidence.** The
two cells asserted the same edge byte for byte, so no mutation could fail one
and not the other, and the table rendered two independent passes for one
piece of evidence. Go has one import statement and it genuinely is both forms,
so the row is not not-applicable; what separates it from the plain form is the
qualifier binding, which is a qualified call, which is ticket 37. It is
`Vacuous` on the same grounds as the two non-edge cells.

**Rust's named-import cell did not test "named".** It asserted only
`imports file:tests/it.rs -> file:src/util.rs`, which `use root_lib::util;`
produces just as well — the member form and the module form were
indistinguishable. `rustroot/tests/it.rs` now binds `helper()` to a local
before asserting on it, because a call written inside `assert_eq!` sits in a
macro token tree and is recorded as no call at all, and the cell asserts that
call edge. Teaching `rust.rs` to bind no names now fails the row; before, it
did not.

**`cargo test` hard-panicked on a directory this repo documents as
disposable.** The shape test read `.scratch/codeatlas-v1` with an
`unwrap_or_else` panic, and CLAUDE.md says that directory goes away once the
feature ships — so the day someone tidies up, the suite reddens and blames a
missing scratch directory rather than a code fault. An absent directory is now
a shipped repository and passes. A directory that is *there* and lacks the
named ticket still fails, which is the half worth keeping: while the tickets
exist, the escape hatch has to point at one.

**`has_edge` was a third copy.** `tests/common/mod.rs` exists to stop exactly
that, and says why. It is hoisted; `scan.rs` and `conventions.rs` share it.

**`external_go_modules_with_colliding_package_suffixes_produce_no_edge` is
retired.** Both halves of it are in the table strictly more strongly: the Go
resolves-nowhere cell pins the *whole set* of import edges out of
`external.go` as empty and preflights `util/util.go` and `util/extra.go`, so
the collision stays a temptation rather than becoming true by the decoy
disappearing, and the plain-import and package-import cells pin `main.go` the
same way. A comment where it stood records the redirect. The other two
consolidation candidates the ticket predicted are **kept**, with a doc comment
each saying why: the Rust one asserts `self::util::helper()` and a whole-source
negative the table has no row for, and the Python one asserts namespace
packages, script-style imports and the bind-module-and-symbol statement. An
original that asserts more than the cell is not a duplicate.

**Two nits.** The `unreachable!` in `Cell::check` said "filtered out above" and
nothing filters — the early return on `fixture()` being `None` is what makes it
unreachable, and it says so. And `map_of` held the maps mutex across
`materialize` and a `cargo_bin` subprocess spawn, serialising every fixture in
the binary behind whichever one a test asked for first; the global lock now
covers only handing out a per-fixture `OnceLock`.

### The corrected table

`pass` / `n/a` / `ticket NN` / `vacuous NN`, rows × languages.

| Convention | TypeScript | Rust | Python | Go | C | C++ |
|---|---|---|---|---|---|---|
| a plain module import | pass | pass | pass | pass | pass | pass |
| a named/member import | pass | pass | pass | n/a | n/a | n/a |
| an aliased import | pass | pass | pass | pass | n/a | n/a |
| a namespace or whole-module import | pass | pass | pass | **vacuous 37** | n/a | n/a |
| a relative import | pass | pass | pass | n/a | pass | pass |
| a package/directory import through an initialiser or index | pass | pass | pass | pass | n/a | n/a |
| a header/source pairing (C/C++ only) | n/a | n/a | n/a | n/a | pass | pass |
| an unqualified call to an imported name | pass | pass | pass | **37** | pass | pass |
| a qualified call through an imported module | pass | pass | pass | **37** | n/a | n/a |
| a qualified call through an aliased module | pass | pass | pass | **37** | n/a | n/a |
| a qualified call through a nested module path | n/a | pass | pass | n/a | n/a | n/a |
| NON-EDGE: receiver is a value, not a module | pass | pass | pass | **vacuous 37** | pass | pass |
| NON-EDGE: a call into a package outside the repo | pass | pass | pass | **vacuous 37** | pass | pass |
| NON-EDGE: an import resolving to no file in the repo | pass | pass | pass | pass | pass | pass |

56 pass, 22 not-applicable, 3 filed, 3 vacuous. Every one of the six Go cells
that is not a plain pass is ticket 37, which is the honest shape of it: Go's
package qualifier is one missing binding, and it is load-bearing for six of
the fourteen rows.

### How the amended cells were proved able to fail

| Mutation | What it printed |
|---|---|
| `rust.rs`: a `use` binds no names | Rust named import — *no calls edge `function:tests/it.rs:helps -> function:src/util.rs:helper`*, with the import edge still present, which is the discrimination the cell was missing |
| `go.rs`: bind package qualifiers and record selector calls (a ticket-37 sketch) | both filed qualified-call cells — *this row now PASSES*; Go whole-module and Go outside-package — *this cell is no longer vacuous*; Go receiver-is-a-value — *fabricated calls edge `function:value.go:onValue -> function:util/util.go:Format`: the resolver reached the decoy* |
| the sketch **plus** an unresolved receiver falling back to any file exporting the callee | Go outside-package guard — *fabricated calls edge `function:external.go:external -> function:util/util.go:Format`* |
| an unresolved *callee* falls back to any file exporting it | Go unqualified call — *this row now PASSES* |
| `go.rs`: the import statement makes no edge | Go whole-module guard — *no imports edge `file:main.go -> file:util/util.go`* |
| `goproj/dot.go`: the decoy is renamed | the preflight — *the fixture no longer holds `function:dot.go:viaDot`, so this cell asserts nothing* |
| ticket 37 removed from `.scratch/codeatlas-v1` | the shape test — *waits on ticket 37, and no such ticket exists in …* |
| the whole of `.scratch/codeatlas-v1` moved aside | nothing: 15 passed, which is the point |
