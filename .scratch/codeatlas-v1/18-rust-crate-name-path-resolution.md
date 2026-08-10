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

**Status:** ready

- [ ] `use <crate>::path` resolves to the file behind it when `<crate>` is a
      crate in the scanned tree, both for a crate naming itself and for a
      workspace sibling
- [ ] Cargo's name normalisation is honoured: package `atlas-engine` is
      `atlas_engine` in source
- [ ] External crates and `std`/`core`/`alloc` still resolve to nothing — a
      crate that is not in the scanned tree must never invent an edge
- [ ] A scanned crate whose name collides with an external one resolves to the
      scanned crate, and the choice is documented as deliberate
- [ ] Candidate order is fixed and documented, so resolution stays
      deterministic when several candidates exist
- [ ] The fixtures gain a two-crate workspace and a crate-name self-reference
      — the conventions that were missing are covered by the suite, not just
      by this fix
- [ ] Existing resolution is unregressed: `crate::`, `self::`, `super::`,
      `mod foo;`, and bare external paths
- [ ] Referential integrity holds — no edge references a missing node
- [ ] Scanning this repository connects `crates/codeatlas/tests/share.rs` to
      `crates/codeatlas/src/`, asserted by name rather than by a count

**Worth deciding while in here:**

- **Where crate names come from.** Two options, and the cheaper one looks
  right. (a) Parse each `Cargo.toml` for `[package] name` — exact, but the
  crate has no TOML dependency today and adding one widens the audit surface
  ADR-0006 exists to keep narrow, so it would want a hand-rolled read rather
  than a new crate. (b) Infer structurally: a directory holding `src/lib.rs`
  or `src/main.rs` is a crate root and its directory name, normalised, is the
  crate name. (b) needs no new dependency and no file reads, and it is correct
  on this repo (`crates/codeatlas` → `codeatlas`) and on the reproduction
  above. It is wrong only when a package's `name` differs from its directory,
  which is legal but uncommon. Recommend (b), and say so in the doc comment.
- **Where the index lives.** `resolve_import` is stateless and per-file; it
  receives the full scanned `files` set and `root`, so it *can* derive crate
  roots itself, but it would redo that work for every import. If that shows up
  as measurable cost, the honest fix is a scan-level index, which changes the
  parser trait — a bigger slice than ticket 17 was. Measure before widening.
- **`extern crate` (2015 edition).** Rare in edition-2018-and-later code and
  not emitted by the parser today. Probably out; record the decision either
  way.
- **Do the other four parsers have an analogous gap?** This walk probed them
  and found none, so unlike ticket 17 there is no suspicion to carry forward.
  The evidence is in the spec's Verification section.
