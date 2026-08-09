//! TypeScript / JavaScript extraction via compiled-in tree-sitter grammars.

use std::collections::HashSet;

use tree_sitter::{Language, Node as TsNode};

use super::{Analysis, Call, Import, ImportedName, Parser, Symbol, SymbolKind};

/// Extensions tried, in order, when an import specifier omits one.
const RESOLVE_EXTENSIONS: &[&str] = &["ts", "tsx", "js", "jsx", "mjs", "cjs"];

struct TsJs {
    name: &'static str,
    extensions: &'static [&'static str],
    language: Language,
}

pub(super) fn parsers() -> Vec<Box<dyn Parser>> {
    vec![
        Box::new(TsJs {
            name: "TypeScript",
            extensions: &["ts"],
            language: tree_sitter_typescript::LANGUAGE_TYPESCRIPT.into(),
        }),
        Box::new(TsJs {
            name: "TypeScript",
            extensions: &["tsx"],
            language: tree_sitter_typescript::LANGUAGE_TSX.into(),
        }),
        Box::new(TsJs {
            name: "JavaScript",
            extensions: &["js", "jsx", "mjs", "cjs"],
            language: tree_sitter_javascript::LANGUAGE.into(),
        }),
    ]
}

impl Parser for TsJs {
    fn language_name(&self) -> &'static str {
        self.name
    }

    fn extensions(&self) -> &'static [&'static str] {
        self.extensions
    }

    fn parse(&self, source: &str) -> Analysis {
        let mut parser = tree_sitter::Parser::new();
        if parser.set_language(&self.language).is_err() {
            return Analysis::default();
        }
        let Some(tree) = parser.parse(source, None) else {
            return Analysis::default();
        };
        let mut analysis = Analysis::default();
        let mut export_clause_names = Vec::new();
        collect(
            tree.root_node(),
            source.as_bytes(),
            Ctx {
                scope: None,
                exported: false,
                enclosing_fn: None,
            },
            &mut analysis,
            &mut export_clause_names,
        );
        // `export { name }` clauses export symbols declared elsewhere in the
        // file; mark them after the walk.
        for symbol in &mut analysis.symbols {
            if export_clause_names.iter().any(|n| n == &symbol.name) {
                symbol.exported = true;
            }
        }
        analysis
    }

    /// ES-module resolution against the scanned file set: relative
    /// specifiers only, with extension inference and the index-file
    /// convention. Bare package names never resolve — packages are not part
    /// of the map.
    fn resolve_import(
        &self,
        importer: &str,
        specifier: &str,
        files: &HashSet<String>,
    ) -> Option<String> {
        if !specifier.starts_with("./") && !specifier.starts_with("../") {
            return None;
        }
        let mut parts: Vec<&str> = importer.split('/').collect();
        parts.pop(); // drop the importing file's name
        for segment in specifier.split('/') {
            match segment {
                "" | "." => {}
                ".." => {
                    parts.pop()?; // escaping the repo root is unresolvable
                }
                s => parts.push(s),
            }
        }
        let base = parts.join("/");
        if files.contains(&base) {
            return Some(base);
        }
        for ext in RESOLVE_EXTENSIONS {
            let candidate = format!("{base}.{ext}");
            if files.contains(&candidate) {
                return Some(candidate);
            }
        }
        for ext in RESOLVE_EXTENSIONS {
            let candidate = format!("{base}/index.{ext}");
            if files.contains(&candidate) {
                return Some(candidate);
            }
        }
        None
    }
}

/// Walk state threaded down the tree.
#[derive(Clone, Copy)]
struct Ctx<'a> {
    /// Enclosing class name, for scope-qualifying members.
    scope: Option<&'a str>,
    /// Whether this node sits directly under an `export` modifier.
    exported: bool,
    /// Index into `Analysis::symbols` of the innermost enclosing function —
    /// the caller of any invocation found below.
    enclosing_fn: Option<usize>,
}

fn collect(
    node: TsNode,
    source: &[u8],
    ctx: Ctx,
    out: &mut Analysis,
    export_clause_names: &mut Vec<String>,
) {
    if node.kind() == "import_statement"
        && let Some(specifier) = string_text(node.child_by_field_name("source"), source)
    {
        let mut names = Vec::new();
        import_bindings(node, source, &mut names);
        out.imports.push(Import { specifier, names });
    }

    // A plain-identifier invocation inside a function body is a call the
    // resolver may connect. Member calls (`obj.method()`) are out of scope
    // for V1 — their receiver types are unknowable structurally.
    if node.kind() == "call_expression"
        && let (Some(callee), Some(caller_idx)) = (
            node.child_by_field_name("function")
                .filter(|f| f.kind() == "identifier")
                .and_then(|f| f.utf8_text(source).ok()),
            ctx.enclosing_fn,
        )
    {
        out.calls.push(Call {
            caller: out.symbols[caller_idx].name.clone(),
            callee: callee.to_string(),
        });
    }

    if node.kind() == "export_statement" {
        collect_export_clause(node, source, out, export_clause_names);
    }

    let kind = match node.kind() {
        "function_declaration" | "generator_function_declaration" | "method_definition" => {
            Some(SymbolKind::Function)
        }
        // Decision (ticket 03): `const x = () => {}` and
        // `const x = function () {}` count as function declarations —
        // idiomatic TS declares many functions this way, and skipping them
        // undercounts real code.
        "variable_declarator" if is_function_value(node) => Some(SymbolKind::Function),
        "class_declaration" => Some(SymbolKind::Class),
        _ => None,
    };
    let name = node
        .child_by_field_name("name")
        .and_then(|n| n.utf8_text(source).ok());

    let mut pushed_fn = None;
    if let (Some(kind), Some(name)) = (kind, name) {
        // Members are scope-qualified (`Alpha.run`) so same-named symbols in
        // different scopes get distinct node IDs.
        let qualified = match (kind, ctx.scope) {
            (SymbolKind::Function, Some(scope)) => format!("{scope}.{name}"),
            _ => name.to_string(),
        };
        out.symbols.push(Symbol {
            kind,
            name: qualified,
            start_line: node.start_position().row as u32 + 1,
            end_line: node.end_position().row as u32 + 1,
            exported: ctx.exported,
        });
        if kind == SymbolKind::Function {
            pushed_fn = Some(out.symbols.len() - 1);
        }
    }

    let child_ctx = Ctx {
        scope: if node.kind() == "class_declaration" {
            name
        } else {
            ctx.scope
        },
        // The export modifier reaches the declaration directly under it —
        // through a `const`/`let`/`var` statement down to its declarators —
        // and no further (a class body is not exported member by member).
        exported: match node.kind() {
            "export_statement" => true,
            "lexical_declaration" | "variable_declaration" => ctx.exported,
            _ => false,
        },
        enclosing_fn: pushed_fn.or(ctx.enclosing_fn),
    };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, source, child_ctx, out, export_clause_names);
    }
}

/// Handles an export statement's clause and source, if any.
///
/// - `export { local }` / `export { local as alias }` marks the local symbol
///   exported and records the alias indirection.
/// - `export { x } from "./y"` is a re-export: the file depends on `./y`
///   (an import edge) and importers of `x` are pointed one level onward.
///   Deeper barrel chains (a barrel re-exporting another barrel's
///   re-export) are out of scope: resolution follows exactly one hop.
/// - `export * from "./y"` records only the file-level dependency — the
///   names it forwards are unknowable without reading `./y`.
fn collect_export_clause(
    node: TsNode,
    source: &[u8],
    out: &mut Analysis,
    export_clause_names: &mut Vec<String>,
) {
    let from = string_text(node.child_by_field_name("source"), source);
    if let Some(specifier) = from.clone() {
        out.imports.push(Import {
            specifier,
            names: Vec::new(),
        });
    }
    let mut clause_cursor = node.walk();
    let Some(clause) = node
        .children(&mut clause_cursor)
        .find(|c| c.kind() == "export_clause")
    else {
        return;
    };
    let mut cursor = clause.walk();
    for specifier in clause.named_children(&mut cursor) {
        if specifier.kind() != "export_specifier" {
            continue;
        }
        let Some(name) = specifier
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source).ok())
        else {
            continue;
        };
        let alias = specifier
            .child_by_field_name("alias")
            .and_then(|n| n.utf8_text(source).ok());
        if from.is_none() {
            // The local symbol named by the clause is exported.
            export_clause_names.push(name.to_string());
        }
        out.reexports.push(super::Reexport {
            exported: alias.unwrap_or(name).to_string(),
            local: name.to_string(),
            specifier: from.clone(),
        });
    }
}

/// Collects the named bindings under an import statement:
/// `import { imported as local }`. Default and namespace imports introduce
/// bindings the resolver cannot match to a named export, so they are skipped
/// here — the file-level import edge still gets emitted.
fn import_bindings(node: TsNode, source: &[u8], out: &mut Vec<ImportedName>) {
    if node.kind() == "import_specifier" {
        if let Some(imported) = node
            .child_by_field_name("name")
            .and_then(|n| n.utf8_text(source).ok())
        {
            let local = node
                .child_by_field_name("alias")
                .and_then(|n| n.utf8_text(source).ok())
                .unwrap_or(imported);
            out.push(ImportedName {
                local: local.to_string(),
                imported: imported.to_string(),
            });
        }
        return;
    }
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        import_bindings(child, source, out);
    }
}

/// Is this variable declarator initialized with a function value?
fn is_function_value(declarator: TsNode) -> bool {
    declarator.child_by_field_name("value").is_some_and(|v| {
        matches!(
            v.kind(),
            "arrow_function" | "function_expression" | "generator_function"
        )
    })
}

/// The unquoted text of a string literal node, e.g. `"./util"` → `./util`.
fn string_text(node: Option<TsNode>, source: &[u8]) -> Option<String> {
    let node = node?;
    let text = node.utf8_text(source).ok()?;
    Some(text.trim_matches(['"', '\'']).to_string())
}
