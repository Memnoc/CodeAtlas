# Ticket 37 — `pkg.Func()` is every cross-package call Go has, and none is an edge

**Status:** ready
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

- [ ] `util.Format(…)` in `goproj/main.go` produces a call edge to
      `function:util/util.go:Format`.
- [ ] `u.Format(…)` in `goproj/alias.go`, through an import alias, produces the
      same edge — the alias, not the package's own name, is what the call site
      writes.
- [ ] A member defined in a *second* file of the package resolves too. The
      receiver names a directory, so answering with the anchor file alone is
      the wrong shape however well it scores on `util.go`.
- [ ] A dot import (`import . "p"`) either resolves its unqualified callees or
      is declined explicitly with a reason; it must not be an accident. The
      fixture is `goproj/dot.go`, and ticket 33's `/crosscheck` files the
      checklist's Go **unqualified-call** row against this ticket on it. That
      cell used to read not-applicable on the reason "a package member is
      always written package-qualified", which is false — a dot import is
      legal Go and binds every exported name unqualified. If the decision here
      is to decline, the decision has to be written into the cell and the cell
      reclassified: a row cannot stay filed against a closed ticket.
- [ ] **The two Go non-edge rows are re-proved, not assumed.** They no longer
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
- [ ] `tests/conventions.rs` is updated in both directions. **Three** cells
      move from `Verdict::Filed` to `Verdict::Holds` — the two qualified-call
      rows and the unqualified-call row above — and **three** move off
      `Verdict::Vacuous`: the two non-edge rows, re-proved by tamper, and the
      whole-module import row, which is vacuous only because the qualifier
      binding is the single piece of evidence separating it from the plain
      import. That file asserts the gap is *still there*, so it fails the
      moment this lands — which is the point.
- [ ] No measurable scan-time regression on the C family; measure before and
      after on a synthetic tree, as tickets 20 and 21 did. Package resolution
      that reads a directory per receiver is where the cost would come from.

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
