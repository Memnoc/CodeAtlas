//! C and C++ extraction via the compiled-in tree-sitter grammars.
//!
//! Decisions this module owns:
//!
//! - **`.h` belongs to the C parser** — the conventional owner: C++-only
//!   headers normally use `.hpp`/`.hh`/`.hxx`. A C++ project's `.h` headers
//!   still work: include edges are path-based, and the header/source pair
//!   search from a `.h` also tries the C++ source extensions.
//! - **Named struct definitions are Class nodes in both languages**,
//!   mirroring the Go parser's struct-as-class call: in C++ a struct is a
//!   class; in C a named struct (or `typedef struct { … } name`) is the
//!   file's closest analog — the data shape the file defines.
//! - **exported = external linkage**: a file-scope function not marked
//!   `static` is exported; `static` functions and methods are not. Classes
//!   and structs at file scope count as exported (types have no `static`).
//! - **Header/source pairing rides on the include**: an implementation file
//!   includes its own header, which is an ordinary `imports` edge. For call
//!   resolution, a file-scope prototype in a header is recorded as a
//!   re-export whose specifier is the [`PAIR`] marker; resolving that marker
//!   finds the same-stem source file next to the header, so a call through
//!   `#include "util.h"` lands on the implementation in `util.c`.
//! - **`#include "…"` resolves** relative to the includer's directory first,
//!   then repo-root-relative (the common `-I<root>` build convention).
//!   `#include <…>` system includes never resolve — no edge, never dangling.
//! - **Include name bindings are synthesized**: a single file cannot know
//!   which names a header provides, so every callee not defined in the file
//!   is offered to every quoted include; cross-file resolution keeps only
//!   the candidates the included header actually re-exports.
//! - **Namespaced symbols carry their qualified name** — `geo::nsq`,
//!   `geo::inner::deep` — which is the form idiomatic call sites use, and a
//!   qualified callee is recorded as that same text, so the two match by
//!   name (ticket 10, the spec's C++ namespaced-calls decision). This is
//!   where the C++ shape deliberately diverges from Go's receiver binding: a
//!   `::` scope names a namespace, never a translation unit, so there is no
//!   module for a receiver to resolve to and [`Call::receiver`] stays empty.
//!   A namespace does not change linkage — a non-static function at
//!   namespace scope is still exported (under the qualified name).
//! - **A bare callee walks its caller's namespace outward, within the file**:
//!   `nsq(k)` written inside `geo::twice` means the sibling `geo::nsq` when
//!   this file defines it, found by trying the caller's own namespace path
//!   prefix by prefix — C++'s enclosing-namespace lookup order, using only
//!   the path the caller's stored name already carries
//!   ([`qualify_bare_callees`]). What is *not* resolved is the genuine
//!   scope-tracking family: a `using namespace` binding, a namespace alias,
//!   and enclosing-namespace lookup *across* files (a sibling only declared
//!   in a header). Those stay unresolved rather than guessed at.

use std::collections::HashSet;

use tree_sitter::Node as TsNode;

use super::{Analysis, Call, Import, ImportedName, Parser, Reexport, Symbol, SymbolKind};

/// Marker specifier for a header's implementation pair; never a real include
/// path (quotes cannot contain a bare `@` include that we would emit).
const PAIR: &str = "@header-pair";

pub(super) struct CFamily {
    language_name: &'static str,
    extensions: &'static [&'static str],
    /// Source extensions a header of this language may pair with, in
    /// preference order.
    pair_sources: &'static [&'static str],
    language: fn() -> tree_sitter::Language,
}

pub(super) fn parsers() -> Vec<Box<dyn Parser>> {
    vec![
        Box::new(CFamily {
            language_name: "C",
            extensions: &["c", "h"],
            // A `.h` header may front a C or a C++ implementation.
            pair_sources: &["c", "cpp", "cc", "cxx"],
            language: || tree_sitter_c::LANGUAGE.into(),
        }),
        Box::new(CFamily {
            language_name: "C++",
            extensions: &["cpp", "cc", "cxx", "hpp", "hh", "hxx"],
            pair_sources: &["cpp", "cc", "cxx"],
            language: || tree_sitter_cpp::LANGUAGE.into(),
        }),
    ]
}

impl Parser for CFamily {
    fn language_name(&self) -> &'static str {
        self.language_name
    }

    fn extensions(&self) -> &'static [&'static str] {
        self.extensions
    }

    fn parse(&self, source: &str) -> Analysis {
        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&(self.language)()).is_err() {
            return Analysis::default();
        }
        let Some(tree) = parser.parse(source, None) else {
            return Analysis::default();
        };
        let mut analysis = Analysis::default();
        collect(
            tree.root_node(),
            source.as_bytes(),
            Ctx {
                scope: None,
                namespace: None,
                enclosing_fn: None,
            },
            &mut analysis,
        );
        qualify_bare_callees(&mut analysis);
        bind_includes(&mut analysis);
        analysis
    }

    fn resolve_import(
        &self,
        importer: &str,
        specifier: &str,
        files: &HashSet<String>,
        _root: &std::path::Path,
    ) -> Option<String> {
        let dir = importer.rsplit_once('/').map_or("", |(d, _)| d);
        if specifier == PAIR {
            // A header's implementation pair: the same-stem source file in
            // the header's own directory.
            let name = importer.rsplit('/').next().unwrap_or(importer);
            let stem = name.rsplit_once('.').map_or(name, |(s, _)| s);
            return self.pair_sources.iter().find_map(|ext| {
                let candidate = join(dir, &format!("{stem}.{ext}"));
                files.contains(&candidate).then_some(candidate)
            });
        }
        // `#include "…"`: relative to the includer's directory first, then
        // repo-root-relative (the -I<root> convention). Anything else —
        // absolute paths, traversal out of the repo — is dropped.
        if let Some(path) = normalize(&join(dir, specifier))
            && files.contains(&path)
        {
            return Some(path);
        }
        let rooted = normalize(specifier)?;
        files.contains(&rooted).then_some(rooted)
    }
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// Collapses `.` and `..` segments; `None` when the path escapes the repo
/// root or is absolute.
fn normalize(path: &str) -> Option<String> {
    if path.starts_with('/') {
        return None;
    }
    let mut parts: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

/// Restores what C++ enclosing-namespace lookup finds *within the caller's
/// own file*: a bare callee written inside `namespace geo` means a sibling
/// `geo::nsq` when this file defines one. No scope tracking is needed — the
/// caller's stored name already carries its namespace path — so the walk
/// tries the path prefix by prefix, innermost first (`geo::inner::f` calling
/// `nsq` tries `geo::inner::nsq`, then `geo::nsq`), which is C++'s actual
/// lookup order, and keeps the bare name when no enclosing prefix defines it
/// (a global-scope caller, or a name that really comes from an include).
/// Rewriting to the qualified stored name is what makes the same-file lookup
/// match again — and what keeps [`bind_includes`] from offering the name to
/// a header whose pair implements an unrelated global, so the suppression
/// that guard needs is caller-scoped by construction rather than file-wide.
///
/// The walk stops at this file's own definitions. Enclosing-namespace lookup
/// *across* files — a header's `geo::nsq` prototype answering a bare call
/// inside another file's `namespace geo` — is the scope-tracking family
/// ticket 10 parked alongside `using namespace`, and it stays parked.
fn qualify_bare_callees(analysis: &mut Analysis) {
    let defined: HashSet<&str> = analysis.symbols.iter().map(|s| s.name.as_str()).collect();
    for call in &mut analysis.calls {
        if call.callee.contains("::") {
            continue;
        }
        let mut namespace = caller_namespace(&call.caller);
        while !namespace.is_empty() {
            let candidate = format!("{namespace}::{}", call.callee);
            if defined.contains(candidate.as_str()) {
                call.callee = candidate;
                break;
            }
            namespace = namespace.rsplit_once("::").map_or("", |(outer, _)| outer);
        }
    }
}

/// The namespace path a caller's stored name carries: `geo::twice` lives in
/// `geo`, `geo::inner::deep` in `geo::inner`, and a global-scope `main` in
/// none. A method (`geo::Circle.area`) looks up from its class's namespace —
/// the path before the class's own segment; class-member lookup itself is
/// not modeled, matching the pre-ticket parser, whose bare stored names
/// never distinguished it either.
fn caller_namespace(caller: &str) -> &str {
    let scope = match caller.rsplit_once('.') {
        Some((class_scope, _)) => class_scope,
        None => caller,
    };
    scope
        .rsplit_once("::")
        .map_or("", |(namespace, _)| namespace)
}

/// Offers every callee the file does not define to every quoted include: the
/// parser sees one file at a time, so which header provides which name is
/// decided later by cross-file resolution (the header must re-export it).
///
/// Bare callees that really mean a namespaced sibling never reach the offer:
/// [`qualify_bare_callees`] runs first and rewrites them to the qualified
/// name the file defines, so `nsq(k)` inside `namespace geo` arrives here as
/// `geo::nsq` — defined, not offered — while the same spelling at *global
/// scope* stays bare and is offered, because lookup there cannot see
/// `geo::nsq` unqualified and a header's global `nsq` is a legitimate
/// answer. `cppproj/bare.hpp`/`bare.cpp` holds both proofs: the decoy the
/// namespaced caller must not reach, and the target the global caller must.
fn bind_includes(analysis: &mut Analysis) {
    let defined: HashSet<&str> = analysis.symbols.iter().map(|s| s.name.as_str()).collect();
    let mut candidates: Vec<&str> = analysis
        .calls
        .iter()
        .map(|c| c.callee.as_str())
        .filter(|callee| !defined.contains(callee))
        .collect();
    candidates.sort_unstable();
    candidates.dedup();
    let names: Vec<ImportedName> = candidates
        .iter()
        .map(|name| ImportedName {
            local: name.to_string(),
            imported: name.to_string(),
        })
        .collect();
    for import in &mut analysis.imports {
        import.names = names
            .iter()
            .map(|n| ImportedName {
                local: n.local.clone(),
                imported: n.imported.clone(),
            })
            .collect();
    }
}

#[derive(Clone, Copy)]
struct Ctx<'a> {
    /// Enclosing class/struct name, for scope-qualifying methods. Already
    /// namespace-qualified when the class sits inside a namespace.
    scope: Option<&'a str>,
    /// Enclosing namespace path (`geo`, `geo::inner`), for qualifying the
    /// names namespaced symbols are stored and exported under. Separate from
    /// `scope` because the two answer different questions: a class scope
    /// makes a method (never exported), a namespace changes only the name.
    namespace: Option<&'a str>,
    /// Index into `Analysis::symbols` of the innermost enclosing function.
    enclosing_fn: Option<usize>,
}

/// `name` as stored under the enclosing namespace: `geo` + `nsq` →
/// `geo::nsq`; no namespace, no change.
fn qualify(namespace: Option<&str>, name: &str) -> String {
    match namespace {
        Some(ns) => format!("{ns}::{name}"),
        None => name.to_string(),
    }
}

/// A qualified name normalized to how a definition stores it: segments
/// trimmed, empty ones dropped (`:: nsq` — explicit global qualification —
/// becomes the bare `nsq` it names), rejoined with `::`.
fn normalize_qualified(text: &str) -> String {
    text.split("::")
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>()
        .join("::")
}

fn collect(node: TsNode, source: &[u8], ctx: Ctx, out: &mut Analysis) {
    match node.kind() {
        "preproc_include" => {
            // `#include "…"` only; `<…>` system includes are ignored — an
            // out-of-repo target can never become an edge.
            if let Some(path) = node
                .child_by_field_name("path")
                .filter(|p| p.kind() == "string_literal")
                .and_then(|p| p.utf8_text(source).ok())
            {
                out.imports.push(Import {
                    specifier: path.trim_matches('"').to_string(),
                    names: Vec::new(), // filled by bind_includes
                    // A `::` in C++ names a namespace or a class, never a
                    // translation unit, so there is no module for a
                    // qualified callee's receiver to resolve to.
                    namespaces: Vec::new(),
                });
            }
            return;
        }
        "declaration" if ctx.scope.is_none() && ctx.enclosing_fn.is_none() => {
            // A file-scope prototype: the header's promise that its pair
            // (same-stem source file) implements the name. Recorded as a
            // re-export through the PAIR marker; harmless in a source file,
            // where the definition itself resolves first.
            if let Some(name) = node
                .child_by_field_name("declarator")
                .and_then(|d| declared_function(d, source))
                .filter(|(scope, _)| scope.is_none())
                .map(|(_, name)| qualify(ctx.namespace, &name))
            {
                out.reexports.push(Reexport {
                    exported: name.clone(),
                    local: name,
                    specifier: Some(PAIR.to_string()),
                });
            }
            return;
        }
        "call_expression" => {
            // A qualified callee — `geo::nsq(…)` — is recorded as its own
            // normalized text, the form namespaced definitions are stored
            // under, so the two match by name. The receiver stays empty: a
            // `::` scope names a namespace or a class, never a translation
            // unit, so there is no module for a receiver to resolve to.
            if let (Some(callee), Some(caller_idx)) = (
                node.child_by_field_name("function")
                    .filter(|f| matches!(f.kind(), "identifier" | "qualified_identifier"))
                    .and_then(|f| f.utf8_text(source).ok()),
                ctx.enclosing_fn,
            ) {
                out.calls.push(Call {
                    caller: out.symbols[caller_idx].name.clone(),
                    callee: normalize_qualified(callee),
                    receiver: Vec::new(),
                });
            }
        }
        _ => {}
    }

    let mut pushed_fn = None;
    let mut class_name: Option<String> = None;
    let mut namespace_path: Option<String> = None;
    match node.kind() {
        // `namespace geo { … }`, nested blocks, or the compact C++17
        // `namespace geo::inner { … }` (whose name field is one node with
        // `::` in its text). An anonymous namespace adds no qualifier: its
        // members are referred to unqualified, so the enclosing path is the
        // truest stored name. (Its internal linkage is not modeled — the
        // exported flag keeps meaning "not static", as before this ticket.)
        "namespace_definition" => {
            if let Some(name) = field_text(node, "name", source) {
                namespace_path = Some(qualify(ctx.namespace, &normalize_qualified(&name)));
            }
        }
        "function_definition" => {
            if let Some((scope, name)) = node
                .child_by_field_name("declarator")
                .and_then(|d| declared_function(d, source))
            {
                // `Circle::area` definitions carry their own scope, prefixed
                // by the enclosing namespace; inline methods take the
                // enclosing class's, which is already namespace-qualified.
                let scope = scope
                    .map(|own| qualify(ctx.namespace, &own))
                    .or_else(|| ctx.scope.map(str::to_string));
                let qualified = match &scope {
                    Some(s) => format!("{s}.{name}"),
                    None => qualify(ctx.namespace, &name),
                };
                out.symbols.push(Symbol {
                    kind: SymbolKind::Function,
                    name: qualified,
                    start_line: node.start_position().row as u32 + 1,
                    end_line: node.end_position().row as u32 + 1,
                    // External linkage: file-scope and not `static`. Methods
                    // are never exported (consistent with the other langs).
                    exported: scope.is_none() && !is_static(node, source),
                });
                pushed_fn = Some(out.symbols.len() - 1);
            }
        }
        // `struct point { … }` / `class Circle { … }`: a definition needs
        // both a name and a body (a bare `struct point;` is a declaration).
        "struct_specifier" | "class_specifier" => {
            if node.child_by_field_name("body").is_some()
                && let Some(name) = field_text(node, "name", source)
            {
                // A class in a namespace is a namespaced symbol like any
                // other: stored qualified, and its methods scope under the
                // qualified name (`geo::Circle.area`).
                let name = qualify(ctx.namespace, &name);
                push_class(node, name.clone(), ctx, out);
                class_name = Some(name);
            }
        }
        // `typedef struct { … } frame;` — the struct is anonymous; the
        // typedef names it. A named struct inside a typedef is already
        // emitted by the branch above.
        "type_definition" => {
            if let Some(inner) = node
                .child_by_field_name("type")
                .filter(|t| t.kind() == "struct_specifier")
                .filter(|t| t.child_by_field_name("body").is_some())
                .filter(|t| t.child_by_field_name("name").is_none())
                && let Some(name) = field_text(node, "declarator", source)
            {
                let name = qualify(ctx.namespace, &name);
                push_class(node, name.clone(), ctx, out);
                class_name = Some(name);
                // Recurse into the struct body under the typedef name.
                let child_ctx = Ctx {
                    scope: class_name.as_deref(),
                    namespace: ctx.namespace,
                    enclosing_fn: ctx.enclosing_fn,
                };
                let mut cursor = inner.walk();
                for child in inner.children(&mut cursor) {
                    collect(child, source, child_ctx, out);
                }
                return;
            }
        }
        _ => {}
    }

    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let child_ctx = Ctx {
            scope: class_name.as_deref().or(ctx.scope),
            namespace: namespace_path.as_deref().or(ctx.namespace),
            enclosing_fn: pushed_fn.or(ctx.enclosing_fn),
        };
        collect(child, source, child_ctx, out);
    }
}

fn push_class(node: TsNode, name: String, ctx: Ctx, out: &mut Analysis) {
    out.symbols.push(Symbol {
        kind: SymbolKind::Class,
        name,
        start_line: node.start_position().row as u32 + 1,
        end_line: node.end_position().row as u32 + 1,
        // Types have no linkage keyword; file-scope types count as
        // exported, nested ones do not.
        exported: ctx.scope.is_none(),
    });
}

/// Unwraps a declarator to the function it declares: `(scope, name)` where
/// scope is present for qualified definitions (`Circle::area`). `None` when
/// the declarator is not a function or is a function *pointer*
/// (`int (*fp)(void)` — a variable, not a function).
fn declared_function(mut declarator: TsNode, source: &[u8]) -> Option<(Option<String>, String)> {
    loop {
        match declarator.kind() {
            "pointer_declarator" | "reference_declarator" => {
                declarator = declarator.child_by_field_name("declarator")?;
            }
            "function_declarator" => {
                let inner = declarator.child_by_field_name("declarator")?;
                return match inner.kind() {
                    "identifier" | "field_identifier" | "operator_name" | "destructor_name" => {
                        Some((None, inner.utf8_text(source).ok()?.to_string()))
                    }
                    "qualified_identifier" => {
                        let text = inner.utf8_text(source).ok()?;
                        let (scope, name) = text.rsplit_once("::")?;
                        Some((
                            Some(scope.trim().replace("::", ".")),
                            name.trim().to_string(),
                        ))
                    }
                    // parenthesized_declarator: a function pointer.
                    _ => None,
                };
            }
            _ => return None,
        }
    }
}

/// Whether a definition carries the `static` storage class.
fn is_static(node: TsNode, source: &[u8]) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor).any(|c| {
        c.kind() == "storage_class_specifier" && c.utf8_text(source).is_ok_and(|t| t == "static")
    })
}

fn field_text(node: TsNode, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|n| n.utf8_text(source).ok())
        .map(|s| s.to_string())
}
