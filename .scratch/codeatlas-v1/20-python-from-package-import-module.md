# 20 — `from package import module` never reaches the module

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** Spec story 2 — "I want the map to capture relationships —
imports, calls, containment, exports — not just a file tree, so that I can
trace how components actually connect". For one of Python's most common import
forms it does not: `from pkg import util`, where `util` is a module file,
never produces an edge to `pkg/util.py`.

`crates/codeatlas/src/parsers/python.rs` resolves the *specifier* and stops.
For `from pkg import util` the specifier is `pkg`, and `resolve_module` tries
`pkg.py` then `pkg/__init__.py`; the imported name `util` is only ever treated
as a symbol, never tried as `pkg/util.py`. So the outcome depends on a detail
of the package that has nothing to do with the import:

```
                                    with pkg/__init__.py   without it
from pkg import util   (a module)   -> pkg/__init__.py      -> no edge at all
from pkg.util import helper         -> pkg/util.py          -> pkg/util.py
import pkg.util                     -> pkg/util.py          -> pkg/util.py
import ns.sub.util as u             -> ns/sub/util.py       -> ns/sub/util.py
```

Measured 2026-08-10 on two identical trees differing only by the presence of
`__init__.py`.

Neither column is right, and they are wrong in different ways:

- **With `__init__.py`** the edge exists but points at the package
  initialiser. It is not a *false* edge — `from pkg import util` really does
  execute `pkg/__init__.py` — but it is the wrong answer to the question the
  map is for. The dependency on `pkg/util.py` is invisible, so a reader
  tracing who uses that module sees nothing.
- **Without `__init__.py`** — a PEP 420 namespace package, legal and ordinary
  since Python 3.3 — there is no edge at all, and a file importing only that
  way is an orphan in the graph.

**Why the suite never caught it:** `tests/fixtures/pyproj/` *does* contain
`from pkg import api` — but `api` there is a function re-exported from
`pkg/__init__.py`, so resolving to `pkg/__init__.py` is the correct answer,
and `tests/scan.rs:1046` asserts exactly that. The form is covered; the case
where the imported name is a **module** is not, and no fixture has a namespace
package at all. As with tickets 17 and 18, the fixture gap is as much the
defect as the resolver is — and this one is subtler, because the fixture looks
like it covers the form.

Found on 2026-08-10 by `/harden`'s fourth walk, in the second round of
per-language import-convention probes. The first round (walk three) tested
`from .util import helper`, `from . import util`, `import pkg.util` and
`from pkg.util import helper` — all of which resolve — and missed this one.

**Blocked by:** none — 04 is done, this corrects it.

**Status:** done

- [x] `from pkg import util` resolves to `pkg/util.py` when that module is in
      the scanned set, both with and without `pkg/__init__.py`
- [x] The existing behaviour is preserved where it is right: when the imported
      name is a symbol re-exported from `pkg/__init__.py` and no `pkg/util.py`
      exists, the edge still lands on `pkg/__init__.py`
      (`tests/scan.rs:1046` must keep passing unchanged)
- [x] `from . import util` likewise reaches `util.py` rather than stopping at
      the package initialiser
- [x] Candidate order is fixed and documented — module before package, or
      whichever way round, but stated, so a name that could be both resolves
      the same way every run
- [x] A name that is neither a module nor a resolvable package still produces
      no edge; external and stdlib imports are unaffected
- [x] The fixtures gain a `from package import module` case and a namespace
      package (a directory of modules with no `__init__.py`)
- [x] Existing resolution is unregressed: relative imports, dotted absolute
      imports, aliased imports, script-style siblings
- [x] Referential integrity holds — no edge references a missing node

**How it landed.** Resolution had no way to see a bound name — the parser
trait maps one specifier to one file, and the specifier is exactly the part
of `from pkg import util` that does *not* say where the edge goes. The trait
gained `resolve_name_as_module`, which resolves a bound name as a module in
its own right and returns `None` when it is not one. `None` is also the
default, and it means the same thing either way: the name lands wherever the
specifier does. Python is the only parser that overrides it.

`scan.rs` now asks per name and falls back to the specifier only for the
names that are not modules, which is what makes a mixed statement land in two
files while a pure module import stays out of the package initialiser.

Verified against a new `pypkgs/` fixture with one exact-set assertion over
every `imports` edge, so the absences are pinned as hard as the presences —
`uses_module.py` reaching `pkg/util.py` proves the fix, and its *not*
reaching `pkg/__init__.py` proves the module-alone rule. Two mutations were
run to prove the test can fail: drop the relative-import special case in
`submodule_of` and both `from . import` edges break; always emit the
specifier target and four spurious initialiser edges appear.

**Decisions taken on the open questions:**

- **One edge or two — one, the module alone**, as the ticket leaned. The
  deciding argument was consistency rather than sparseness: this resolver
  already answers `from pkg.util import helper` with `pkg/util.py` alone and
  never records the package chain it walked through. Two edges here would
  make the same dependency look different depending on which of two
  equivalent import forms the author happened to type.
- **`from pkg import a, b, c` — the extractor already yields one
  `ImportedName` per bound name**, so nothing had to change there; it was
  resolution that collapsed them. `uses_both.py` (`from pkg import api, util`)
  is the fixture, and it lands on `pkg/__init__.py` *and* `pkg/util.py`.
- **Ambiguity — the module wins**, pinned by `pkg/shadow.py` existing
  alongside a `shadow` symbol in `pkg/__init__.py`. This is the one case
  where the two candidate orders disagree, so it is the fixture that gives
  the documented order teeth.
- **The other parsers — confirmed, not assumed, and Python-only stands.**
  Rust is the only other language with the form (`use a::b` where `b` may be
  a module or an item), so it got a probe rather than a reading: `use
  crate::a::b`, `use crate::a::deep::leaf` and `use self::a::b::helper` all
  land on the right file. For the rest the form does not exist in the
  grammar — Go and C/C++ specifiers name the package or file outright with no
  separate bound name, and TypeScript's braced names are always symbols
  inside the module the specifier already named.

**A first attempt was measured and thrown away.** The obvious shape — a
`resolve_imported_name` that defaults to resolving the specifier, called once
per bound name — is correct but costs the C family badly, because
`bind_includes` offers every unresolved callee to every include as a bound
name. On a synthetic 200-file C repo (8 includes × 80 callees each) it took
the scan from 77ms to 108ms for byte-identical output: 640 full resolutions
per file where 8 would do. Inverting it so the default does *nothing* and the
specifier is resolved at most once per statement puts it back at 78ms. On a
synthetic 300-file Python repo the fix is free (22ms either way) and finds
2400 import edges that were previously invisible; the self-scan is unchanged
at 78ms.

**What `/crosscheck` found, and what changed because of it.** Both axes
landed on the same defect independently, and it was a real one.

- **The module-first rule was dropping a `calls` edge, and the fixture was
  written so it could not notice.** `from pkg import shadow`, where
  `pkg/__init__.py` defines a `shadow` symbol beside `pkg/shadow.py`, bound
  the name to the module and nothing else — so a bare `shadow()` resolved to
  nothing, where before the change it reached
  `function:pkg/__init__.py:shadow`. Reproduced against both binaries before
  believing it. The two rules genuinely pull opposite ways: an *edge* should
  point at the module a reader would open, a *call* at the function that
  actually runs, and a module can never answer a bare call. `bindings` was
  already an ordered candidate list with a first-that-works trial — built for
  C-family includes — so the fix is to push both, specifier last, because
  candidates are tried last-first. Import edges are unaffected; they are
  computed separately. `uses_shadow.py` now calls `shadow()` instead of
  merely naming it, and the assertion was mutation-tested: bind only the
  module and the test fails.
- **A dead intra-doc link** in the module header, left pointing at the
  discarded first attempt's name. Fixed, and `cargo doc
  --document-private-items` now reports zero unresolved links — which also
  turned up the same defect one module over in `rust.rs:13` from ticket 18,
  fixed in passing rather than left as a known-broken twin.
- **AC7 had no script-style case through the new path.** Added
  `scripts/tool.py` importing `from local import render`, which resolves only
  by trying the name as a module beside the importer — there is no root-level
  anchor and no `__init__.py`.
- **The Rust confirmation rested on a probe that was thrown away.** Made
  durable instead: the `rustroot` fixture gained `src/deep/leaf.rs` and a
  `use crate::deep::leaf`, so the claim that Rust already handles
  name-as-module is now a standing test rather than a paragraph.
- **Declined: sharing one helper between `import_targets` and the binding
  loop.** The reviewer was right that the two read alike and that the bug
  lived in their divergence — but the divergence is the point, and it is now
  the thing the code has to say out loud. Imports treat the specifier as a
  *fallback*; calls treat it as an *additional candidate*. A shared helper
  would have to be parameterised by which, and the parameter would carry all
  the meaning. Both sites say why in a comment instead.

**Noticed in passing, out of scope:** `lib.rs:100` prints `mapped {} files`
with `graph.nodes.len()`, which counts symbol nodes too — the 208-file C
probe reports "mapped 408 files". Cosmetic and user-facing; not this ticket.

**Worth deciding while in here:**

- **One edge or two?** `from pkg import util` genuinely executes both
  `pkg/__init__.py` and `pkg/util.py`. Emitting both is the truthful reading
  and costs nothing structurally; emitting only the module is the more useful
  one and keeps the graph sparser. Lean toward the module alone — the package
  initialiser is rarely what a reader wants to trace to — but say which and
  why in the doc comment.
- **`from pkg import a, b, c`.** One statement, several names, each possibly a
  module. The extractor already yields one `Import` per bound name for Rust
  (`expand_use`); check whether the Python side does the same or collapses
  them, because a shared specifier with several names needs each name resolved
  separately.
- **Ambiguity.** If both `pkg/util.py` and a `util` symbol in
  `pkg/__init__.py` exist, the module wins under the rule above. Worth a
  fixture, since it is the case where the two candidate orders disagree.
- **Do the other parsers have the analogous gap?** The question is now
  specific: "the imported *name* may itself be a file". It does not arise for
  Rust (ticket 18 already falls back through the crate root), Go (packages are
  directories), or C/C++ (includes name files outright). For TypeScript,
  `import { x } from "./y"` always names the module in the specifier, so the
  form does not exist. This looks Python-only; confirm rather than assume.
