# 18 — Resolve Rust `use <crate-name>::` paths to the crate behind them

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** Spec story 2 — "I want the map to capture relationships —
imports, calls, containment, exports — not just a file tree, so that I can
trace how components actually connect". For Rust it currently does not,
whenever a file refers to a crate in the scanned tree *by its name*:
`use codeatlas::map::MAP_CONTRACT_VERSION`, or, across a workspace,
`use atlas_engine::engine::run`. Both are dropped.

`crates/codeatlas/src/parsers/rust.rs:69` splits the specifier and matches on
its first segment: `crate` resolves against the enclosing `src/`, `self` and
`super` walk the module tree, and everything else — line 89's
`_ => return None` — is declined as an external crate. A crate that is
*in the scanned tree* is indistinguishable, at that point, from `serde`.

Minimal reproduction — a two-crate workspace, the idiomatic Rust layout:

```rust
// crates/atlas-engine/src/engine.rs
pub fn run() -> i32 { 42 }

// crates/atlas-cli/src/main.rs
use atlas_engine::engine::run;
```

```
$ codeatlas scan . && jq '[.edges[]|select(.kind=="imports")]' …
crates/atlas-engine/src/lib.rs -> crates/atlas-engine/src/engine.rs   # resolved
                       # crates/atlas-cli/src/main.rs -> …/engine.rs is missing
```

The two crates are islands. Nothing reports it.

**Why the suite never caught it:** `tests/fixtures/rustproj/` is a single
crate whose every path is `crate::`, `self::`, `super::` or `mod foo;` —
the four forms that do resolve. There is no fixture with two crates, and none
where a crate names itself. Ticket 04's acceptance criteria were met against
a layout that cannot exercise the gap. As in ticket 17, the fixture gap is as
much the defect as the resolver is.

**What it costs**, measured 2026-08-10:

- Against **CodeAtlas itself** the cost is small and worth stating honestly:
  two dropped statements, both in `crates/codeatlas/tests/share.rs`
  (`use codeatlas::map::…`, `use codeatlas::share::…`). All 8 integration-test
  files are orphaned in the import graph, but only those 2 would be
  reconnected by this fix — the other 6 drive the built binary through
  `assert_cmd` and genuinely import nothing.
- Against a **two-crate workspace** it is total: zero inter-crate edges, so
  every crate is an island and the tour, the layer projection and the diff
  blast radius all see a repo with no seams between its components.

That second case is the reason this is worth a ticket rather than a footnote.
CodeAtlas is a single-crate workspace, which is precisely why the defect looks
cheap here; multi-crate is the ordinary shape of a Rust project, and there the
map loses exactly the relationships a newcomer opens it to find. It is the
same failure ticket 17 fixed for TypeScript — a silently dropped specifier
form, no error, no warning, just absent edges — in a different parser.

Found on 2026-08-10 by `/harden`'s third walk, which probed each V1 language's
real-world import conventions rather than the conventions its fixtures happen
to use. TypeScript, Python (absolute, relative, `from . import`), Go
(module-path) and C/C++ (quoted vs angled) all resolved correctly; Rust
crate-name paths were the only failure.

**Blocked by:** none — 04 is done, this corrects it.

**Status:** done

- [x] `use <crate>::path` resolves to the file behind it when `<crate>` is a
      crate in the scanned tree, both for a crate naming itself and for a
      workspace sibling
- [x] Cargo's name normalisation is honoured: package `atlas-engine` is
      `atlas_engine` in source
- [x] External crates and `std`/`core`/`alloc` still resolve to nothing — a
      crate that is not in the scanned tree must never invent an edge
- [x] A scanned crate whose name collides with an external one resolves to the
      scanned crate, and the choice is documented as deliberate
- [x] Candidate order is fixed and documented, so resolution stays
      deterministic when several candidates exist
- [x] The fixtures gain a two-crate workspace and a crate-name self-reference
      — the conventions that were missing are covered by the suite, not just
      by this fix
- [x] Existing resolution is unregressed: `crate::`, `self::`, `super::`,
      `mod foo;`, and bare external paths
- [x] Referential integrity holds — no edge references a missing node
- [x] Scanning this repository connects `crates/codeatlas/tests/share.rs` to
      `crates/codeatlas/src/`, asserted by name rather than by a count

**How it landed.** `resolve_import`'s catch-all arm — the `_ => return None`
that treated every unrecognised first segment as external — became a lookup
against the crate roots in the scanned tree. A crate root is a directory
holding `src/lib.rs` or `src/main.rs`; its name comes from its `Cargo.toml`,
and names are compared after Cargo's `-`-to-`_` normalisation. The full
four-step candidate order is documented on the function.

Two behaviours emerged while building that the ticket had not anticipated:

- **A crate-name path falls back to the named crate's root module** when no
  submodule matches. `use atlas_tools::helper` names a function re-exported
  from `lib.rs`, not a `helper.rs` — `pub use` is the ordinary way a crate
  publishes anything, and without the fallback the commonest form of
  inter-crate import resolves to nothing. Deliberately *not* extended to
  `crate::`/`self::`/`super::`: an importer already inside the crate has no
  separate file to fall back to, and pointing it at its own root module would
  manufacture noise rather than recover an edge.
- **The nearest crate of a given name wins**, measured in shared leading path
  segments, ties broken by path order. One tree can hold several crates of one
  name — vendored copies, or the fixture repositories this repository keeps
  under `tests/fixtures/`. Without a rule the winner depends on hash iteration
  order; with the wrong rule a vendored tree resolves against the workspace
  that contains it.

Measured on this repository: Rust import edges **30 → 38**, including the two
the ticket was filed for, and `crates/codeatlas/tests/share.rs` is no longer
an orphan. All 38 were read individually to confirm none was invented.

**Decisions taken on the open questions:**

- **Where crate names come from — (a), not the recommended (b).** The
  recommendation was wrong, for a case the ticket missed: a crate at the scan
  root has *no directory name in any scanned path*, and that is the commonest
  Rust layout there is (`cargo new` then scan the crate). A directory-name
  heuristic cannot resolve `use root_lib::util` there at all. So the manifest
  is authoritative, with the directory name as fallback when no manifest was
  scanned. ADR-0006 is respected: the read is hand-rolled, about fifteen
  lines, and adds no dependency. There is precedent — the Go parser already
  reads `go.mod` from `root` during resolution. Fixture `rustroot` covers it.
- **Where the index lives — measured, and not needed.** Scanning this
  repository costs 0.06–0.07s against a 0.06s baseline, and the 3000-file
  synthetic repository is unchanged at 0.09s. The per-import manifest reads
  the ticket worried about are real but invisible at this scale, so the
  scan-level index that would change the parser trait stays unbuilt. If a
  large workspace ever shows the cost, the cheap fix before touching the trait
  is to match directory names first and read manifests only when that fails.
- **`extern crate` (2015 edition) — out.** The parser does not emit it as an
  import today, so there is nothing to resolve; supporting it means changing
  extraction as well as resolution, which is a different slice. No behaviour
  here depends on it.
- **Do the other four parsers have an analogous gap?** No — `/harden`'s third
  walk probed each V1 language's real conventions and Rust was the only
  failure. Evidence is in the spec's Verification section.

**Known limitation, recorded not fixed.** Every crate anywhere in the scanned
tree is a candidate name, so a repository that vendors or fixtures a crate
called `log` makes `log` resolvable from anywhere in it. Nearness makes the
choice sane rather than arbitrary, but it cannot express "this crate is not a
dependency of that one" — that would mean reading the dependency graph out of
every manifest, which is a much larger slice and buys little for a map whose
purpose is to show what is in the tree.
