//! Go extraction via the compiled-in tree-sitter grammar.
//!
//! Go's unit of import is the package (a directory). Resolution is
//! deliberately conservative (ticket 04): an import path resolves only when
//! stripping its module prefix leaves exactly one in-map directory holding
//! Go files — the honest reading of "module-relative paths you can map to
//! in-repo directories" without duplicating a full go.mod resolver. The
//! resolved edge points at the package's anchor file (`<dir>/<dir>.go` when
//! present, else the first Go file in the directory). Stdlib and external
//! modules never resolve. Same-package calls need no import at all —
//! [`Parser::directory_shares_scope`] tells the resolver so.

use std::collections::HashSet;

use tree_sitter::Node as TsNode;

use super::{Analysis, Call, Import, Parser, Symbol, SymbolKind};

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
        let mut analysis = Analysis::default();
        collect(
            tree.root_node(),
            source.as_bytes(),
            Ctx { enclosing_fn: None },
            &mut analysis,
        );
        analysis
    }

    fn resolve_import(
        &self,
        importer: &str,
        specifier: &str,
        files: &HashSet<String>,
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

        // Module path: strip 1..n-1 leading segments (the module name) and
        // keep the result only when it names exactly one in-map package.
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

#[derive(Clone, Copy)]
struct Ctx {
    /// Index into `Analysis::symbols` of the innermost enclosing function.
    enclosing_fn: Option<usize>,
}

fn collect(node: TsNode, source: &[u8], ctx: Ctx, out: &mut Analysis) {
    match node.kind() {
        "import_spec" => {
            if let Some(path) = node
                .child_by_field_name("path")
                .and_then(|n| n.utf8_text(source).ok())
            {
                out.imports.push(Import {
                    specifier: path.trim_matches(['"', '`']).to_string(),
                    // Package members are reached via selector expressions
                    // (`util.Format`), which V1 call resolution skips, so no
                    // name bindings.
                    names: Vec::new(),
                });
            }
            return;
        }
        "call_expression" => {
            if let (Some(callee), Some(caller_idx)) = (
                node.child_by_field_name("function")
                    .filter(|f| f.kind() == "identifier")
                    .and_then(|f| f.utf8_text(source).ok()),
                ctx.enclosing_fn,
            ) {
                out.calls.push(Call {
                    caller: out.symbols[caller_idx].name.clone(),
                    callee: callee.to_string(),
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
        out.symbols.push(Symbol {
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
            pushed_fn = Some(out.symbols.len() - 1);
        }
    }

    let child_ctx = Ctx {
        enclosing_fn: pushed_fn.or(ctx.enclosing_fn),
    };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, source, child_ctx, out);
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
