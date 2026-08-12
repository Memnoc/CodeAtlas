//! Go extraction via the compiled-in tree-sitter grammar.
//!
//! Go's unit of import is the package (a directory). Module-path imports are
//! anchored by the root go.mod's `module` line: only imports under this
//! module's own path resolve, so an external module whose suffix collides
//! with an in-repo directory never produces an edge. Repos without a go.mod
//! fall back to conservative prefix-stripping (documented residual risk at
//! the fallback site; nested go.mod modules are not modeled). The
//! resolved edge points at the package's anchor file (`<dir>/<dir>.go` when
//! present, else the first Go file in the directory). Stdlib and external
//! modules never resolve. Same-package calls need no import at all —
//! [`Parser::directory_shares_scope`] tells the resolver so.
//!
//! **The package qualifier.** Every cross-package call Go has is written
//! `util.Format(…)`; the language has no member import, so this is not a
//! style a codebase can avoid. An import therefore binds a namespace — the
//! alias when the statement writes one, else the package's own name — and a
//! `selector_expression` callee is recorded as a qualified call with that
//! single identifier as its receiver. A qualifier is always one identifier:
//! in `a.b.C()` the `a.b` is a field of a value, never a package.
//!
//! Two things stop that binding from inventing edges. Go keeps the dotted
//! language default of [`Parser::receiver_is_never_a_value`], so a receiver
//! no import bound resolves to nothing rather than being resolved on sight.
//! And a receiver the enclosing function *declares* is a value whatever the
//! file's imports say, so [`local_bindings`] collects the function's own
//! names and a call through one of them is not recorded at all.
//!
//! That second check suppresses **more** than a strict reading of Go's scopes
//! would, and the extra reach is worth stating rather than discovering. It is
//! whole-function, blind to declaration order, and it extends through every
//! nested function literal — so a *closure parameter* named after a package
//! suppresses that package for the length of the enclosing function, not just
//! inside the closure. Three shapes of legal Go therefore lose an edge:
//! `cfg := cfg.Load()`, where the right-hand side really is the package
//! because a `:=` name's scope begins only after the statement; a shadowing
//! declaration in a sibling block that the call never enters; and the closure
//! case above. Each costs one edge, where following a shadowed receiver would
//! invent one between two files with no relationship at all — the bug ticket
//! 21 shipped. With the error directions that asymmetric, the approximation
//! leans.
//!
//! **Dot imports.** `import . "p"` binds every exported name of `p`
//! unqualified, and one file cannot know which names those are — so
//! [`bind_dot_imports`] offers every callee the file does not define to the
//! dot import and lets cross-file resolution keep the ones the package really
//! exports, exactly as `c_cpp` does for a quoted `#include`.

use std::collections::HashSet;

use tree_sitter::Node as TsNode;

use super::{Analysis, Call, Import, ImportedName, Parser, Symbol, SymbolKind};

pub(super) struct Go;

pub(super) fn parsers() -> Vec<Box<dyn Parser>> {
    vec![Box::new(Go)]
}

impl Parser for Go {
    fn language_name(&self) -> &'static str {
        "Go"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["go"]
    }

    fn directory_shares_scope(&self) -> bool {
        true // a directory is a package; its files share one namespace
    }

    fn parse(&self, source: &str) -> Analysis {
        let mut parser = tree_sitter::Parser::new();
        if parser
            .set_language(&tree_sitter_go::LANGUAGE.into())
            .is_err()
        {
            return Analysis::default();
        }
        let Some(tree) = parser.parse(source, None) else {
            return Analysis::default();
        };
        let mut collected = Collected::default();
        // Nothing is declared at file scope that could shadow a qualifier: an
        // identifier cannot be declared in both the file block (where imports
        // live) and the package block, so only a function's own names shadow.
        let file_scope = HashSet::new();
        collect(
            tree.root_node(),
            source.as_bytes(),
            Ctx {
                enclosing_fn: None,
                shadowed: &file_scope,
            },
            &mut collected,
        );
        bind_dot_imports(&mut collected);
        collected.analysis
    }

    fn resolve_import(
        &self,
        importer: &str,
        specifier: &str,
        files: &HashSet<String>,
        root: &std::path::Path,
    ) -> Option<String> {
        // Legacy relative imports resolve like any path.
        if specifier.starts_with("./") || specifier.starts_with("../") {
            let mut parts: Vec<&str> = importer.split('/').collect();
            parts.pop();
            for segment in specifier.split('/') {
                match segment {
                    "" | "." => {}
                    ".." => {
                        parts.pop()?;
                    }
                    s => parts.push(s),
                }
            }
            return package_anchor(&parts.join("/"), files);
        }

        // Module path: anchored by the root go.mod when present — the
        // import must start with this module's own path, everything else is
        // an external module even when its trailing segments collide with an
        // in-repo directory (e.g. github.com/external/lib/util vs ./util).
        if files.contains("go.mod") {
            let module = std::fs::read_to_string(root.join("go.mod"))
                .ok()
                .and_then(|src| module_path(&src))?;
            let rest = specifier.strip_prefix(&module)?.strip_prefix('/')?;
            return package_anchor(rest, files);
        }

        // No go.mod (legacy layout): fall back to prefix-stripping — keep
        // the import only when stripping 1..n-1 leading segments names
        // exactly one in-map package. Residual risk, accepted for module-
        // less repos only: an external import whose suffix uniquely matches
        // an in-repo directory still produces a false edge.
        let segments: Vec<&str> = specifier.split('/').collect();
        let mut matches: Vec<String> = (1..segments.len())
            .filter_map(|skip| {
                let dir = segments[skip..].join("/");
                package_anchor(&dir, files)
            })
            .collect();
        matches.sort();
        matches.dedup();
        match matches.as_slice() {
            [only] => Some(only.clone()),
            _ => None, // no match, or ambiguous — resolution declined
        }
    }
}

/// The `module` line of a go.mod: `module example.com/demo` → the path.
fn module_path(go_mod: &str) -> Option<String> {
    go_mod.lines().find_map(|line| {
        line.trim()
            .strip_prefix("module")
            .and_then(|rest| rest.strip_prefix(char::is_whitespace))
            .map(|path| path.trim().trim_matches('"').to_string())
    })
}

/// The file an edge to package directory `dir` points at: `<dir>/<name>.go`
/// named after the directory when present, else the lexicographically first
/// Go file directly inside it.
fn package_anchor(dir: &str, files: &HashSet<String>) -> Option<String> {
    if dir.is_empty() {
        return None;
    }
    let named = format!("{dir}/{}.go", dir.rsplit('/').next().unwrap_or(dir));
    if files.contains(&named) {
        return Some(named);
    }
    files
        .iter()
        .filter(|f| {
            f.strip_prefix(dir)
                .and_then(|rest| rest.strip_prefix('/'))
                .is_some_and(|rest| rest.ends_with(".go") && !rest.contains('/'))
        })
        .min()
        .cloned()
}

/// What one parse produces: the analysis, plus the bookkeeping
/// [`bind_dot_imports`] needs once the whole file has been walked.
#[derive(Default)]
struct Collected {
    analysis: Analysis,
    /// Indices into `analysis.imports` of the file's dot imports. Recorded
    /// rather than re-derived because the `.` is a property of the statement
    /// and nothing in the resulting [`Import`] remembers it.
    dot_imports: Vec<usize>,
}

#[derive(Clone, Copy)]
struct Ctx<'a> {
    /// Index into `Analysis::symbols` of the innermost enclosing function.
    enclosing_fn: Option<usize>,
    /// Names the enclosing function declares. A package qualifier of the same
    /// name is shadowed by every one of them, so a call through it is a method
    /// call on a value and not a package member.
    shadowed: &'a HashSet<String>,
}

fn collect(node: TsNode, source: &[u8], ctx: Ctx, out: &mut Collected) {
    match node.kind() {
        "import_spec" => {
            if let Some(path) = node
                .child_by_field_name("path")
                .and_then(|n| n.utf8_text(source).ok())
            {
                let specifier = path.trim_matches(['"', '`']).to_string();
                // The statement's optional `name`: an alias replacing the
                // package's own name, `.` binding every exported name
                // unqualified, or `_` binding nothing at all (a side-effect
                // import, which still makes the file edge).
                let written = node
                    .child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source).ok());
                let namespaces = match written {
                    Some(".") | Some("_") => Vec::new(),
                    Some(alias) => vec![alias.to_string()],
                    None => package_qualifier(&specifier).into_iter().collect(),
                };
                if written == Some(".") {
                    out.dot_imports.push(out.analysis.imports.len());
                }
                out.analysis.imports.push(Import {
                    specifier,
                    // A plain import binds no *name*, only the qualifier
                    // above; a dot import binds every exported name, and
                    // `bind_dot_imports` fills those in once the file's
                    // callees are known.
                    names: Vec::new(),
                    namespaces,
                });
            }
            return;
        }
        "call_expression" => {
            if let Some(caller_idx) = ctx.enclosing_fn
                && let Some(function) = node.child_by_field_name("function")
                && let Some((receiver, callee)) = callee_of(function, source, ctx.shadowed)
            {
                out.analysis.calls.push(Call {
                    caller: out.analysis.symbols[caller_idx].name.clone(),
                    callee,
                    receiver,
                });
            }
        }
        _ => {}
    }

    let (kind, name, scope) = match node.kind() {
        "function_declaration" => (
            Some(SymbolKind::Function),
            field_text(node, "name", source),
            None,
        ),
        "method_declaration" => (
            Some(SymbolKind::Function),
            field_text(node, "name", source),
            receiver_type(node, source),
        ),
        // A named struct type is the closest Go analog of a class.
        "type_spec" if is_struct(node) => (
            Some(SymbolKind::Class),
            field_text(node, "name", source),
            None,
        ),
        _ => (None, None, None),
    };

    let mut pushed_fn = None;
    if let (Some(kind), Some(name)) = (kind, name.as_deref()) {
        let qualified = match &scope {
            Some(receiver) => format!("{receiver}.{name}"),
            None => name.to_string(),
        };
        out.analysis.symbols.push(Symbol {
            kind,
            name: qualified,
            start_line: node.start_position().row as u32 + 1,
            end_line: node.end_position().row as u32 + 1,
            // Go's notion of export: a capitalized top-level name. Methods
            // are not themselves exported (consistent with class methods in
            // the other languages).
            exported: scope.is_none() && name.chars().next().is_some_and(|c| c.is_uppercase()),
        });
        if kind == SymbolKind::Function {
            pushed_fn = Some(out.analysis.symbols.len() - 1);
        }
    }

    // A function is where names get declared, so it is where a qualifier can
    // be shadowed. Anything else inherits its enclosing function's set —
    // including a nested `func_literal`, which needs no set of its own:
    // `gather_bindings` already recursed into it when the enclosing
    // declaration was walked, so re-collecting there could only reproduce what
    // was inherited, at the price of a `HashSet` clone per literal. A literal
    // at *package* scope (`var f = func(){…}`) has no enclosing declaration
    // and so no set either, and needs none: with no enclosing function there
    // is no caller to attribute a call to, and the walk records nothing.
    let scope_names = matches!(node.kind(), "function_declaration" | "method_declaration")
        .then(|| local_bindings(node, source, ctx.shadowed));
    let child_ctx = Ctx {
        enclosing_fn: pushed_fn.or(ctx.enclosing_fn),
        shadowed: scope_names.as_ref().unwrap_or(ctx.shadowed),
    };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, source, child_ctx, out);
    }
}

/// The callee of a call expression as `(receiver, name)`, or `None` when the
/// call is not one this pipeline can bind.
///
/// A bare `run()` has no receiver. `util.Format(…)` has one, and it is the
/// only cross-package call form Go offers — but only when the operand is a
/// single identifier the enclosing function has not declared. Both conditions
/// matter and neither is redundant:
///
/// - a deeper operand (`a.b.C()`, `f().C()`, `p[i].C()`) is a field or result
///   of a *value*; a Go package qualifier is always exactly one identifier;
/// - an identifier the function declares is a value here whatever the file's
///   imports say. `goproj/value.go` imports the `util` package and then takes
///   a `util Logger` parameter, so the call site is written identically to a
///   real package call and only the declaration tells them apart.
///
/// Declining leaves no call at all rather than an unqualified one: a method
/// call on a value is not an edge, and rewriting it as `Format()` would offer
/// it to the same-package and imported-name lookups, which is a different
/// invented edge rather than a fix.
fn callee_of(
    function: TsNode,
    source: &[u8],
    shadowed: &HashSet<String>,
) -> Option<(Vec<String>, String)> {
    match function.kind() {
        "identifier" => Some((Vec::new(), function.utf8_text(source).ok()?.to_string())),
        "selector_expression" => {
            let operand = function
                .child_by_field_name("operand")
                .filter(|o| o.kind() == "identifier")?
                .utf8_text(source)
                .ok()?;
            if shadowed.contains(operand) {
                return None;
            }
            let field = function
                .child_by_field_name("field")?
                .utf8_text(source)
                .ok()?;
            Some((vec![operand.to_string()], field.to_string()))
        }
        _ => None,
    }
}

/// Every name the function rooted at `node` declares: its receiver,
/// parameters and named results, and every `:=`, `var`, `const`, `range`,
/// type-switch and channel-receive binding anywhere inside it — unioned with
/// whatever the enclosing scope already shadowed.
///
/// "Anywhere inside it" includes every nested function literal, because
/// [`gather_bindings`] recurses unconditionally. So the set a function is
/// walked with is already closed over its closures, and a literal needs no
/// set of its own — which is also why a closure's parameters shadow a package
/// qualifier for the whole enclosing function and not merely inside the
/// closure.
///
/// Deliberately whole-function rather than block-scoped, and deliberately
/// blind to declaration order. All three approximations err the same way:
/// they call a name shadowed where a stricter reading might not, and a
/// shadowed name only ever *declines* to follow a receiver. The cost of
/// declining wrongly is a missing edge — a name declared after the call that
/// uses it, or in a sibling block, or in a closure — and the cost of
/// following wrongly is an edge between two files with no relationship at
/// all, which is the bug ticket 21 shipped and ticket 33 built
/// `goproj/value.go` to catch.
fn local_bindings(node: TsNode, source: &[u8], inherited: &HashSet<String>) -> HashSet<String> {
    let mut names = inherited.clone();
    gather_bindings(node, source, &mut names);
    names
}

fn gather_bindings(node: TsNode, source: &[u8], out: &mut HashSet<String>) {
    match node.kind() {
        // `func f(a, b int)`, `func f(rest ...int)`, `var x, y = …`,
        // `const k = …`: one node, a repeated `name` field.
        "parameter_declaration" | "variadic_parameter_declaration" | "var_spec" | "const_spec" => {
            let mut names = node.walk();
            for name in node.children_by_field_name("name", &mut names) {
                if let Ok(text) = name.utf8_text(source) {
                    out.insert(text.to_string());
                }
            }
        }
        // `x := …`, `for k, v := range …`, `case v := <-ch:`: an expression
        // list on the left.
        "short_var_declaration" | "range_clause" | "receive_statement" => {
            if let Some(left) = node.child_by_field_name("left") {
                insert_identifiers(left, source, out);
            }
        }
        // `switch t := x.(type)` binds `t` in every case clause.
        "type_switch_statement" => {
            if let Some(alias) = node.child_by_field_name("alias") {
                insert_identifiers(alias, source, out);
            }
        }
        _ => {}
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        gather_bindings(child, source, out);
    }
}

fn insert_identifiers(node: TsNode, source: &[u8], out: &mut HashSet<String>) {
    let mut cursor = node.walk();
    for child in node.named_children(&mut cursor) {
        if child.kind() == "identifier"
            && let Ok(text) = child.utf8_text(source)
        {
            out.insert(text.to_string());
        }
    }
}

/// The qualifier a plain `import "example.com/demo/util"` binds: `util`.
///
/// Go's actual rule is the target directory's `package` clause, which a
/// one-file-at-a-time parser cannot read, so this takes the last path
/// segment — the convention every Go project follows and the same assumption
/// `goimports` makes. A package whose clause name differs from its directory
/// name binds under the directory name instead, and that only ever costs an
/// edge: a receiver matching no bound name resolves to nothing, so the wrong
/// key cannot be reached by the right call site or the right key by a wrong
/// one.
fn package_qualifier(specifier: &str) -> Option<String> {
    let last = specifier.rsplit('/').next()?;
    (!last.is_empty()).then(|| last.to_string())
}

/// Offers every callee the file does not define to each of its dot imports.
///
/// `import . "p"` binds every *exported* name of `p` unqualified, and the
/// parser sees one file at a time, so which names those are is not knowable
/// here — the same problem `c_cpp` has with a header, solved the same way.
/// Cross-file resolution keeps only the candidates the package really
/// exports, so a name the package does not publish resolves to nothing rather
/// than to a guess, and a name the file defines itself is never offered at
/// all (in valid Go it could not collide with a dot import anyway).
///
/// One residual, inherited with the shape. "Defines itself" means a
/// *top-level* symbol: the shadow check that [`local_bindings`] performs is
/// consulted for selector receivers only, never for an unqualified callee, so
/// a local of func type whose name collides with a dot-imported export —
/// `F := func() {}; F()` — is still offered and resolves to the package's
/// `F`. It needs an uppercase local (Go exports only capitalised names) whose
/// name a dot-imported package also publishes, which is contrived, and it is
/// the same class of over-offer the C header handling already carries.
fn bind_dot_imports(out: &mut Collected) {
    if out.dot_imports.is_empty() {
        return;
    }
    let defined: HashSet<&str> = out
        .analysis
        .symbols
        .iter()
        .map(|s| s.name.as_str())
        .collect();
    let mut candidates: Vec<String> = out
        .analysis
        .calls
        .iter()
        // A qualified call reaches its callee through a package name, so it is
        // never the unqualified binding a dot import provides.
        .filter(|call| call.receiver.is_empty() && !defined.contains(call.callee.as_str()))
        .map(|call| call.callee.clone())
        .collect();
    candidates.sort_unstable();
    candidates.dedup();
    for index in &out.dot_imports {
        out.analysis.imports[*index].names = candidates
            .iter()
            .map(|name| ImportedName {
                local: name.clone(),
                imported: name.clone(),
            })
            .collect();
    }
}

/// The bare type name of a method's receiver: `(f *Formatter)` → `Formatter`.
fn receiver_type(node: TsNode, source: &[u8]) -> Option<String> {
    let receiver = node.child_by_field_name("receiver")?;
    let mut cursor = receiver.walk();
    let declaration = receiver
        .named_children(&mut cursor)
        .find(|c| c.kind() == "parameter_declaration")?;
    let ty = declaration.child_by_field_name("type")?;
    let text = ty.utf8_text(source).ok()?;
    let bare = text.trim_start_matches(['*', '&']);
    // Drop generic type arguments: `Box[T]` → `Box`.
    Some(bare.split('[').next().unwrap_or(bare).trim().to_string())
}

fn is_struct(type_spec: TsNode) -> bool {
    type_spec
        .child_by_field_name("type")
        .is_some_and(|t| t.kind() == "struct_type")
}

fn field_text(node: TsNode, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|n| n.utf8_text(source).ok())
        .map(|s| s.to_string())
}
