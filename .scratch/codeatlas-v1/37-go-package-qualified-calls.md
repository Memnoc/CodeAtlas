# Ticket 37 — `pkg.Func()` is every cross-package call Go has, and none is an edge

**Status:** done
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 2 — the map captures relationships, not just a file tree; the
checklist rows "a qualified call through an imported module" and "a qualified
call through an aliased module", for Go
**Blocks:** none
**Blocked by:** none
**Filed:** 2026-08-11, by ticket 33's fixture table — the two Go cells it
could not turn green

## Problem

A Go call into another package is always written `util.Format(…)`. There is no
other form: the language has no member import, so the package qualifier is not
a style choice a codebase can avoid. The parser records no such call, so the
Go cross-package call graph is empty **by construction** — not sparse, empty.

Measured on `tests/fixtures/goproj`, which is a four-file `main` package plus a
`util` package that `main.go` calls:

```
call edges: 3
  function:main.go:main       -> function:server.go:run
  function:server.go:run      -> function:server.go:banner
  function:util/util.go:Format -> function:util/util.go:indent
```

`main.go` writes `fmt.Println(util.Format(run()))`. The edge to
`function:util/util.go:Format` is absent, and `Format` is therefore reported as
a *domain-flow root* — a function nothing calls — in a repository where the
entry point calls it directly.

Both halves of the convention fail together, and ticket 33's `/crosscheck`
added a third form that fails with them:

| Form | Fixture | Edge? |
|---|---|---|
| `import "example.com/demo/util"` → `util.Format(…)` | `goproj/main.go` | **no** |
| `import u "example.com/demo/util"` → `u.Format(…)` | `goproj/alias.go` | **no** |
| `import . "example.com/demo/util"` → `Format(…)` | `goproj/dot.go` | **no** |

The *import* edges are present and correct in all three cases, including the
alias and the dot import. This is purely the call-binding step, exactly as
ticket 21 found for Rust, Python and TypeScript.

## Why it was left

Ticket 21 closed the same gap in the other three languages and stopped short
here, deliberately and with a reason recorded at the time:

> A Go package is a *directory* while `resolve_import` answers with one file,
> so binding the package name would resolve members in whichever file the
> resolver picked and silently miss the rest — right on a single-file package
> like the `goproj` fixture, quietly wrong on a real one.

That reason still stands, and it is what makes this a ticket rather than a
two-line change. `goproj/util/` now holds two files (`util.go` and
`extra.go`, the latter added by ticket 33), so a fix that binds the package to
its anchor file alone can be *seen* to be wrong rather than having to be
imagined: `u.Extra(…)` would resolve to nothing while `u.Format(…)` resolved.

## What to build

A qualified call through a Go package name reaches the function that actually
defines it, wherever in the package directory that is.

## Acceptance criteria

- [x] `util.Format(…)` in `goproj/main.go` produces a call edge to
      `function:util/util.go:Format`.
- [x] `u.Format(…)` in `goproj/alias.go`, through an import alias, produces the
      same edge — the alias, not the package's own name, is what the call site
      writes.
- [x] A member defined in a *second* file of the package resolves too. The
      receiver names a directory, so answering with the anchor file alone is
      the wrong shape however well it scores on `util.go`. `main.go:second`
      calls `util.Extra(…)` and reaches `function:util/extra.go:Extra`;
      binding to the anchor alone was tried as a mutation and printed *no
      calls edge `function:main.go:second -> function:util/extra.go:Extra`*
      while `util.Format` still resolved, which is the ticket's prediction
      observed rather than argued.
- [x] A dot import (`import . "p"`) either resolves its unqualified callees or
      is declined explicitly with a reason; it must not be an accident. The
      fixture is `goproj/dot.go`, and ticket 33's `/crosscheck` files the
      checklist's Go **unqualified-call** row against this ticket on it. That
      cell used to read not-applicable on the reason "a package member is
      always written package-qualified", which is false — a dot import is
      legal Go and binds every exported name unqualified. If the decision here
      is to decline, the decision has to be written into the cell and the cell
      reclassified: a row cannot stay filed against a closed ticket.
      **Resolved rather than declined** — see the write-up. `goproj/dot.go`
      gained a second function so the cell asserts both
      `viaDot -> util/util.go:Format` and
      `viaDotSecondFile -> util/extra.go:Extra`, and it is `Verdict::Holds`.
- [x] **The two Go non-edge rows are re-proved, not assumed.** They no longer
      read as passes: `/crosscheck` gave them `Verdict::Vacuous`, which
      asserts the guard *and* asserts that no Go selector call is recorded
      anywhere, so both cells fail the moment this ticket lands and demand the
      re-proof instead of quietly continuing to assert nothing.
      - `goproj/value.go:onValue` calls `util.Format(…)` where `util` is a
        parameter holding a `Logger` — a value, not a package. The file now
        **imports the util package** too, which is what makes it a decoy at
        all: it had no import statement when ticket 33 wrote it, so nothing
        could bind `util` and no over-eager resolver could reach the decoy
        however tempting the call site looked. Measured against a sketch of
        this ticket — package names bound, selector calls recorded — the guard
        fabricates `function:value.go:onValue -> function:util/util.go:Format`.
        So the binding step has to ask whether the call site shadowed the name.
      - `goproj/external.go:external` calls `util.Format(…)` where `util` is
        `github.com/external/lib/util`, outside the module. That one needs the
        sketch *plus* an unresolved-receiver fallback to trip; the two together
        fabricate the same edge.
      No edge to `function:util/util.go:Format` from either. Ticket 21 shipped
      a fabricated-edge bug that seven mutations missed for want of exactly
      these, so a mutation that makes the resolver over-eager must fail both.
      Both re-proved, and on **different** mutations, so neither is carrying
      the other: dropping the shadow check fabricates the `value.go` edge and
      leaves `external.go` alone, and the unresolved-receiver fallback
      fabricates the `external.go` edge and leaves `value.go` alone. The
      printed messages are in the write-up.
- [x] `tests/conventions.rs` is updated in both directions. **Three** cells
      move from `Verdict::Filed` to `Verdict::Holds` — the two qualified-call
      rows and the unqualified-call row above — and **three** move off
      `Verdict::Vacuous`: the two non-edge rows, re-proved by tamper, and the
      whole-module import row, which is vacuous only because the qualifier
      binding is the single piece of evidence separating it from the plain
      import. That file asserts the gap is *still there*, so it fails the
      moment this lands — which is the point. It did: all four positive rows
      reddened before a line of parser changed. The table is now 62 pass, 22
      not-applicable, nothing filed and nothing vacuous, and ticket 33's copy
      of it is updated.
- [x] No measurable scan-time regression on the C family; measure before and
      after on a synthetic tree, as tickets 20 and 21 did. Package resolution
      that reads a directory per receiver is where the cost would come from.
      Interleaved A/B, 16 runs per arm on a 404-file / 160k-call-site C probe,
      twice: **0.853s → 0.846s** (−0.8%) and **0.835s → 0.836s** (+0.2%), both
      inside a ~3% within-arm spread and disagreeing on the sign, as ticket
      21's rounds did. The two binaries emit a byte-identical map on that
      probe. The Go cost is real and reported in the write-up rather than
      hidden here. **The criterion is corrected below** — see *What
      `/crosscheck` found*. C cannot reach this change at all, so these
      numbers are a non-regression check on the five languages the work was
      not supposed to touch, not a test of the work.

## Design note carried in from filing

The trait's `resolve_import` answers with one file, and that is the mismatch.
Two shapes worth weighing before writing code:

- Teach the *scan* to bind a package name to the set of files in the
  directory, and look the member up across the set — the receiver resolves to
  a package, not a file. `directory_shares_scope` already tells the resolver
  that a Go directory is one namespace, so the concept exists; what is missing
  is a way to say "this receiver names that namespace".
- Or bind the package name to its anchor file and let the existing
  sibling-search widen the lookup. Cheaper, and wrong in a way the fixture can
  now show: the sibling search is keyed on the *caller's* directory, not the
  callee's.

The first looks right. Neither should be chosen on this note alone.

## What the work found

**The binding shape is the first one, and the fixture settled it rather than
the argument.** A Go import binds its qualifier to a *package*, and the member
lookup runs across the set of files in the package's directory. The second
shape — bind to the anchor file and lean on the existing sibling search — was
built as a mutation and run, because the ticket asked for the decision to be
observed rather than reasoned: it resolves `util.Format` and prints *no calls
edge `function:main.go:second -> function:util/extra.go:Extra`*. The reason is
the one the design note predicted. The sibling search is keyed on the
*caller's* directory, and `main.go` sits in `.`, so widening there answers
"which other files are in my own package" when the question was "which other
files are in the package I imported". Two searches that look alike and are not.

What the shape did *not* need was a change to `resolve_import`. Ticket 21
recorded the mismatch as "`resolve_import` answers with one file", and the
temptation is to make it answer with a set. It does not have to: the one file
it answers with is the package's anchor, the anchor names a directory, and a
directory is the package. So the trait is untouched and the widening lives at
the one place that consumes a resolved module — `resolve_in_module` in
`scan.rs`, gated on `directory_shares_scope`, which already existed to say
that a Go directory is one namespace. Five of the six languages take the
early return and behave exactly as before.

**The dot import is resolved, not declined.** `import . "p"` binds every
exported name of `p` unqualified, and the parser cannot know what those names
are because it sees one file at a time — which is precisely the problem
`c_cpp` already has with a header, and it is solved there by offering every
callee the file does not define to every quoted include and letting cross-file
resolution keep the ones the target really re-exports. `bind_dot_imports` is
that, for Go, and it is *better* founded than the C version: a dot import
genuinely does bind every exported name, so the offer is not a guess about
which header provides what, it is the language rule written down. Declining
would have meant marking a legal, unambiguous Go construct as unsupported when
the machinery to support it was already in the tree under another parser.

**The shadow check belongs in the parser, and it is deliberately blunt.**
`goproj/value.go` imports the `util` package and then takes a `util Logger`
parameter, so the call site `util.Format("value")` is written identically to
`main.go`'s and only the enclosing function's own declarations tell them
apart. The resolver knows nothing about scopes; the syntax tree does. So
`local_bindings` collects every name a function declares — receiver,
parameters, named results, and every `:=`, `var`, `const`, `range`,
type-switch and channel-receive binding anywhere inside it — and a selector
call through one of those names is not recorded at all.

It is whole-function rather than block-scoped and blind to declaration order,
and both approximations are chosen rather than tolerated. They err by calling
a name shadowed where a stricter reading would not, and a shadowed name only
ever *declines* to follow a receiver. Getting that wrong costs an edge.
Getting it wrong the other way is an edge between two files with no
relationship at all, which is the bug ticket 21 shipped and which
`goproj/value.go` exists to catch. When the two error directions are that
asymmetric, the approximation should lean.

**Six mutations, and the two non-edge cells failed on different ones.** That
mattered more than the count. Ticket 33 could only say the two Go guards were
vacuous together; if they had turned out to trip on the same mutation they
would still be one piece of evidence wearing two cells.

| Mutation | What it printed |
|---|---|
| `go.rs`: the shadow check is dropped | Go receiver-is-a-value — *fabricated calls edge `function:value.go:onValue -> function:util/util.go:Format`: the resolver reached the decoy*. Go outside-package **survived** |
| `scan.rs`: an unresolved *receiver* falls back to any file exporting the callee | Go outside-package — *fabricated calls edge `function:external.go:external -> function:util/util.go:Format`*. Go receiver-is-a-value **survived**, because a shadowed receiver records no call for the fallback to reach. Also fabricated the TypeScript, Rust and Python outside-package edges and the TS/Python value-receiver ones, as ticket 33 recorded |
| `go.rs`: a specifier that resolved nowhere falls back to any package of that name | Go resolves-nowhere — *the imports edges out of `file:external.go` are `["file:util/util.go"]`, not `[]`* — and, one step behind it, Go outside-package fabricating the call edge, because a specifier that resolves binds a qualifier that resolves |
| `scan.rs`: the qualifier binds to the anchor file alone | Go qualified-call — *no calls edge `function:main.go:second -> function:util/extra.go:Extra`*; Go unqualified-call — the same for `viaDotSecondFile`. The two `Format` edges still resolved, which is the shape being wrong while scoring well on `util.go` |
| `go.rs`: an import binds no package qualifier | Go whole-module — *no calls edge `function:main.go:main -> function:util/util.go:Format`*; both qualified-call rows likewise. Go plain-import **survived**, which is the discrimination the whole-module cell lacked when it was `Vacuous` |
| `go.rs`: a dot import binds no unqualified name | Go unqualified-call — both edges gone. Every other Go row **survived**, so the dot import is asserted by itself and not by the qualifier |

**The whole-module cell is now the same shape as its TypeScript and Python
neighbours.** `/crosscheck` marked it vacuous because it asserted the
plain-import cell's edge byte for byte. It now asserts that edge plus one call
through the bound qualifier — which is exactly what the TS and Python cells in
that row assert, and those have always been passes. The qualified-call cell
asserts strictly more (the second-file member), so the rows are ordered rather
than duplicated, and the no-qualifier mutation separates whole-module from
plain-import in one direction. Perfect mutual separation is not available in a
language whose single import statement genuinely is both forms; ordering plus
one discriminating mutation is.

**A test the table cannot write.** Every cell says "this edge is present" or
"this edge is absent". None can say "and nothing else".
`a_go_package_qualifier_names_the_whole_directory_and_nothing_outside_it` pins
the complete outgoing call set of all eight `goproj` functions that write a
selector call, which is what catches the failure mode the cells are blind to:
`fmt.Println` and `strings.TrimSpace` becoming edges. Both receivers are bound
namespaces in their files — the parser binds `fmt` and `strings` like any
other — and both are saved only by `resolve_import` declining a stdlib path.
The cells would never have noticed.

**`Verdict::Filed` and `Verdict::Vacuous` are now constructed nowhere.** All
84 cells are `Holds` or `NotApplicable`, so `-D warnings` wanted both arms
deleted. They are kept behind an `#[allow(dead_code)]` carrying the reason,
because they are not leftovers: `Filed` is the escape hatch ticket 33 was
agreed on, `Vacuous` is what stops an unfalsifiable cell rendering as a pass,
and each asserts something no other verdict does. Deleting the vocabulary
would mean the next walk that finds a gap has to reinvent it, and the likelier
outcome is a row left silently red or silently green. An empty table of
exceptions is the state the table is *for*.

### What it cost

The C family is unmoved, which is the criterion. Interleaved A/B, 16 runs per
arm on a 404-file / 160k-call-site C probe, run twice: **0.853s → 0.846s**
(−0.8%) and **0.835s → 0.836s** (+0.2%), both inside a ~3% within-arm spread
and disagreeing on the sign. The two binaries emit a byte-identical map on that
probe. Five of the six languages take the early return and never enter the new
code path at all.

Go pays, and the honest number is worth writing down. On a synthetic 561-file
Go tree with 160k cross-package call sites, the before binary produced **zero**
call edges and the after binary produces **160,000** — the defect restated as
a measurement, and also most of the cost. Scan time goes 1.00s → 1.67s
(**+69%**). Isolating the halves: making every one of those members unexported,
so every lookup still runs and widens across the package and finds nothing and
no edge is emitted, costs **+29%**; the remaining forty points is building and
serialising 160k edges that did not exist before. Inside the +29%, the shadow
walk is about 0.2s. It is a second traversal of each function's subtree, and
folding it into the main walk would turn an order-blind over-approximation into
an order-dependent under-approximation, which is the unsafe direction — so it
stays.

The first cut was much worse: **+171%**, because the widening scanned every
file in the repository on each miss, and in a Go tree most members are not in
their package's anchor file. A directory index built once in `resolve_calls`
took it to +75%, and hoisting a per-node cursor allocation out of the shadow
walk took it to +69%. That is the only performance work here, and it was found
by measuring rather than by reading — the C probe says nothing about it,
because C never reaches the branch.

## What `/crosscheck` found

No severe defects. The two non-edge guards, the dot-import binding, the blast
radius on the other five languages and the table itself were all re-verified
correct. What follows are the judgement calls.

**A branch of the walk could not change an outcome, and paid an allocation per
closure to do it.** `collect` computed a fresh binding set at every
`func_literal` as well as at every function and method declaration, and the
literal arm cannot matter — for two reasons that between them cover every
literal Go can write. Inside a declaration, `gather_bindings` had already
recursed unconditionally into the literal when the enclosing function was
walked, so the inherited set is a superset of anything the literal could add
and `local_bindings(literal, …, inherited)` returns `inherited`, after a
`HashSet<String>` clone and a second traversal of the subtree. Outside one —
`var f = func(…) {…}` at package scope — there is no enclosing declaration to
inherit from, but there is also no enclosing *function*, and a call with no
caller is never recorded, so the set is never consulted. The arm is gone.

Checked rather than argued. A probe seeding closures in five shapes — a
parameter shadowing a package, a package call in a function that later
declares a closure of that name, a `:=` inside a literal, a literal that
shadows nothing, and two levels of nesting — plus a package-scope literal with
no enclosing function, plus all twelve committed fixtures, emits a
byte-identical map either way, and every Go cell keeps its verdict.

It is also faster, which is the point: the only performance work this ticket
did was hoisting a per-node allocation out of this same walk. Interleaved A/B
on a regenerated 561-file / 160,000-call-site Go probe with func literals in a
third of the caller functions, 16 runs per arm, twice with the arm order
swapped: **1.949s → 1.878s** (−3.6%) and **1.947s → 1.875s** (−3.7%), the
after arm holding the tighter within-arm spread in both rounds, maps
byte-identical.

**The escape hatch had become untested machinery.** `Verdict::Filed` and
`Verdict::Vacuous` are constructed by no row, which is the state the table
exists to reach — but it left the two arms of `Cell::check` that implement
them, and `status()`, `ticket()` and `expectations()` with them, run by
nothing. `Vacuous` is the mechanism that stops an unfalsifiable cell rendering
as a pass. It only works if it *objects*, and an escape hatch that has quietly
stopped objecting looks exactly like one with nothing to object to. Two tests
now build synthetic cells of both kinds and run them in both directions: each
passes while the gap it names is open, and fails once that gap has closed.
Three tampers, one per arm — the filed arm no longer objecting to a closed
gap, the vacuous arm no longer noticing it can now fail, the vacuous arm
skipping its guard — redden exactly the right test and nothing else.

The `#[allow(dead_code)]` is therefore **gone rather than narrowed**. The
review's objection was that it sat on the whole enum and so also silenced
future unused fields on `Holds` and `NotApplicable`, wider than the reason
written above it. With both arms now constructed and read, nothing is dead and
no allow of any width is needed: deleting the two tests and re-running clippy
prints *variants `Filed` and `Vacuous` are never constructed*, which is the
tests standing in for the attribute rather than sitting beside it. The prose
above the enum stays, because why the vocabulary is kept is a different
question from why the compiler tolerates it.

The synthetic cells sit on the `simple` fixture, on one call edge and on that
same pair reversed. The first draft used `goproj` and reused the shadowed
`value.go` receiver as its still-open gap; dropping the shadow check then
reddened three tests instead of one, two of them announcing *this row now
PASSES. Close ticket 99* about a fabricated edge. An edge absent because
nothing in the fixture writes that call cannot be closed by a bug, so a real
regression stays on the row that owns it.

**Three approximations that were true and unwritten.** None is a defect; each
is something the next reader should not have to rediscover.

The shadow suppression is wider than the write-up above disclosed. Because
`gather_bindings` recurses into nested literals, a *closure parameter* named
after a package suppresses that package for the whole enclosing function — the
probe shows `a := util.Format("outer")` losing its edge to a closure declared
on the next line. Two further shapes of legal Go lose an edge the same way:
`cfg := cfg.Load()`, where the right-hand side really is the package because a
`:=` name's scope begins only after the statement, and a shadowing declaration
in a sibling block the call never enters. All three lean the safe way, which
is the reason to keep them; the module doc now says so, and says the
suppression reaches through literals.

The widening treats every `.go` file in a directory as one package, and Go
allows `package foo_test` beside `package foo`, so an exported name defined
only in an external test package could answer a production call. No parser
here reads a `package` clause, and the sibling search already assumed the same
thing from the caller's side, so this widens a standing approximation rather
than introducing one. `resolve_in_module` says that where it explains itself.

A dot-import residual. The shadow check is consulted for selector receivers
only, never for an unqualified callee, so an uppercase local of func type
whose name a dot-imported package also exports — `F := func() {}; F()` —
resolves to the package's `F`. Contrived, and the same class of over-offer the
C header handling already carries. One sentence in `bind_dot_imports`.

**The performance criterion named a language this change cannot reach.** The
last acceptance criterion asks for no measurable regression on the C family,
and the measurement is real and was taken honestly. But five of the six
languages return early on `directory_shares_scope`, so C never enters the new
code path, and the C probe was structurally incapable of failing the criterion
it was written for. That is the shape the table itself calls `Vacuous`, sitting
in an acceptance criterion, in the ticket that emptied the table of vacuous
cells.

The criterion stands as written and is corrected here rather than reworded
there. What the C numbers are is a non-regression check: they say the change
did not leak into the five languages it was not supposed to touch, which is
worth having and is not a test of this work. The test of this work is the Go
probe, and it was measured — 1.00s → 1.67s, +69%, buying 160,000 call edges
that did not previously exist, with the halves isolated at +29% for resolution
and +40% for construction. A budget written against Go would have had to say
that the cost of *looking* is small beside the cost of the edges the looking
finds; +29% against +40% passes that, and the first cut at +171% would have
failed it loudly, which is the discrimination the C probe never had. Nothing
needs re-measuring. What was missing was the sentence saying which probe
answers which question.

**`bind_dot_imports` stays a copy of `bind_includes`, deliberately.** The two
are the same shape — the same `defined` set from symbols, the same
undefined-callee candidates, the same sort and dedup, the same `ImportedName`
fan-out — differing in one filter (`receiver.is_empty()`) and in which imports
receive the names. A shared helper in `parsers/mod.rs` taking a predicate and
a list of target indices was sketched and declined on the condition the review
set, that it be as easy to reason about as the two copies. It is not. The C
call site has to say "all of them", which cannot be written inline —
`helper(&mut analysis, 0..analysis.imports.len(), |_| true)` is E0502, so it
needs a length bound hoisted out or a materialised `Vec<usize>`, on the path
this ticket measured. More to the point, the invariant the Go copy keeps is
that *only* dot imports may receive names: a plain Go import that got them
would bind every callee in the file unqualified and fabricate edges across the
whole language. Today that is impossible to get wrong, because
`out.dot_imports` is the only list in scope. Behind a helper taking a target
list it becomes a caller's responsibility. Two readable copies on an
edge-fabrication path beat one clever one.

**The overlap between
`a_go_package_qualifier_names_the_whole_directory_and_nothing_outside_it` and
four table cells is kept.** Its doc comment already justifies it: no cell can
say "and nothing else", and pinning the complete outgoing call set of all
eight selector-calling functions is what catches `fmt.Println` and
`strings.TrimSpace` becoming edges. Re-asserting the four edges it shares with
the table is what makes that set readable in one sitting.
