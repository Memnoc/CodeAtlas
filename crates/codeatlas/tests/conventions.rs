//! Story 2's convention checklist, as a fixture table.
//!
//! The story used to say "every import convention in six languages", which has
//! no bottom: three consecutive `/harden` walks each found a seventh
//! convention, and each was closed by widening a per-language probe. The spec
//! closed the story on 2026-08-11 by naming a finite checklist — seven import
//! forms, four call forms, three non-edges — and this file is that checklist
//! with one cell per convention per language.
//!
//! [`CHECKLIST`] is the table. Every one of the 6 × 14 cells is present, and
//! each says one of four things:
//!
//! - **[`Verdict::Holds`]** — this fixture, and these edges must be in its map.
//! - **[`Verdict::NotApplicable`]** — the language has no such convention, and
//!   why. A missing cell and an inapplicable cell must not look alike, which
//!   is the whole reason the table is exhaustive rather than sparse.
//! - **[`Verdict::Filed`]** — the row genuinely fails, and the named ticket
//!   owns it. The runner asserts the gap is *still there*, so closing the
//!   ticket fails this file rather than leaving the table quietly stale.
//! - **[`Verdict::Vacuous`]** — the convention exists here and the cell cannot
//!   currently fail, because the language support that would let it fail is
//!   not written yet. The expectation is still asserted, but the cell does not
//!   render as a pass: a claim nothing can falsify must not sit in the table
//!   looking like one that can. The runner asserts the *reason* it is vacuous
//!   is still true, so the ticket that removes the reason forces the cell to
//!   be re-proved rather than letting it drift into a silent pass.
//!
//! One test per convention, so a failure names the row. Assertions are at
//! seam 1 — run the binary over a committed fixture, read the emitted map —
//! and no cell reaches into the pipeline.
//!
//! **On the three non-edge rows.** They matter as much as the positive ones,
//! because they are where a resolver invents edges: ticket 21 shipped a
//! fabricated-edge bug that seven mutations missed, because no fixture had a
//! decoy for the resolver to be tempted by. So a non-edge cell never asserts
//! that some unrelated count is unchanged. It names a **decoy** — a node that
//! really is in the map, that a naive resolver would have connected — and
//! asserts the edge to it is absent. [`Cell::preflight`] fails if the decoy
//! stops existing, so the guard cannot rot into a tautology.

mod common;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use common::{has_edge, materialize, read_map};
use serde_json::Value;

// ---------------------------------------------------------------------------
// The two axes
// ---------------------------------------------------------------------------

/// The six V1 languages (spec, Implementation Decisions: "TypeScript/
/// JavaScript, Rust, Python, Go, C, and C++"). Markdown is scanned too, but it
/// is a document format with links rather than a language with imports, and
/// the story does not count it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Language {
    TypeScript,
    Rust,
    Python,
    Go,
    C,
    Cpp,
}

const LANGUAGES: [Language; 6] = [
    Language::TypeScript,
    Language::Rust,
    Language::Python,
    Language::Go,
    Language::C,
    Language::Cpp,
];

impl Language {
    fn label(self) -> &'static str {
        match self {
            Language::TypeScript => "TypeScript",
            Language::Rust => "Rust",
            Language::Python => "Python",
            Language::Go => "Go",
            Language::C => "C",
            Language::Cpp => "C++",
        }
    }
}

/// The fourteen conventions, in the spec's own order and wording. Adding one
/// is a spec change, not a discovery this file may make on its own.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Convention {
    PlainModuleImport,
    NamedImport,
    AliasedImport,
    WholeModuleImport,
    RelativeImport,
    PackageOrIndexImport,
    HeaderSourcePair,
    UnqualifiedCall,
    QualifiedCall,
    QualifiedCallThroughAlias,
    QualifiedCallThroughNestedPath,
    ReceiverIsAValue,
    CallIntoOutsidePackage,
    ImportResolvingNowhere,
}

const CONVENTIONS: [Convention; 14] = [
    Convention::PlainModuleImport,
    Convention::NamedImport,
    Convention::AliasedImport,
    Convention::WholeModuleImport,
    Convention::RelativeImport,
    Convention::PackageOrIndexImport,
    Convention::HeaderSourcePair,
    Convention::UnqualifiedCall,
    Convention::QualifiedCall,
    Convention::QualifiedCallThroughAlias,
    Convention::QualifiedCallThroughNestedPath,
    Convention::ReceiverIsAValue,
    Convention::CallIntoOutsidePackage,
    Convention::ImportResolvingNowhere,
];

impl Convention {
    fn label(self) -> &'static str {
        match self {
            Convention::PlainModuleImport => "a plain module import",
            Convention::NamedImport => "a named/member import",
            Convention::AliasedImport => "an aliased import",
            Convention::WholeModuleImport => "a namespace or whole-module import",
            Convention::RelativeImport => "a relative import",
            Convention::PackageOrIndexImport => {
                "a package-or-directory import through an initialiser or index file"
            }
            Convention::HeaderSourcePair => "a header/source pairing (C/C++ only)",
            Convention::UnqualifiedCall => "an unqualified call to an imported name",
            Convention::QualifiedCall => "a qualified call through an imported module",
            Convention::QualifiedCallThroughAlias => "a qualified call through an aliased module",
            Convention::QualifiedCallThroughNestedPath => {
                "a qualified call through a nested module path"
            }
            Convention::ReceiverIsAValue => {
                "NON-EDGE: a call whose receiver is a value rather than a module"
            }
            Convention::CallIntoOutsidePackage => {
                "NON-EDGE: a call into a package outside the repository"
            }
            Convention::ImportResolvingNowhere => {
                "NON-EDGE: an import resolving to no file in the repository"
            }
        }
    }
}

// ---------------------------------------------------------------------------
// A cell
// ---------------------------------------------------------------------------

struct Cell {
    language: Language,
    convention: Convention,
    /// The form as source writes it, so a reader can check the fixture against
    /// the row without running anything.
    form: &'static str,
    verdict: Verdict,
}

enum Verdict {
    /// The row passes: scan `fixture` and `expect` must hold of its map.
    Holds {
        fixture: &'static str,
        expect: Expect,
    },
    /// The language has no such convention, and here is why.
    NotApplicable { because: &'static str },
    /// The row fails today and `ticket` owns it. `want` is what the cell will
    /// assert once the ticket lands; until then the runner asserts `want` does
    /// *not* hold, so a fix cannot land without this table being updated.
    Filed {
        ticket: &'static str,
        fixture: &'static str,
        want: Expect,
    },
    /// The convention exists in this language, `guard` really does hold of the
    /// map, and **nothing could make it fail**. Either no mutation of the
    /// pipeline can violate the guard, or the only evidence the row has is
    /// another cell's — so a reader who saw `pass` here would be reading a
    /// claim beside sixteen real ones that nobody actually makes.
    ///
    /// `vacuous_until` is what makes it so: an expectation that does *not*
    /// hold today and whose arrival is exactly what gives the row something to
    /// say. The runner asserts `guard` holds **and** that `vacuous_until`
    /// still does not, so the day `ticket` lands this cell fails and has to be
    /// re-proved by tamper instead of sliding quietly into a pass.
    Vacuous {
        ticket: &'static str,
        fixture: &'static str,
        guard: Expect,
        vacuous_until: Expect,
        because: &'static str,
    },
}

enum Expect {
    /// Every one of these `(kind, source, target)` edges is in the map.
    Edges(&'static [(&'static str, &'static str, &'static str)]),
    /// No `kind` edge joins `source` to `decoy` — where `decoy` is a node that
    /// really is in the map and that a resolver matching on names alone would
    /// have connected.
    NoEdge {
        kind: &'static str,
        source: &'static str,
        decoy: &'static str,
    },
    /// The complete set of `kind` edges leaving `source`, so the absences are
    /// pinned as hard as the presences: a wrongly resolved specifier does not
    /// point at something conveniently named, it points at some real file.
    /// `decoys` are the files a resolver searching by name rather than by path
    /// would have reached; they are preflighted like any other node, so the
    /// set stays a temptation rather than becoming a formality.
    EdgeSetFrom {
        kind: &'static str,
        source: &'static str,
        targets: &'static [&'static str],
        decoys: &'static [&'static str],
    },
}

impl Expect {
    /// Every node id this expectation names.
    fn nodes(&self) -> Vec<&'static str> {
        match self {
            Expect::Edges(edges) => edges
                .iter()
                .flat_map(|(_, source, target)| [*source, *target])
                .collect(),
            Expect::NoEdge { source, decoy, .. } => vec![source, decoy],
            Expect::EdgeSetFrom {
                source,
                targets,
                decoys,
                ..
            } => std::iter::once(*source)
                .chain(targets.iter().copied())
                .chain(decoys.iter().copied())
                .collect(),
        }
    }
}

// ---------------------------------------------------------------------------
// The table
// ---------------------------------------------------------------------------

const CHECKLIST: &[Cell] = &[
    // -- a plain module import ---------------------------------------------
    Cell {
        language: Language::TypeScript,
        convention: Convention::PlainModuleImport,
        form: "import \"./side\";",
        verdict: Verdict::Holds {
            fixture: "simple",
            expect: Expect::Edges(&[("imports", "file:src/plain.ts", "file:src/side.ts")]),
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::PlainModuleImport,
        form: "pub mod leaf;",
        verdict: Verdict::Holds {
            fixture: "rustroot",
            expect: Expect::Edges(&[("imports", "file:src/deep/mod.rs", "file:src/deep/leaf.rs")]),
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::PlainModuleImport,
        form: "import pkg.util",
        verdict: Verdict::Holds {
            fixture: "pypkgs",
            expect: Expect::Edges(&[("imports", "file:uses_dotted.py", "file:pkg/util.py")]),
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::PlainModuleImport,
        form: "import \"example.com/demo/util\"",
        verdict: Verdict::Holds {
            fixture: "goproj",
            expect: Expect::Edges(&[("imports", "file:main.go", "file:util/util.go")]),
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::PlainModuleImport,
        form: "#include \"util.h\"",
        verdict: Verdict::Holds {
            fixture: "cproj",
            expect: Expect::Edges(&[("imports", "file:main.c", "file:util.h")]),
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::PlainModuleImport,
        form: "#include \"geometry.hpp\"",
        verdict: Verdict::Holds {
            fixture: "cppproj",
            expect: Expect::Edges(&[("imports", "file:main.cpp", "file:geometry.hpp")]),
        },
    },
    // -- a named/member import ---------------------------------------------
    Cell {
        language: Language::TypeScript,
        convention: Convention::NamedImport,
        form: "import { greet } from \"./util\";",
        verdict: Verdict::Holds {
            fixture: "simple",
            expect: Expect::Edges(&[
                ("imports", "file:src/main.ts", "file:src/util.ts"),
                (
                    "calls",
                    "function:src/main.ts:main",
                    "function:src/util.ts:greet",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::NamedImport,
        form: "use root_lib::util::helper; helper()",
        verdict: Verdict::Holds {
            fixture: "rustroot",
            expect: Expect::Edges(&[
                ("imports", "file:tests/it.rs", "file:src/util.rs"),
                // The file edge alone does not test "named": `use
                // root_lib::util;` produces the identical one. Only the member
                // form *binds* `helper`, so the bare call is the assertion
                // that tells the two apart — the same argument
                // `rust_qualified_calls_resolve_through_the_module_that_holds_them`
                // records for `from_bound_name`.
                (
                    "calls",
                    "function:tests/it.rs:helps",
                    "function:src/util.rs:helper",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::NamedImport,
        form: "from utils import shout",
        verdict: Verdict::Holds {
            fixture: "pyproj",
            expect: Expect::Edges(&[
                ("imports", "file:app.py", "file:utils.py"),
                ("calls", "function:app.py:main", "function:utils.py:shout"),
            ]),
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::NamedImport,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "an import path names a package and nothing inside it; Go has no \
                      member-import form. `import . \"p\"` binds every exported name at \
                      once rather than selecting one, which is the unqualified-call row \
                      rather than this one",
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::NamedImport,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "an #include names a file; the preprocessor has no way to select \
                      names out of it",
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::NamedImport,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "an #include names a file; the preprocessor has no way to select \
                      names out of it",
        },
    },
    // -- an aliased import -------------------------------------------------
    Cell {
        language: Language::TypeScript,
        convention: Convention::AliasedImport,
        form: "import { greet as hello } from \"./util\";",
        verdict: Verdict::Holds {
            fixture: "simple",
            expect: Expect::Edges(&[
                ("imports", "file:src/renamed.ts", "file:src/util.ts"),
                (
                    "calls",
                    "function:src/renamed.ts:viaRenamed",
                    "function:src/util.ts:greet",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::AliasedImport,
        form: "use crate::util as u;",
        verdict: Verdict::Holds {
            fixture: "rustroot",
            expect: Expect::Edges(&[
                ("imports", "file:src/deep/leaf.rs", "file:src/util.rs"),
                (
                    "calls",
                    "function:src/deep/leaf.rs:via_alias",
                    "function:src/util.rs:helper",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::AliasedImport,
        form: "from pkg import util as u",
        verdict: Verdict::Holds {
            fixture: "pypkgs",
            expect: Expect::Edges(&[
                ("imports", "file:uses_alias.py", "file:pkg/util.py"),
                (
                    "calls",
                    "function:uses_alias.py:aliased",
                    "function:pkg/util.py:helper",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::AliasedImport,
        form: "import u \"example.com/demo/util\"",
        verdict: Verdict::Holds {
            fixture: "goproj",
            expect: Expect::Edges(&[("imports", "file:alias.go", "file:util/util.go")]),
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::AliasedImport,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "an #include binds no name, so there is nothing to rename",
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::AliasedImport,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "an #include binds no name, so there is nothing to rename \
                      (a namespace alias renames a namespace, not a file)",
        },
    },
    // -- a namespace or whole-module import --------------------------------
    Cell {
        language: Language::TypeScript,
        convention: Convention::WholeModuleImport,
        form: "import * as util from \"./util\";",
        verdict: Verdict::Holds {
            fixture: "simple",
            expect: Expect::Edges(&[
                ("imports", "file:src/namespace.ts", "file:src/util.ts"),
                (
                    "calls",
                    "function:src/namespace.ts:viaNamespace",
                    "function:src/util.ts:greet",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::WholeModuleImport,
        form: "use crate::deep::leaf;",
        verdict: Verdict::Holds {
            fixture: "rustroot",
            expect: Expect::Edges(&[
                ("imports", "file:src/lib.rs", "file:src/deep/leaf.rs"),
                (
                    "calls",
                    "function:src/lib.rs:tip",
                    "function:src/deep/leaf.rs:tip",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::WholeModuleImport,
        form: "from pkg import util",
        verdict: Verdict::Holds {
            fixture: "pypkgs",
            expect: Expect::Edges(&[
                ("imports", "file:uses_module.py", "file:pkg/util.py"),
                (
                    "calls",
                    "function:uses_module.py:run",
                    "function:pkg/util.py:helper",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::WholeModuleImport,
        form: "import \"example.com/demo/util\" — the same statement as the plain form",
        verdict: Verdict::Vacuous {
            ticket: "37",
            fixture: "goproj",
            guard: Expect::Edges(&[("imports", "file:main.go", "file:util/util.go")]),
            vacuous_until: Expect::Edges(&[(
                "calls",
                "function:main.go:main",
                "function:util/util.go:Format",
            )]),
            because: "Go has one import statement and it is both forms at once: the \
                      statement that makes the file edge is the statement that binds \
                      `util` as a qualifier. The edge is therefore byte-for-byte the \
                      plain-import cell's, so no mutation can fail one and not the other \
                      and the table must not claim two independent passes for it. The \
                      only evidence that tells the rows apart is the qualifier binding, \
                      which is a qualified call, which is ticket 37",
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::WholeModuleImport,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "an #include binds no qualifier — the header's contents enter the \
                      current scope outright, which is the plain-import row",
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::WholeModuleImport,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "an #include binds no qualifier; a C++ `::` names a namespace or a \
                      class, never a translation unit",
        },
    },
    // -- a relative import -------------------------------------------------
    Cell {
        language: Language::TypeScript,
        convention: Convention::RelativeImport,
        form: "import { greet } from \"../util\";",
        verdict: Verdict::Holds {
            fixture: "simple",
            expect: Expect::Edges(&[
                ("imports", "file:src/lib/index.ts", "file:src/util.ts"),
                (
                    "calls",
                    "function:src/lib/index.ts:helper",
                    "function:src/util.ts:greet",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::RelativeImport,
        form: "use super::util::helper;",
        verdict: Verdict::Holds {
            fixture: "rustroot",
            expect: Expect::Edges(&[
                ("imports", "file:src/deep/mod.rs", "file:src/util.rs"),
                (
                    "calls",
                    "function:src/deep/mod.rs:relative",
                    "function:src/util.rs:helper",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::RelativeImport,
        form: "from . import util",
        verdict: Verdict::Holds {
            fixture: "pypkgs",
            expect: Expect::Edges(&[
                ("imports", "file:pkg/inside.py", "file:pkg/util.py"),
                (
                    "calls",
                    "function:pkg/inside.py:use",
                    "function:pkg/util.py:helper",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::RelativeImport,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "a Go import path is always a full module path; `import \"./util\"` \
                      is illegal inside a module",
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::RelativeImport,
        form: "#include \"../util.h\"",
        verdict: Verdict::Holds {
            fixture: "cproj",
            expect: Expect::Edges(&[("imports", "file:app/app.h", "file:util.h")]),
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::RelativeImport,
        form: "#include \"../geometry.hpp\"",
        verdict: Verdict::Holds {
            fixture: "cppproj",
            expect: Expect::Edges(&[
                ("imports", "file:detail/inner.cpp", "file:geometry.hpp"),
                (
                    "calls",
                    "function:detail/inner.cpp:twice_tau",
                    "function:geometry.cpp:tau",
                ),
            ]),
        },
    },
    // -- a package-or-directory import through an initialiser or index -----
    Cell {
        language: Language::TypeScript,
        convention: Convention::PackageOrIndexImport,
        form: "import { helper } from \"./lib\";  // lib/index.ts",
        verdict: Verdict::Holds {
            fixture: "simple",
            expect: Expect::Edges(&[("imports", "file:src/app.ts", "file:src/lib/index.ts")]),
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::PackageOrIndexImport,
        form: "pub mod deep;  // src/deep/mod.rs",
        verdict: Verdict::Holds {
            fixture: "rustroot",
            expect: Expect::Edges(&[("imports", "file:src/lib.rs", "file:src/deep/mod.rs")]),
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::PackageOrIndexImport,
        form: "from pkg import api  // pkg/__init__.py",
        verdict: Verdict::Holds {
            fixture: "pypkgs",
            expect: Expect::Edges(&[
                ("imports", "file:uses_symbol.py", "file:pkg/__init__.py"),
                (
                    "calls",
                    "function:uses_symbol.py:boot",
                    "function:pkg/__init__.py:api",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::PackageOrIndexImport,
        // A Go import names a *directory*, and `util/` holds two files. The
        // edge landing on `util/util.go` alone is what says the directory
        // resolved through its anchor rather than through whichever file the
        // resolver happened to see first.
        form: "import \"example.com/demo/util\"  // util/ holds util.go and extra.go",
        verdict: Verdict::Holds {
            fixture: "goproj",
            expect: Expect::EdgeSetFrom {
                kind: "imports",
                source: "file:main.go",
                targets: &["file:util/util.go"],
                decoys: &["file:util/extra.go"],
            },
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::PackageOrIndexImport,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "an #include names a file outright; C has no package, directory or \
                      initialiser import",
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::PackageOrIndexImport,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "an #include names a file outright; C++ has no package, directory or \
                      initialiser import",
        },
    },
    // -- a header/source pairing (C/C++ only) ------------------------------
    Cell {
        language: Language::TypeScript,
        convention: Convention::HeaderSourcePair,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "the spec scopes this row to C/C++; a TypeScript module is its own \
                      declaration",
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::HeaderSourcePair,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "the spec scopes this row to C/C++; a Rust module is its own \
                      declaration",
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::HeaderSourcePair,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "the spec scopes this row to C/C++; a Python module is its own \
                      declaration",
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::HeaderSourcePair,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "the spec scopes this row to C/C++; a Go package is its own \
                      declaration",
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::HeaderSourcePair,
        form: "util.c includes util.h; main.c calls util_greet() through it",
        verdict: Verdict::Holds {
            fixture: "cproj",
            expect: Expect::Edges(&[
                ("imports", "file:util.c", "file:util.h"),
                (
                    "calls",
                    "function:main.c:main",
                    "function:util.c:util_greet",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::HeaderSourcePair,
        form: "report.cc includes report.hpp; main.cpp calls report() through it",
        verdict: Verdict::Holds {
            fixture: "cppproj",
            expect: Expect::Edges(&[
                ("imports", "file:report.cc", "file:report.hpp"),
                (
                    "calls",
                    "function:main.cpp:main",
                    "function:report.cc:report",
                ),
            ]),
        },
    },
    // -- an unqualified call to an imported name ---------------------------
    Cell {
        language: Language::TypeScript,
        convention: Convention::UnqualifiedCall,
        form: "import { greet } from \"./util\"; greet(\"atlas\")",
        verdict: Verdict::Holds {
            fixture: "simple",
            expect: Expect::Edges(&[(
                "calls",
                "function:src/main.ts:main",
                "function:src/util.ts:greet",
            )]),
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::UnqualifiedCall,
        form: "use util::helper; helper()",
        verdict: Verdict::Holds {
            fixture: "rustroot",
            expect: Expect::Edges(&[(
                "calls",
                "function:src/lib.rs:from_bound_name",
                "function:src/util.rs:helper",
            )]),
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::UnqualifiedCall,
        form: "from utils import shout; shout(\"atlas\")",
        verdict: Verdict::Holds {
            fixture: "pyproj",
            expect: Expect::Edges(&[("calls", "function:app.py:main", "function:utils.py:shout")]),
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::UnqualifiedCall,
        // Not inapplicable, though it read that way until /crosscheck: a dot
        // import binds every exported name of the package unqualified and is
        // legal Go, so `Format("dot")` in `goproj/dot.go` really is an
        // unqualified call to an imported name. The parser binds no name for
        // any Go import, so it resolves to nothing — the same root cause as
        // the two qualified-call cells, and ticket 37 already carries dot
        // imports as a criterion.
        form: "import . \"example.com/demo/util\"; Format(\"dot\")",
        verdict: Verdict::Filed {
            ticket: "37",
            fixture: "goproj",
            want: Expect::Edges(&[(
                "calls",
                "function:dot.go:viaDot",
                "function:util/util.go:Format",
            )]),
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::UnqualifiedCall,
        form: "#include \"util.h\"; util_greet(\"app\")",
        verdict: Verdict::Holds {
            fixture: "cproj",
            expect: Expect::Edges(&[(
                "calls",
                "function:app/app.c:app_run",
                "function:util.c:util_greet",
            )]),
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::UnqualifiedCall,
        form: "#include \"legacy.h\"; legacy_go()",
        verdict: Verdict::Holds {
            fixture: "cppproj",
            expect: Expect::Edges(&[(
                "calls",
                "function:main.cpp:main",
                "function:legacy.cpp:legacy_go",
            )]),
        },
    },
    // -- a qualified call through an imported module -----------------------
    Cell {
        language: Language::TypeScript,
        convention: Convention::QualifiedCall,
        form: "import * as util; util.greet(\"ns\")",
        verdict: Verdict::Holds {
            fixture: "simple",
            expect: Expect::Edges(&[(
                "calls",
                "function:src/namespace.ts:viaNamespace",
                "function:src/util.ts:greet",
            )]),
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::QualifiedCall,
        form: "util::helper()",
        verdict: Verdict::Holds {
            fixture: "rustroot",
            expect: Expect::Edges(&[(
                "calls",
                "function:src/lib.rs:from_bare_module",
                "function:src/util.rs:helper",
            )]),
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::QualifiedCall,
        form: "from pkg import util; util.helper(2)",
        verdict: Verdict::Holds {
            fixture: "pypkgs",
            expect: Expect::Edges(&[(
                "calls",
                "function:uses_module.py:run",
                "function:pkg/util.py:helper",
            )]),
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::QualifiedCall,
        form: "import \"example.com/demo/util\"; util.Format(…)",
        verdict: Verdict::Filed {
            ticket: "37",
            fixture: "goproj",
            want: Expect::Edges(&[(
                "calls",
                "function:main.go:main",
                "function:util/util.go:Format",
            )]),
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::QualifiedCall,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "C has no module qualifier: a call is a bare identifier, and a \
                      dotted one reaches a struct field",
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::QualifiedCall,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "`util::helper()` qualifies by namespace or class, never by \
                      translation unit, so there is no module for the receiver to name",
        },
    },
    // -- a qualified call through an aliased module ------------------------
    Cell {
        language: Language::TypeScript,
        convention: Convention::QualifiedCallThroughAlias,
        form: "import * as u from \"./util\"; u.greet(\"alias\")",
        verdict: Verdict::Holds {
            fixture: "simple",
            expect: Expect::Edges(&[(
                "calls",
                "function:src/namespace.ts:viaAlias",
                "function:src/util.ts:greet",
            )]),
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::QualifiedCallThroughAlias,
        form: "use crate::util as u; u::helper()",
        verdict: Verdict::Holds {
            fixture: "rustroot",
            expect: Expect::Edges(&[(
                "calls",
                "function:src/lib.rs:through_alias",
                "function:src/util.rs:helper",
            )]),
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::QualifiedCallThroughAlias,
        form: "import pkg.util as pu; pu.helper(6)",
        verdict: Verdict::Holds {
            fixture: "pypkgs",
            expect: Expect::Edges(&[(
                "calls",
                "function:uses_dotted_alias.py:dotted_alias",
                "function:pkg/util.py:helper",
            )]),
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::QualifiedCallThroughAlias,
        form: "import u \"example.com/demo/util\"; u.Format(…)",
        verdict: Verdict::Filed {
            ticket: "37",
            fixture: "goproj",
            want: Expect::Edges(&[(
                "calls",
                "function:alias.go:aliased",
                "function:util/util.go:Format",
            )]),
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::QualifiedCallThroughAlias,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "C has neither a module qualifier nor an import alias",
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::QualifiedCallThroughAlias,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "a C++ namespace alias renames a namespace, not a translation unit, \
                      so there is still no module for the receiver to name",
        },
    },
    // -- a qualified call through a nested module path ---------------------
    Cell {
        language: Language::TypeScript,
        convention: Convention::QualifiedCallThroughNestedPath,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "`a.b.c()` can never name a module in JavaScript — a namespace \
                      import binds exactly one identifier, and anything deeper is a \
                      property of a value (ticket 21 removed the branch that tried)",
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::QualifiedCallThroughNestedPath,
        form: "crate::util::helper()",
        verdict: Verdict::Holds {
            fixture: "rustroot",
            expect: Expect::Edges(&[
                (
                    "calls",
                    "function:src/lib.rs:from_crate_root",
                    "function:src/util.rs:helper",
                ),
                (
                    "calls",
                    "function:src/deep/mod.rs:up_and_across",
                    "function:src/util.rs:helper",
                ),
            ]),
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::QualifiedCallThroughNestedPath,
        form: "import pkg.util; pkg.util.helper(5)",
        verdict: Verdict::Holds {
            fixture: "pypkgs",
            expect: Expect::Edges(&[(
                "calls",
                "function:uses_dotted.py:dotted",
                "function:pkg/util.py:helper",
            )]),
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::QualifiedCallThroughNestedPath,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "a Go call qualifier is a single package identifier; the import \
                      path's own segments never appear at the call site",
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::QualifiedCallThroughNestedPath,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "C has no module qualifier at all, nested or otherwise",
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::QualifiedCallThroughNestedPath,
        form: "—",
        verdict: Verdict::NotApplicable {
            because: "a nested `a::b::c()` qualifies by nested namespace, never by \
                      translation unit",
        },
    },
    // -- NON-EDGE: a call whose receiver is a value ------------------------
    Cell {
        language: Language::TypeScript,
        convention: Convention::ReceiverIsAValue,
        form: "handle(logger) { logger.info(\"hi\") }  // beside src/logger.ts",
        verdict: Verdict::Holds {
            fixture: "simple",
            expect: Expect::NoEdge {
                kind: "calls",
                source: "function:src/service.ts:handle",
                decoy: "function:src/logger.ts:info",
            },
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::ReceiverIsAValue,
        form: "call_on_a_value(util: Util) { util.helper() }  // in the file declaring mod util;",
        verdict: Verdict::Holds {
            fixture: "rustroot",
            expect: Expect::NoEdge {
                kind: "calls",
                source: "function:src/lib.rs:call_on_a_value",
                decoy: "function:src/util.rs:helper",
            },
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::ReceiverIsAValue,
        form: "call_on_a_value(util) { util.helper(1) }  // beside pkg/util.py",
        verdict: Verdict::Holds {
            fixture: "pypkgs",
            expect: Expect::NoEdge {
                kind: "calls",
                source: "function:pkg/uses_value.py:call_on_a_value",
                decoy: "function:pkg/util.py:helper",
            },
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::ReceiverIsAValue,
        form: "import \"…/util\"; onValue(util Logger) { util.Format(…) }  // shadowed",
        verdict: Verdict::Vacuous {
            ticket: "37",
            fixture: "goproj",
            guard: Expect::NoEdge {
                kind: "calls",
                source: "function:value.go:onValue",
                decoy: "function:util/util.go:Format",
            },
            vacuous_until: Expect::Edges(&[(
                "calls",
                "function:main.go:main",
                "function:util/util.go:Format",
            )]),
            because: "`go.rs` keeps a call only when the callee node is an `identifier`, \
                      and `util.Format(…)` is a `selector_expression` — so no Go selector \
                      call is recorded at all and no mutation of the resolver can \
                      fabricate this edge. The decoy is a real temptation now rather than \
                      a pose: `value.go` imports the `util` package and then shadows the \
                      name with a parameter holding a Logger, which is precisely what a \
                      resolver that binds package qualifiers without checking for \
                      shadowing would get wrong. It cannot get it wrong until ticket 37 \
                      records selector calls",
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::ReceiverIsAValue,
        form: "run_through(struct ops o) { o.util_greet(\"x\") }  // util.h is included",
        verdict: Verdict::Holds {
            fixture: "cproj",
            expect: Expect::NoEdge {
                kind: "calls",
                source: "function:ops.c:run_through",
                decoy: "function:util.c:util_greet",
            },
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::ReceiverIsAValue,
        form: "c.area()  // geometry.hpp also declares a free area()",
        verdict: Verdict::Holds {
            fixture: "cppproj",
            expect: Expect::NoEdge {
                kind: "calls",
                source: "function:main.cpp:main",
                decoy: "function:geometry.cpp:area",
            },
        },
    },
    // -- NON-EDGE: a call into a package outside the repository ------------
    Cell {
        language: Language::TypeScript,
        convention: Convention::CallIntoOutsidePackage,
        form: "import * as ext from \"node:util\"; ext.greet(…)",
        verdict: Verdict::Holds {
            fixture: "simple",
            expect: Expect::NoEdge {
                kind: "calls",
                source: "function:src/namespace.ts:viaExternal",
                decoy: "function:src/util.ts:greet",
            },
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::CallIntoOutsidePackage,
        form: "serde_json::from_str(\"7\")  // src/util.rs exports a from_str",
        verdict: Verdict::Holds {
            fixture: "rustroot",
            expect: Expect::NoEdge {
                kind: "calls",
                source: "function:src/lib.rs:external",
                decoy: "function:src/util.rs:from_str",
            },
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::CallIntoOutsidePackage,
        form: "import os; os.helper()  // pkg/util.py exports a helper",
        verdict: Verdict::Holds {
            fixture: "pypkgs",
            expect: Expect::NoEdge {
                kind: "calls",
                source: "function:uses_absent.py:nothing",
                decoy: "function:pkg/util.py:helper",
            },
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::CallIntoOutsidePackage,
        form: "import \"github.com/external/lib/util\"; util.Format(…)",
        verdict: Verdict::Vacuous {
            ticket: "37",
            fixture: "goproj",
            guard: Expect::NoEdge {
                kind: "calls",
                source: "function:external.go:external",
                decoy: "function:util/util.go:Format",
            },
            vacuous_until: Expect::Edges(&[(
                "calls",
                "function:main.go:main",
                "function:util/util.go:Format",
            )]),
            because: "the same reason as the value-receiver cell above: no Go selector \
                      call is recorded, so there is no call for an over-eager resolver \
                      to mis-target. The import half of this guard is real and asserted \
                      by the resolves-nowhere cell, which pins the whole edge set out of \
                      `external.go` as empty; the call half waits on ticket 37",
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::CallIntoOutsidePackage,
        form: "printf(…) through <stdio.h>  // decoy.c defines a printf",
        verdict: Verdict::Holds {
            fixture: "cproj",
            expect: Expect::NoEdge {
                kind: "calls",
                source: "function:main.c:main",
                decoy: "function:decoy.c:printf",
            },
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::CallIntoOutsidePackage,
        form: "puts(…) through <cstdio>  // decoy.cpp defines a puts",
        verdict: Verdict::Holds {
            fixture: "cppproj",
            expect: Expect::NoEdge {
                kind: "calls",
                source: "function:main.cpp:main",
                decoy: "function:decoy.cpp:puts",
            },
        },
    },
    // -- NON-EDGE: an import resolving to no file in the repository --------
    Cell {
        language: Language::TypeScript,
        convention: Convention::ImportResolvingNowhere,
        form: "import { ghost } from \"./missing\";  // src/lib/missing.ts is the decoy",
        verdict: Verdict::Holds {
            fixture: "simple",
            expect: Expect::EdgeSetFrom {
                kind: "imports",
                source: "file:src/app.ts",
                targets: &["file:src/arrow.ts", "file:src/lib/index.ts"],
                decoys: &["file:src/lib/missing.ts"],
            },
        },
    },
    Cell {
        language: Language::Rust,
        convention: Convention::ImportResolvingNowhere,
        form: "use serde::Serialize;  // vendor/log is the decoy for `use log::note`",
        verdict: Verdict::Holds {
            fixture: "rustws",
            expect: Expect::EdgeSetFrom {
                kind: "imports",
                source: "file:crates/cli/src/main.rs",
                targets: &[
                    "file:crates/atlas-engine/src/engine.rs",
                    "file:crates/log/src/lib.rs",
                    "file:crates/toolbox/src/lib.rs",
                ],
                decoys: &["file:vendor/log/src/lib.rs"],
            },
        },
    },
    Cell {
        language: Language::Python,
        convention: Convention::ImportResolvingNowhere,
        form: "from ns import nowhere  // ns/ is a real namespace package: the decoy",
        verdict: Verdict::Holds {
            fixture: "pypkgs",
            expect: Expect::EdgeSetFrom {
                kind: "imports",
                source: "file:uses_absent.py",
                targets: &[],
                decoys: &["file:ns/parse.py", "file:ns/emit.py"],
            },
        },
    },
    Cell {
        language: Language::Go,
        convention: Convention::ImportResolvingNowhere,
        form: "import \"github.com/external/lib/util\"  // suffix collides with ./util",
        verdict: Verdict::Holds {
            fixture: "goproj",
            expect: Expect::EdgeSetFrom {
                kind: "imports",
                source: "file:external.go",
                targets: &[],
                decoys: &["file:util/util.go", "file:util/extra.go"],
            },
        },
    },
    Cell {
        language: Language::C,
        convention: Convention::ImportResolvingNowhere,
        form: "#include <stdio.h>; #include \"config.h\"  // app/config.h is the decoy",
        verdict: Verdict::Holds {
            fixture: "cproj",
            expect: Expect::EdgeSetFrom {
                kind: "imports",
                source: "file:main.c",
                targets: &["file:app/app.h", "file:util.h"],
                decoys: &["file:app/config.h"],
            },
        },
    },
    Cell {
        language: Language::Cpp,
        convention: Convention::ImportResolvingNowhere,
        form: "#include <iostream>; #include \"shapes.hpp\"  // detail/shapes.hpp is the decoy",
        verdict: Verdict::Holds {
            fixture: "cppproj",
            expect: Expect::EdgeSetFrom {
                kind: "imports",
                source: "file:main.cpp",
                targets: &["file:geometry.hpp", "file:legacy.h", "file:report.hpp"],
                decoys: &["file:detail/shapes.hpp"],
            },
        },
    },
];

// ---------------------------------------------------------------------------
// Running a cell
// ---------------------------------------------------------------------------

/// The map a fixture scans to, computed once per test binary. Fourteen tests
/// over nine fixtures would otherwise re-run the binary eighty-odd times for
/// byte-identical output.
///
/// The global lock is held only long enough to hand out the slot for one
/// fixture; the scan itself runs under that fixture's own `OnceLock`. Holding
/// the global lock across `materialize` and a subprocess spawn would serialise
/// every fixture in the binary behind whichever one a test happened to ask for
/// first, for no gain — two different fixtures share nothing.
fn map_of(fixture: &str) -> Value {
    static MAPS: OnceLock<Mutex<HashMap<String, Arc<OnceLock<Value>>>>> = OnceLock::new();
    let cache = MAPS.get_or_init(|| Mutex::new(HashMap::new()));
    let slot = {
        let mut maps = cache.lock().unwrap();
        Arc::clone(maps.entry(fixture.to_string()).or_default())
    };
    slot.get_or_init(|| {
        let repo = materialize(fixture);
        assert_cmd::Command::cargo_bin("codeatlas")
            .unwrap()
            .arg("scan")
            .current_dir(repo.path())
            .assert()
            .success();
        read_map(repo.path())
    })
    .clone()
}

fn has_node(map: &Value, id: &str) -> bool {
    map["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["id"] == id)
}

/// Whether the map satisfies the expectation, and if not, why not.
fn holds(map: &Value, expect: &Expect) -> Result<(), String> {
    match expect {
        Expect::Edges(edges) => {
            let missing: Vec<String> = edges
                .iter()
                .filter(|(kind, source, target)| !has_edge(map, kind, source, target))
                .map(|(kind, source, target)| format!("no {kind} edge {source} -> {target}"))
                .collect();
            if missing.is_empty() {
                Ok(())
            } else {
                Err(missing.join("; "))
            }
        }
        Expect::NoEdge {
            kind,
            source,
            decoy,
        } => {
            if has_edge(map, kind, source, decoy) {
                Err(format!(
                    "fabricated {kind} edge {source} -> {decoy}: the resolver reached the decoy"
                ))
            } else {
                Ok(())
            }
        }
        Expect::EdgeSetFrom {
            kind,
            source,
            targets,
            ..
        } => {
            let mut found: Vec<&str> = map["edges"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|e| e["kind"] == *kind && e["source"] == *source)
                .filter_map(|e| e["target"].as_str())
                .collect();
            found.sort_unstable();
            let mut want = targets.to_vec();
            want.sort_unstable();
            if found == want {
                Ok(())
            } else {
                Err(format!(
                    "the {kind} edges out of {source} are {found:?}, not {want:?}"
                ))
            }
        }
    }
}

impl Cell {
    /// Every expectation the cell names: the one it asserts, plus — for a
    /// vacuous cell — the one whose *absence* is why it cannot fail. Both are
    /// preflighted, because a vacuous cell's honesty depends on the second
    /// expectation still naming nodes that exist.
    fn expectations(&self) -> Vec<&Expect> {
        match &self.verdict {
            Verdict::Holds { expect, .. } => vec![expect],
            Verdict::Filed { want, .. } => vec![want],
            Verdict::Vacuous {
                guard,
                vacuous_until,
                ..
            } => vec![guard, vacuous_until],
            Verdict::NotApplicable { .. } => Vec::new(),
        }
    }

    fn fixture(&self) -> Option<&'static str> {
        match &self.verdict {
            Verdict::Holds { fixture, .. }
            | Verdict::Filed { fixture, .. }
            | Verdict::Vacuous { fixture, .. } => Some(fixture),
            Verdict::NotApplicable { .. } => None,
        }
    }

    /// The ticket this cell is waiting on, if any — filed or vacuous alike.
    /// Both are promises about work that does not exist yet, so both owe the
    /// reader a ticket that does.
    fn ticket(&self) -> Option<&'static str> {
        match &self.verdict {
            Verdict::Filed { ticket, .. } | Verdict::Vacuous { ticket, .. } => Some(ticket),
            Verdict::Holds { .. } | Verdict::NotApplicable { .. } => None,
        }
    }

    /// Every node the cell names is really in the fixture's map — asserted for
    /// filed cells as loudly as for passing ones. This is what stops a
    /// non-edge row decaying into a tautology: the decoy has to keep existing,
    /// so "no edge to it" keeps meaning something, and a filed row's gap is
    /// the missing *edge* rather than a fixture that never had the pieces.
    fn preflight(&self, map: &Value) {
        for id in self.expectations().iter().flat_map(|e| e.nodes()) {
            assert!(
                has_node(map, id),
                "{} / {}: the fixture no longer holds {id}, so this cell asserts nothing",
                self.language.label(),
                self.convention.label(),
            );
        }
    }

    /// A failure message that names the row, the source form and the fixture,
    /// so a red row can be read without opening this file.
    fn blame(&self, fixture: &str, why: &str) -> String {
        format!(
            "{} — `{}` in fixture `{fixture}`: {why}",
            self.language.label(),
            self.form,
        )
    }

    fn check(&self) -> Result<(), String> {
        let Some(fixture) = self.fixture() else {
            return Ok(()); // not-applicable: no fixture, so nothing to run
        };
        let map = map_of(fixture);
        self.preflight(&map);
        match &self.verdict {
            Verdict::Holds { expect, .. } => {
                holds(&map, expect).map_err(|why| self.blame(fixture, &why))
            }
            Verdict::Filed { ticket, want, .. } => match holds(&map, want) {
                Err(_) => Ok(()), // the gap is still there
                Ok(()) => Err(self.blame(
                    fixture,
                    &format!(
                        "this row now PASSES. Close ticket {ticket} and move the cell to \
                         Verdict::Holds."
                    ),
                )),
            },
            Verdict::Vacuous {
                ticket,
                guard,
                vacuous_until,
                ..
            } => {
                // The guard is asserted anyway — a cell that runs nothing is
                // worth less than one that runs something unfalsifiable.
                holds(&map, guard).map_err(|why| self.blame(fixture, &why))?;
                match holds(&map, vacuous_until) {
                    Err(_) => Ok(()), // still nothing that could trip the guard
                    Ok(()) => Err(self.blame(
                        fixture,
                        &format!(
                            "this cell is no longer vacuous — ticket {ticket} looks to have \
                             landed. Re-prove the guard by tampering, then move the cell to \
                             Verdict::Holds."
                        ),
                    )),
                }
            }
            // `fixture()` answers `None` for a not-applicable cell, so the
            // early return above has already taken it.
            Verdict::NotApplicable { .. } => unreachable!("a not-applicable cell names no fixture"),
        }
    }

    /// How the cell reads in the rendered table.
    fn status(&self) -> String {
        match &self.verdict {
            Verdict::Holds { .. } => "pass".to_string(),
            Verdict::NotApplicable { .. } => "n/a".to_string(),
            Verdict::Filed { ticket, .. } => format!("ticket {ticket}"),
            Verdict::Vacuous { ticket, .. } => format!("vacuous ({ticket})"),
        }
    }
}

/// Runs every cell of one row and reports the whole row's failures at once —
/// a row is the unit a reader cares about, and stopping at the first language
/// hides how wide the gap is.
fn check_row(convention: Convention) {
    let failures: Vec<String> = CHECKLIST
        .iter()
        .filter(|cell| cell.convention == convention)
        .filter_map(|cell| cell.check().err())
        .collect();
    assert!(
        failures.is_empty(),
        "{}:\n  {}",
        convention.label(),
        failures.join("\n  ")
    );
}

// ---------------------------------------------------------------------------
// The rows
// ---------------------------------------------------------------------------

#[test]
fn a_plain_module_import() {
    check_row(Convention::PlainModuleImport);
}

#[test]
fn a_named_or_member_import() {
    check_row(Convention::NamedImport);
}

#[test]
fn an_aliased_import() {
    check_row(Convention::AliasedImport);
}

#[test]
fn a_namespace_or_whole_module_import() {
    check_row(Convention::WholeModuleImport);
}

#[test]
fn a_relative_import() {
    check_row(Convention::RelativeImport);
}

#[test]
fn a_package_or_directory_import_through_an_initialiser_or_index_file() {
    check_row(Convention::PackageOrIndexImport);
}

#[test]
fn a_header_source_pairing() {
    check_row(Convention::HeaderSourcePair);
}

#[test]
fn an_unqualified_call_to_an_imported_name() {
    check_row(Convention::UnqualifiedCall);
}

#[test]
fn a_qualified_call_through_an_imported_module() {
    check_row(Convention::QualifiedCall);
}

#[test]
fn a_qualified_call_through_an_aliased_module() {
    check_row(Convention::QualifiedCallThroughAlias);
}

#[test]
fn a_qualified_call_through_a_nested_module_path() {
    check_row(Convention::QualifiedCallThroughNestedPath);
}

#[test]
fn no_edge_when_a_calls_receiver_is_a_value_rather_than_a_module() {
    check_row(Convention::ReceiverIsAValue);
}

#[test]
fn no_edge_for_a_call_into_a_package_outside_the_repository() {
    check_row(Convention::CallIntoOutsidePackage);
}

#[test]
fn no_edge_for_an_import_resolving_to_no_file_in_the_repository() {
    check_row(Convention::ImportResolvingNowhere);
}

// ---------------------------------------------------------------------------
// The table's own shape
// ---------------------------------------------------------------------------

/// The table is the deliverable, so its shape is asserted too: exhaustive, no
/// duplicates, every not-applicable cell reasoned, and every filed cell
/// pointing at a ticket that exists on disk. Without the last one the escape
/// hatch — "a failing row is filed rather than fixed" — would be a comment
/// rather than a commitment.
#[test]
fn the_table_holds_one_reasoned_cell_for_every_convention_in_every_language() {
    let mut seen: BTreeMap<(Convention, Language), &Cell> = BTreeMap::new();
    for cell in CHECKLIST {
        assert!(
            seen.insert((cell.convention, cell.language), cell)
                .is_none(),
            "two cells for {} / {}",
            cell.language.label(),
            cell.convention.label()
        );
    }

    let mut absent = Vec::new();
    for convention in CONVENTIONS {
        for language in LANGUAGES {
            if !seen.contains_key(&(convention, language)) {
                absent.push(format!("{} / {}", language.label(), convention.label()));
            }
        }
    }
    assert!(
        absent.is_empty(),
        "the checklist has holes, which is exactly what it exists to prevent:\n  {}",
        absent.join("\n  ")
    );
    assert_eq!(
        CHECKLIST.len(),
        CONVENTIONS.len() * LANGUAGES.len(),
        "the table must be {} × {} cells",
        CONVENTIONS.len(),
        LANGUAGES.len()
    );

    // `.scratch/<spec-slug>/` holds the tickets and CLAUDE.md calls it
    // disposable once the feature ships, so its absence is a shipped
    // repository rather than a defect — and `cargo test` must not turn tidying
    // up into a red suite that names a missing scratch directory instead of a
    // code fault. A directory that *is* there and lacks the named ticket still
    // fails: while the tickets exist, the escape hatch has to point at one.
    let tickets = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.scratch/codeatlas-v1");
    let filed: Option<HashSet<String>> = std::fs::read_dir(&tickets).ok().map(|entries| {
        entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
            .collect()
    });
    for cell in CHECKLIST {
        match &cell.verdict {
            Verdict::NotApplicable { because } => assert!(
                !because.is_empty(),
                "{} / {} is not-applicable without saying why",
                cell.language.label(),
                cell.convention.label()
            ),
            Verdict::Vacuous { because, .. } => assert!(
                !because.is_empty(),
                "{} / {} cannot fail and does not say why",
                cell.language.label(),
                cell.convention.label()
            ),
            Verdict::Holds { .. } | Verdict::Filed { .. } => {}
        }
        if let (Some(ticket), Some(filed)) = (cell.ticket(), &filed) {
            assert!(
                filed
                    .iter()
                    .any(|name| name.starts_with(&format!("{ticket}-"))),
                "{} / {} waits on ticket {ticket}, and no such ticket exists in {}",
                cell.language.label(),
                cell.convention.label(),
                tickets.display()
            );
        }
    }

    // Rendered for `cargo test -- --nocapture`: the point of a table is that
    // coverage is something a reader can see.
    let mut rendered = String::from("\nstory 2's convention checklist\n");
    for convention in CONVENTIONS {
        rendered.push_str(&format!("\n{}\n", convention.label()));
        for language in LANGUAGES {
            let cell = seen[&(convention, language)];
            rendered.push_str(&format!(
                "  {:<12} {:<13} {}\n",
                language.label(),
                cell.status(),
                cell.form
            ));
        }
    }
    println!("{rendered}");
}
