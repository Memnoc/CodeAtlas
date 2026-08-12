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
      hidden here.

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
