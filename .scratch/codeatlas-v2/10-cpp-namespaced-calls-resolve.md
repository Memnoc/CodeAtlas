# Ticket 10 — C++ namespaced calls resolve

**Status:** ready
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 16 — namespaced calls like `geo::nsq()` resolve, so a C++ map has
call edges on idiomatic code
**Blocks:** none
**Blocked by:** none — can start immediately

## Problem

A C++ developer maps their repository and gets a field of disconnected boxes.
Symbols defined inside a namespace are stored under their bare name, while
every idiomatic call site spells them qualified — `geo::nsq()` — so the two
never match and no call edge is drawn. The visualization work in this lap
improves a picture that, for C++, currently has almost nothing in it.

## What to build

Namespaced symbols are stored and exported under their qualified name, which
is the form call sites use.

## Acceptance criteria

- [ ] A call to `geo::nsq()` resolves to the definition inside namespace
      `geo`, and the map draws a `calls` edge.
- [ ] Namespaced symbols carry their qualified name — `geo::nsq` — as the
      name nodes are keyed and exported by, following the shape ticket 37
      established for Go package-qualified calls.
- [ ] Nested namespaces resolve on their full qualification.
- [ ] The behaviour enters the parser convention checklist as new fixture
      rows, not as a special case in prose.
- [ ] Existing C and C++ fixtures are unaffected, and no other language's
      checklist rows change.

## Notes

**Non-goal: `using namespace`.** Resolving an unqualified call that a
`using`-directive brought into scope is a scope-tracking problem, not a naming
one, and it is not this ticket. If a fixture tempts the work in that
direction, file it — the other five parser gaps are already parked in the
spec's Out of Scope and this would join them.

Qualified names change how a symbol appears to a reader in panels and search.
That is the intended outcome, not a side effect: `geo::nsq` is what the code
calls it.
