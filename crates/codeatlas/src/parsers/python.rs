//! Python extraction via the compiled-in tree-sitter grammar.
//!
//! Import resolution covers relative imports (`from .x import y`) and
//! absolute module paths that land on files inside the map — resolved from
//! the repository root first, then as siblings of the importer (script
//! style). `__init__.py` is the package's index-file analog, and a package
//! that has none is a namespace package (PEP 420), which resolves through
//! its modules alone. In `from pkg import util` the bound name may itself be
//! a module rather than a symbol; see [`Python::resolve_name_as_module`] for
//! the candidate order. External and stdlib modules never resolve.

use std::collections::HashSet;

use tree_sitter::Node as TsNode;

use super::{Analysis, Call, Import, ImportedName, Parser, Symbol, SymbolKind};

pub(super) struct Python;

pub(super) fn parsers() -> Vec<Box<dyn Parser>> {
    vec![Box::new(Python)]
}

impl Parser for Python {
    fn language_name(&self) -> &'static str {
        "Python"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["py"]
    }

    fn parse(&self, source: &str) -> Analysis {
        let mut parser = tree_sitter::Parser::new();
        if parser
            .set_language(&tree_sitter_python::LANGUAGE.into())
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
            Ctx {
                scope: None,
                enclosing_fn: None,
                module_level: true,
            },
            &mut analysis,
        );
        analysis
    }

    fn resolve_import(
        &self,
        importer: &str,
        specifier: &str,
        files: &HashSet<String>,
        _root: &std::path::Path,
    ) -> Option<String> {
        let dots = specifier.chars().take_while(|c| *c == '.').count();
        let rest: Vec<&str> = specifier[dots..]
            .split('.')
            .filter(|s| !s.is_empty())
            .collect();

        if dots > 0 {
            // Relative: one dot is the importer's package, each further dot
            // one package up.
            let mut parts: Vec<&str> = importer.split('/').collect();
            parts.pop(); // the file itself
            for _ in 1..dots {
                parts.pop()?; // escaping the repo root is unresolvable
            }
            return resolve_module(&parts.join("/"), &rest, files);
        }
        // Absolute: from the repository root, then script-style from the
        // importer's own directory.
        if let Some(found) = resolve_module("", &rest, files) {
            return Some(found);
        }
        let dir = importer.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        if dir.is_empty() {
            return None; // root-relative already tried
        }
        resolve_module(dir, &rest, files)
    }

    /// `from pkg import util` binds either the module `pkg/util.py` or a
    /// symbol defined in `pkg`, and the statement does not say which. The
    /// candidate order is **module before package**: the name is tried as a
    /// submodule of the specifier, and only a miss falls back to the
    /// specifier alone. Both candidates keep the anchor order
    /// [`Python::resolve_import`] already uses — repository root, then
    /// script-style beside the importer.
    ///
    /// Module first, for two reasons. It is the only order that produces an
    /// edge at all for a PEP 420 namespace package, where there is no
    /// `__init__.py` to fall back to. And where both exist it is the answer
    /// a reader wants: tracing who uses `pkg/util.py` is the question the
    /// map is for, while the package initialiser is a waypoint.
    ///
    /// One edge, not two, even though the statement really does execute
    /// `pkg/__init__.py` as well. That matches what this resolver already
    /// does for `from pkg.util import helper`, which reaches `pkg/util.py`
    /// alone and never records the package chain it walked through.
    fn resolve_name_as_module(
        &self,
        importer: &str,
        specifier: &str,
        name: &str,
        files: &HashSet<String>,
        root: &std::path::Path,
    ) -> Option<String> {
        self.resolve_import(importer, &submodule_of(specifier, name), files, root)
    }
}

/// The specifier naming `name` as a submodule: `pkg` + `util` is `pkg.util`.
/// A relative specifier already ends in the separator — `from . import util`
/// is `.util`, and adding a dot would read as one package further up.
fn submodule_of(specifier: &str, name: &str) -> String {
    if specifier.ends_with('.') {
        format!("{specifier}{name}")
    } else {
        format!("{specifier}.{name}")
    }
}

/// Resolves module path segments under `base`: `<path>.py`, then the package
/// form `<path>/__init__.py`; an empty path is the package itself.
fn resolve_module(base: &str, segments: &[&str], files: &HashSet<String>) -> Option<String> {
    let stem = if segments.is_empty() {
        base.to_string()
    } else if base.is_empty() {
        segments.join("/")
    } else {
        format!("{base}/{}", segments.join("/"))
    };
    if !segments.is_empty() {
        let module = format!("{stem}.py");
        if files.contains(&module) {
            return Some(module);
        }
    }
    let package = if stem.is_empty() {
        "__init__.py".to_string()
    } else {
        format!("{stem}/__init__.py")
    };
    files.contains(&package).then_some(package)
}

#[derive(Clone, Copy)]
struct Ctx<'a> {
    /// Enclosing class name, for scope-qualifying methods.
    scope: Option<&'a str>,
    /// Index into `Analysis::symbols` of the innermost enclosing function.
    enclosing_fn: Option<usize>,
    /// Whether this node sits at module level (exportability is a top-level
    /// notion).
    module_level: bool,
}

fn collect(node: TsNode, source: &[u8], ctx: Ctx, out: &mut Analysis) {
    match node.kind() {
        "import_statement" => {
            // `import a.b` makes the module reachable as the dotted path
            // itself, so that whole path is what a call site writes:
            // `a.b.f()`. `import a.b as c` replaces it with the alias, and
            // then only `c.f()` is legal. Either way the bound form is the
            // namespace — unlike `from a import b`, which binds `b` and
            // leaves `a` unavailable, and so binds no namespace at all.
            let mut cursor = node.walk();
            for child in node.children_by_field_name("name", &mut cursor) {
                let (module, bound) = match child.kind() {
                    "aliased_import" => (
                        field_text(child, "name", source),
                        field_text(child, "alias", source),
                    ),
                    _ => {
                        let path = child.utf8_text(source).ok().map(str::to_string);
                        (path.clone(), path)
                    }
                };
                if let Some(specifier) = module {
                    out.imports.push(Import {
                        specifier,
                        names: Vec::new(),
                        namespaces: bound.into_iter().collect(),
                    });
                }
            }
            return;
        }
        "import_from_statement" => {
            if let Some(specifier) = field_text(node, "module_name", source) {
                let mut names = Vec::new();
                let mut cursor = node.walk();
                for child in node.children_by_field_name("name", &mut cursor) {
                    match child.kind() {
                        "aliased_import" => {
                            if let (Some(imported), Some(local)) = (
                                field_text(child, "name", source),
                                field_text(child, "alias", source),
                            ) {
                                names.push(ImportedName { local, imported });
                            }
                        }
                        _ => {
                            if let Ok(text) = child.utf8_text(source) {
                                names.push(ImportedName {
                                    local: text.to_string(),
                                    imported: text.to_string(),
                                });
                            }
                        }
                    }
                }
                out.imports.push(Import {
                    specifier,
                    names,
                    namespaces: Vec::new(),
                });
            }
            return;
        }
        "call" => {
            if let (Some(function), Some(caller_idx)) =
                (node.child_by_field_name("function"), ctx.enclosing_fn)
            {
                // `f()` is an identifier; `util.helper()` and
                // `pkg.util.other()` are attributes whose object chain is the
                // receiver. `self.method()` and `obj.method()` are attributes
                // too — they simply bind to no module and drop.
                let call = match function.kind() {
                    "identifier" => function
                        .utf8_text(source)
                        .ok()
                        .map(|callee| (callee.to_string(), Vec::new())),
                    "attribute" => attribute_parts(function, source),
                    _ => None,
                };
                if let Some((callee, receiver)) = call {
                    out.calls.push(Call {
                        caller: out.symbols[caller_idx].name.clone(),
                        callee,
                        receiver,
                    });
                }
            }
        }
        _ => {}
    }

    let kind = match node.kind() {
        "function_definition" => Some(SymbolKind::Function),
        "class_definition" => Some(SymbolKind::Class),
        _ => None,
    };
    let name = field_text(node, "name", source);

    let mut pushed_fn = None;
    if let (Some(kind), Some(name)) = (kind, name.as_deref()) {
        let qualified = match (kind, ctx.scope) {
            (SymbolKind::Function, Some(scope)) => format!("{scope}.{name}"),
            _ => name.to_string(),
        };
        out.symbols.push(Symbol {
            kind,
            name: qualified,
            start_line: node.start_position().row as u32 + 1,
            end_line: node.end_position().row as u32 + 1,
            // Convention: a module exports its top-level non-underscore
            // names; methods and nested definitions are not exported.
            exported: ctx.module_level && ctx.scope.is_none() && !name.starts_with('_'),
        });
        if kind == SymbolKind::Function {
            pushed_fn = Some(out.symbols.len() - 1);
        }
    }

    let class_scope = match node.kind() {
        "class_definition" => name.as_deref(),
        _ => None,
    };
    let child_ctx = Ctx {
        scope: class_scope.or(ctx.scope),
        enclosing_fn: pushed_fn.or(ctx.enclosing_fn),
        // Only definitions deepen nesting; decorated_definition and plain
        // statements keep the current level.
        module_level: match node.kind() {
            "function_definition" | "class_definition" => false,
            _ => ctx.module_level,
        },
    };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        collect(child, source, child_ctx, out);
    }
}

/// Splits an `attribute` call target into its final name and the dotted
/// receiver in front of it: `pkg.util.other` becomes
/// `("other", ["pkg", "util"])`. `None` when the chain bottoms out in
/// anything but plain identifiers — `f().g()`, `d["k"].g()`.
///
/// A receiver that *is* a plain identifier still comes back here, whether or
/// not it names a module: `self.g()` yields `("g", ["self"])`, and
/// `logger.info()` yields `("info", ["logger"])`. Deciding which of those is
/// a module is resolution's job, not the parser's, and it decides by asking
/// whether an import bound the name — so a value never resolves to anything.
fn attribute_parts(node: TsNode, source: &[u8]) -> Option<(String, Vec<String>)> {
    let name = field_text(node, "attribute", source)?;
    let mut receiver = Vec::new();
    let mut object = node.child_by_field_name("object");
    while let Some(segment) = object {
        match segment.kind() {
            "identifier" => {
                receiver.push(segment.utf8_text(source).ok()?.to_string());
                break;
            }
            "attribute" => {
                receiver.push(field_text(segment, "attribute", source)?);
                object = segment.child_by_field_name("object");
            }
            _ => return None,
        }
    }
    receiver.reverse();
    Some((name, receiver))
}

fn field_text(node: TsNode, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|n| n.utf8_text(source).ok())
        .map(|s| s.to_string())
}
