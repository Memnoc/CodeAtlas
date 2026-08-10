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

**Status:** ready

- [ ] `from pkg import util` resolves to `pkg/util.py` when that module is in
      the scanned set, both with and without `pkg/__init__.py`
- [ ] The existing behaviour is preserved where it is right: when the imported
      name is a symbol re-exported from `pkg/__init__.py` and no `pkg/util.py`
      exists, the edge still lands on `pkg/__init__.py`
      (`tests/scan.rs:1046` must keep passing unchanged)
- [ ] `from . import util` likewise reaches `util.py` rather than stopping at
      the package initialiser
- [ ] Candidate order is fixed and documented — module before package, or
      whichever way round, but stated, so a name that could be both resolves
      the same way every run
- [ ] A name that is neither a module nor a resolvable package still produces
      no edge; external and stdlib imports are unaffected
- [ ] The fixtures gain a `from package import module` case and a namespace
      package (a directory of modules with no `__init__.py`)
- [ ] Existing resolution is unregressed: relative imports, dotted absolute
      imports, aliased imports, script-style siblings
- [ ] Referential integrity holds — no edge references a missing node

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
