# Ticket 10 — C++ namespaced calls resolve

**Status:** done
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

- [x] A call to `geo::nsq()` resolves to the definition inside namespace
      `geo`, and the map draws a `calls` edge. `cppproj/uses_geo.cpp` calls
      `geo::nsq(2)` through `#include "geo.hpp"` and the map draws
      `function:uses_geo.cpp:use_geo -> function:geo.cpp:geo::nsq`; a
      qualified call inside the defining file resolves too
      (`geo::inner::deep -> geo::nsq`, no include involved). Proved able to
      fail 2026-08-14: mutation A below.
- [x] Namespaced symbols carry their qualified name — `geo::nsq` — as the
      name nodes are keyed and exported by, following the shape ticket 37
      established for Go package-qualified calls. Nodes are
      `function:geo.cpp:geo::nsq`, `class:geo.hpp:geo::Disc`; the exports
      edge targets the qualified name (linkage is unchanged by a namespace:
      non-static means exported, as before). Where the shape *diverges* from
      Go's receiver binding, and why, is recorded below. Proved able to fail
      2026-08-14: mutations C1, C2, D.
- [x] Nested namespaces resolve on their full qualification.
      `geo::inner::deep` is declared in geo.hpp's classic nested blocks,
      defined in geo.cpp's compact C++17 `namespace geo::inner`, and the call
      `geo::inner::deep(3)` resolves across files on the full path. Proved
      able to fail 2026-08-14: mutation B, which qualified with only the
      innermost namespace and reddened exactly this cell while `geo::nsq`
      stayed green.
- [x] The behaviour enters the parser convention checklist as new fixture
      rows, not as a special case in prose. The two C++ cells that were
      not-applicable on "no module for the receiver to name" —
      qualified-call and nested-path — are now `Verdict::Holds` on new
      `cppproj` fixture rows (the reclassification move ticket 33's
      /crosscheck established for Go's unqualified-call cell). The table
      reads 64 pass, 20 not-applicable, nothing filed, nothing vacuous.
- [x] Existing C and C++ fixtures are unaffected, and no other language's
      checklist rows change. Measured 2026-08-14 10:47–10:52: the HEAD
      binary's maps of `cproj`, `cppproj` and `polyglot` (pre-ticket file
      sets) are **byte-identical** to the post-change binary's maps of the
      same trees; the five fixture files this ticket adds are additions, no
      committed file was edited. `git diff tests/conventions.rs` touches the
      three C++ cells and nothing else, so every other language's rows are
      byte-identical.

## Notes

**Non-goal: `using namespace`.** Resolving an unqualified call that a
`using`-directive brought into scope is a scope-tracking problem, not a naming
one, and it is not this ticket. If a fixture tempts the work in that
direction, file it — the other five parser gaps are already parked in the
spec's Out of Scope and this would join them.

Qualified names change how a symbol appears to a reader in panels and search.
That is the intended outcome, not a side effect: `geo::nsq` is what the code
calls it.

**Inherited from ticket 16's crosscheck (2026-08-13).** The magnify lens
relates files by `imports` only — recorded with its reason in `graph.ts`
beside `neighbourhoodOf`. The call edges this ticket creates are exactly the
case that limitation bites: a C++ file connected to its neighbours only
through calls will magnify alone while the info panel names those calls.
Nothing here needs to fix that — but walk it once on the C++ fixture and
record in this ticket what the reader actually sees, so the limitation stays
a decision and not a surprise.

## What the work found

**The qualification lives in the name, not in a receiver — the one place
the Go shape cannot be followed literally, recorded as the ticket asked.**
Ticket 37's mechanism binds a call's receiver to a module (a file, standing
for a directory) and looks the member up inside it. C++ has nothing for
that receiver to name: `geo::` qualifies a namespace, never a translation
unit — the reason the two checklist cells were not-applicable, and it stays
true. So the spec's decision is implemented the other way round, and it is
the same shape one level up: *the form the call site writes is the form the
definition is stored and exported under*. A symbol defined inside
`namespace geo` is stored as `geo::nsq`; a `qualified_identifier` callee is
recorded as its own normalized text (`geo::nsq`, receiver empty); the two
match by name through the machinery that already existed — same-file
lookup, `bind_includes`' offer to quoted includes, the header/source PAIR
re-export (prototypes inside namespace blocks are re-exported qualified).
`scan.rs` and the `Parser` trait are untouched; the whole change is inside
`c_cpp.rs`, and the other five languages cannot reach any of it.

**Reader-visible, as intended:** panels and search now say `geo::nsq`,
which is what the code calls it. An explicit global qualification
`::hidden()` normalizes to the bare `hidden` it names.

**Nested namespaces, both spellings.** Classic nested blocks
(`namespace geo { namespace inner { … } }`) accumulate by recursion; the
compact C++17 `namespace geo::inner { … }` arrives as one name node whose
text is the whole path. Both store `geo::inner::deep`, and the fixture
deliberately declares in one spelling and defines in the other.

**One new guard, leaning the way the Go shadow check leans.** Storing
qualified names creates a hazard the old parser could not have: an
*unqualified* call inside `namespace geo` to a sibling `nsq` no longer
matches anything in its own file (the file defines `geo::nsq`), so
`bind_includes` would have offered the bare name to every quoted include —
and a header whose pair implements a **global** `nsq` would answer a call
C++ name lookup gives to `geo::nsq`. So a namespaced definition now also
claims its trailing segment: a callee matching the tail of a defined
qualified symbol is not offered to the includes at all. Declining costs an
edge; offering fabricates one between two files with no relationship, and
with the error directions that asymmetric the approximation leans
(`cppproj/bare.hpp`/`bare.cpp` is the committed decoy; mutation E is the
proof). The cost is the recorded non-goal: an unqualified call to a
same-namespace sibling — `nsq(k)` inside `geo::twice` — resolves to
nothing this lap, same-file though it is. Resolving it is enclosing-scope
name lookup, the same scope-tracking family as `using namespace`, parked
with it.

**Residuals, written down rather than rediscovered.**

- An out-of-line qualified definition at *file scope* —
  `double geo::nsq(int) { … }` outside any namespace block — is
  syntactically indistinguishable from a method definition
  (`double Circle::area() { … }`): both are one `qualified_identifier`
  declarator, and only a symbol table knows whether the scope is a class
  or a namespace. It keeps the pre-ticket treatment (method-style
  `geo.nsq`, not exported). Definitions *inside* namespace blocks — the
  idiomatic implementation-file shape — carry the namespace correctly,
  including out-of-line method definitions there (`Circle::area` inside
  `namespace geo` stores `geo::Circle.area`).
- An anonymous namespace adds no qualifier (its members are referred to
  unqualified, so the enclosing path is the truest stored name), and its
  internal linkage is not modeled: exported keeps meaning "not static",
  exactly as before this ticket.
- A namespace alias (`namespace g = geo; g::nsq()`) does not resolve: the
  stored name is `geo::nsq`, and rewriting `g::` back needs the alias
  declaration tracked — the same scope-tracking family. The checklist's
  C++ aliased-call cell stays not-applicable and now says this itself.

### How every guard was proved able to fail (2026-08-14)

Six mutations, each applied to `c_cpp.rs`, run, and reverted; the restored
file byte-matches the pre-tamper snapshot.

| Mutation | What it printed |
|---|---|
| A: qualified callees not recorded (identifier-only filter) | both C++ cells — *no calls edge `function:uses_geo.cpp:use_geo -> function:geo.cpp:geo::nsq`*, *no calls edge `…use_deep -> …geo::inner::deep`*; scan.rs — the same-file `geo::inner::deep -> geo::nsq` edge assert fails |
| B: namespace path drops the outer prefix (innermost only) | nested-path cell only — *no calls edge `…use_deep -> function:geo.cpp:geo::inner::deep`*; the plain qualified-call cell **survived**, so the full-qualification criterion is asserted by its own cell |
| C1: namespaced functions stored bare | qualified-call cell — *the fixture no longer holds `function:geo.cpp:geo::nsq`, so this cell asserts nothing* (the preflight objecting); scan.rs ids assert fails showing `function:geo.cpp:nsq` back |
| C2: namespaced classes stored bare | scan.rs — *assertion failed: ids.contains(&"class:geo.hpp:geo::Disc")* |
| D: a namespace kills the export | scan.rs — *assertion failed: has_edge(&map, "exports", "file:geo.cpp", "function:geo.cpp:geo::nsq")* |
| E: the trailing-segment suppression dropped from `bind_includes` | scan.rs — *fabricated calls edge geo::twice -> bare.cpp:nsq: the resolver reached the decoy* |

### The magnify walk (inherited task, recorded 2026-08-14)

Walked at the projection level: the real `neighbourhoodOf` from
`dashboard/src/app/graph.ts`, run over the scanned `cppproj` map (vitest,
transient file, deleted after the walk; no dashboard code changed).
`file:uses_geo.cpp` and `file:geo.cpp` are related **only** through this
ticket's calls edges (`use_geo -> geo::nsq`, `use_deep -> geo::inner::deep`)
— no imports edge joins them, because `uses_geo.cpp` includes the header,
not the implementation.

What the reader actually sees:

- magnify on `file:uses_geo.cpp` draws `{uses_geo.cpp, geo.hpp}` —
  `geo.cpp`, the file every one of its calls lands in, is not drawn;
- magnify on `file:geo.cpp` draws `{geo.cpp, geo.hpp, bare.hpp}` —
  `uses_geo.cpp`, its only caller, is not drawn;
- meanwhile the detail panel for `use_geo` lists `calls → geo::nsq` and its
  narrative says "It reaches geo::nsq" — the panel names exactly the
  neighbour the lens declines to draw.

The two files do share `geo.hpp` in their neighbourhoods, so the reader
reaches one from the other in two magnify hops through the header — the
C-family's header indirection is what keeps the lens from being a dead end
here. The limitation stands as decided (recorded with its reason beside
`neighbourhoodOf`); this is its observed consequence on a C++ map, not a
defect filed against it.

### Suites (2026-08-14)

Existing-fixture non-regression: HEAD binary vs. this ticket's binary on
the pre-ticket file sets of `cproj`, `cppproj`, `polyglot` — maps
byte-identical (measured 10:47–10:52 IST). Suites, all green:
`cargo test --workspace` 271 passed; `--no-default-features` (sealed) 232
passed; `--no-default-features --features agent-cli` 259 passed;
`cargo fmt --all --check` clean; clippy `-D warnings` clean on all three
feature sets.

## Correction (2026-08-14, from the /crosscheck of this ticket's commit)

The crosscheck built both binaries and proved the cost paragraph above
("One new guard…") understates what the guard cost, in two ways. The
original paragraphs stand as written — a record is a record — and this
section says what was actually true and what the repair commit changed.

**The sibling call was a regression, not a non-goal.** The paragraph says
the unqualified sibling call — `nsq(k)` inside `geo::twice` —
"resolves to nothing this lap" and files resolving it under the parked
scope-tracking family. What was actually true: that call **resolved before
this ticket** (symbols were stored bare, so the same-file lookup matched —
correct per C++ name lookup) and **stopped resolving** when definitions
became `geo::nsq` while the callee text stayed `nsq`. And resolving it
never needed the parked scope tracking: the caller's own stored name
already carries its namespace path. The repair walks that path outward
within the caller's file — `geo::inner::f` calling `nsq` tries
`geo::inner::nsq`, then `geo::nsq`, then the bare name — which is C++'s
enclosing-namespace lookup order, from data the parser already had
(`qualify_bare_callees` in `c_cpp.rs`). `twice → geo::nsq` resolves again.
What genuinely stays in the parked family: `using namespace`, namespace
aliases, and enclosing-namespace lookup **across** files (a bare call whose
namespaced sibling is only declared in an included header stays
unresolved — following it needs a cross-file symbol table, and the walk
deliberately stops at the file boundary).

**The suppression was file-wide, not namespace-scoped.** The paragraph
reads as if the trailing-segment guard declined only the call C++ gives to
`geo::nsq`. In fact it suppressed the bare tail for **every** caller in the
file: the crosscheck's counter-case — a *global-scope* function calling a
global `nsq()` from an included header, in a file that also defines a
namespaced `…::nsq` — lost a legitimate edge, since global-scope lookup
cannot see `geo::nsq` unqualified and the header's global really does
answer. The repair makes the suppression caller-scoped by construction:
the walk rewrites a namespaced caller's bare callee to the qualified
same-file sibling (defined, so never offered to includes — the decoy stays
unreachable), while a global-scope caller's bare callee stays bare and is
offered. The tail-claiming in `bind_includes` is gone; the fixture
`mixed.cpp` holds both directions (`use_global → bare.cpp:nsq` now exists;
`alg::inner::f → alg::inner::nsq` pins the walk order), and the checklist's
C++ unqualified-call cell gains the `use_global` edge — the only cells
touched are C++'s.

### How the repair's guards were proved able to fail (2026-08-14)

Three mutations, each applied to `c_cpp.rs`, run, and reverted; the
restored file byte-matches the pre-tamper snapshot (sha256 checked).

| Mutation | What it printed |
|---|---|
| F1: the walk removed (`qualify_bare_callees` not called) | scan.rs — *fabricated calls edge geo::twice -> bare.cpp:nsq: the resolver reached the decoy* (mutation E's guard still fires), and both sibling-resolution tests red |
| F2: the walk inverted (outermost prefix first) | only the walk-order pin red — *no calls edge alg::inner::f -> alg::inner::nsq* while `twice → geo::nsq` stayed green, so the order is asserted by its own cell |
| F3: file-wide tail suppression restored in `bind_includes` | scan.rs — *no calls edge use_global -> bare.cpp:nsq*; conventions — the C++ unqualified-call cell red on the same edge; sibling resolution stayed green |

### Suites (2026-08-14, repair commit)

All green: `cargo test --workspace` 272 passed; `--no-default-features`
(sealed) 233 passed; `--no-default-features --features agent-cli` 260
passed; `cargo fmt --all --check` clean; clippy `-D warnings` clean on all
three feature sets. One pre-existing test edited, with reason: the two
closing asserts of `cpp_namespaced_symbols_carry_qualified_names_and_resolve_in_file`
pinned the regression itself ("resolve to nothing this lap"); they now
assert `twice → geo::nsq` while the decoy assert stands unweakened.
