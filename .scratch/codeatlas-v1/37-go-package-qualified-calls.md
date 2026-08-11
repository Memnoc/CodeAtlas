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

Both halves of the convention fail together:

| Form | Fixture | Edge? |
|---|---|---|
| `import "example.com/demo/util"` → `util.Format(…)` | `goproj/main.go` | **no** |
| `import u "example.com/demo/util"` → `u.Format(…)` | `goproj/alias.go` | **no** |

The *import* edges are present and correct in both cases, including the alias.
This is purely the call-binding step, exactly as ticket 21 found for Rust,
Python and TypeScript.

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
      is declined explicitly with a reason; it must not be an accident.
- [ ] **The two Go non-edge rows are re-proved, not assumed.** Both pass today
      *vacuously*, because no selector call is recorded at all, and this ticket
      is what makes them mean something:
      - `goproj/value.go:onValue` calls `util.Format(…)` where `util` is a
        parameter holding a `Logger` — a value, not a package. No edge to
        `function:util/util.go:Format`.
      - `goproj/external.go:external` calls `util.Format(…)` where `util` is
        `github.com/external/lib/util`, outside the module. No edge to
        `function:util/util.go:Format`.
      Both decoys are already in the fixture. Ticket 21 shipped a
      fabricated-edge bug that seven mutations missed for want of exactly
      these, so a mutation that makes the resolver over-eager must fail both.
- [ ] `tests/conventions.rs` moves the two Go cells from `Verdict::Filed` to
      `Verdict::Holds`. That file asserts the gap is *still there*, so it fails
      the moment this lands — which is the point.
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
