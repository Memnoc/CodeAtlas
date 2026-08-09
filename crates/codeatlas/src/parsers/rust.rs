//! Rust extraction via the compiled-in tree-sitter grammar.
//!
//! Import resolution is deliberately conservative (ticket 04): `mod foo;`
//! resolves to the sibling `foo.rs` / `foo/mod.rs`, and `crate::` / `self::` /
//! `super::` use-paths resolve against the enclosing `src/` layout by trying
//! progressively shorter module prefixes. Anything else — external crates,
//! `std`, paths that land on no scanned file — is dropped, never dangling.

use std::collections::HashSet;

use tree_sitter::Node as TsNode;

use super::{Analysis, Call, Import, ImportedName, Parser, Symbol, SymbolKind};

/// Marker prefix distinguishing `mod foo;` declarations from use-paths in the
/// import specifier channel.
const MOD_PREFIX: &str = "mod ";

pub(super) struct Rust;

pub(super) fn parsers() -> Vec<Box<dyn Parser>> {
    vec![Box::new(Rust)]
}

impl Parser for Rust {
    fn language_name(&self) -> &'static str {
        "Rust"
    }

    fn extensions(&self) -> &'static [&'static str] {
        &["rs"]
    }

    fn parse(&self, source: &str) -> Analysis {
        let mut parser = tree_sitter::Parser::new();
        if parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
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
        if let Some(name) = specifier.strip_prefix(MOD_PREFIX) {
            // `mod foo;` declares a child module: sibling file or directory.
            return resolve_segments(&children_dir(importer), &[name], files);
        }
        let mut segments: Vec<&str> = specifier.split("::").collect();
        let base = match *segments.first()? {
            "crate" => src_root(importer)?,
            "self" => children_dir(importer),
            "super" => {
                // Consume any run of leading `super`s, each one module up.
                let mut dir = children_dir(importer);
                while segments.first() == Some(&"super") {
                    segments.remove(0);
                    let (parent, _) = dir.rsplit_once('/').unwrap_or(("", &dir));
                    dir = parent.to_string();
                }
                if segments.is_empty() {
                    // `use super::{..}`: the parent module file itself.
                    return module_file(&dir, files);
                }
                segments.insert(0, "super"); // uniform removal below
                dir
            }
            // External crates, std, and 2015-style bare paths never resolve.
            _ => return None,
        };
        segments.remove(0);
        if segments.is_empty() {
            return module_file(&base, files);
        }
        resolve_segments(&base, &segments, files)
    }
}

/// The directory where child modules of `file` live: `mod.rs` / `lib.rs` /
/// `main.rs` own their directory, any other file owns its stem directory.
fn children_dir(file: &str) -> String {
    let (dir, name) = file.rsplit_once('/').unwrap_or(("", file));
    if matches!(name, "mod.rs" | "lib.rs" | "main.rs") {
        dir.to_string()
    } else {
        file.strip_suffix(".rs").unwrap_or(file).to_string()
    }
}

/// The nearest ancestor directory named `src` — the crate-root layout
/// `crate::` paths resolve against. `None` when the importer sits under no
/// `src/` (resolution is then declined rather than guessed).
fn src_root(importer: &str) -> Option<String> {
    let mut parts: Vec<&str> = importer.split('/').collect();
    parts.pop(); // the file itself
    while let Some(last) = parts.last() {
        if *last == "src" {
            return Some(parts.join("/"));
        }
        parts.pop();
    }
    None
}

/// The file defining the module whose children live in `dir`.
fn module_file(dir: &str, files: &HashSet<String>) -> Option<String> {
    for name in ["mod.rs", "lib.rs", "main.rs"] {
        let candidate = if dir.is_empty() {
            name.to_string()
        } else {
            format!("{dir}/{name}")
        };
        if files.contains(&candidate) {
            return Some(candidate);
        }
    }
    let sibling = format!("{dir}.rs");
    files.contains(&sibling).then_some(sibling)
}

/// Resolves a `::`-path under `base` by trying progressively shorter module
/// prefixes (trailing segments may be items, not modules): for each prefix,
/// `<prefix>.rs` then `<prefix>/mod.rs`.
fn resolve_segments(base: &str, segments: &[&str], files: &HashSet<String>) -> Option<String> {
    for take in (1..=segments.len()).rev() {
        let joined = segments[..take].join("/");
        let stem = if base.is_empty() {
            joined
        } else {
            format!("{base}/{joined}")
        };
        let file = format!("{stem}.rs");
        if files.contains(&file) {
            return Some(file);
        }
        let dir_mod = format!("{stem}/mod.rs");
        if files.contains(&dir_mod) {
            return Some(dir_mod);
        }
    }
    None
}

#[derive(Clone, Copy)]
struct Ctx<'a> {
    /// Enclosing `impl`/`trait` type name, for scope-qualifying methods.
    scope: Option<&'a str>,
    /// Index into `Analysis::symbols` of the innermost enclosing function.
    enclosing_fn: Option<usize>,
}

fn collect(node: TsNode, source: &[u8], ctx: Ctx, out: &mut Analysis) {
    match node.kind() {
        "use_declaration" => {
            if let Some(argument) = node.child_by_field_name("argument") {
                expand_use(argument, source, "", out);
            }
            return; // nothing below a use is a symbol or call
        }
        "mod_item" if node.child_by_field_name("body").is_none() => {
            // `mod foo;` — a file-form child module declaration.
            if let Some(name) = field_text(node, "name", source) {
                out.imports.push(Import {
                    specifier: format!("{MOD_PREFIX}{name}"),
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

    let kind = match node.kind() {
        "function_item" => Some(SymbolKind::Function),
        "struct_item" => Some(SymbolKind::Class),
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
            // `pub` at the top level exports; methods of an impl are not
            // themselves exported (mirrors the TS/JS convention for class
            // methods).
            exported: ctx.scope.is_none() && has_pub(node),
        });
        if kind == SymbolKind::Function {
            pushed_fn = Some(out.symbols.len() - 1);
        }
    }

    // `impl Circle { .. }` / `impl Trait for Circle { .. }` / `trait T { .. }`
    // scope-qualify the functions inside them.
    let scope_name = match node.kind() {
        "impl_item" => field_text(node, "type", source).map(strip_generics),
        "trait_item" => field_text(node, "name", source),
        _ => None,
    };

    let child_ctx = Ctx {
        scope: None, // set below via owned string handling
        enclosing_fn: pushed_fn.or(ctx.enclosing_fn),
    };
    let mut cursor = node.walk();
    for child in node.children(&mut cursor) {
        let ctx_for_child = Ctx {
            scope: scope_name.as_deref().or(ctx.scope),
            ..child_ctx
        };
        collect(child, source, ctx_for_child, out);
    }
}

/// Expands a `use` argument into one [`Import`] per leaf path:
/// `use crate::util::{greet, fmt as f};` yields two imports whose specifiers
/// carry the full path and whose names bind the leaf (or its alias).
fn expand_use(node: TsNode, source: &[u8], prefix: &str, out: &mut Analysis) {
    match node.kind() {
        "scoped_use_list" => {
            let joined = join_path(
                prefix,
                &field_text(node, "path", source).unwrap_or_default(),
            );
            if let Some(list) = node.child_by_field_name("list") {
                expand_use(list, source, &joined, out);
            }
        }
        "use_list" => {
            let mut cursor = node.walk();
            for child in node.named_children(&mut cursor) {
                expand_use(child, source, prefix, out);
            }
        }
        "use_as_clause" => {
            let path = field_text(node, "path", source).unwrap_or_default();
            let full = join_path(prefix, &path);
            let alias = field_text(node, "alias", source).unwrap_or_default();
            out.imports.push(Import {
                names: vec![ImportedName {
                    local: alias,
                    imported: leaf_of(&full).to_string(),
                }],
                specifier: full,
            });
        }
        "use_wildcard" => {
            // `use crate::util::*` — file-level edge only, no name bindings.
            let path = node
                .named_child(0)
                .and_then(|c| c.utf8_text(source).ok())
                .unwrap_or_default();
            out.imports.push(Import {
                specifier: join_path(prefix, path),
                names: Vec::new(),
            });
        }
        _ => {
            // identifier, scoped_identifier, crate, self, super
            let Ok(text) = node.utf8_text(source) else {
                return;
            };
            let full = join_path(prefix, &text.replace(char::is_whitespace, ""));
            let leaf = leaf_of(&full).to_string();
            out.imports.push(Import {
                specifier: full,
                names: vec![ImportedName {
                    local: leaf.clone(),
                    imported: leaf,
                }],
            });
        }
    }
}

fn join_path(prefix: &str, path: &str) -> String {
    if prefix.is_empty() {
        path.to_string()
    } else {
        format!("{prefix}::{path}")
    }
}

fn leaf_of(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

fn has_pub(node: TsNode) -> bool {
    let mut cursor = node.walk();
    node.children(&mut cursor)
        .any(|c| c.kind() == "visibility_modifier")
}

fn field_text(node: TsNode, field: &str, source: &[u8]) -> Option<String> {
    node.child_by_field_name(field)
        .and_then(|n| n.utf8_text(source).ok())
        .map(|s| s.to_string())
}

/// `Circle<T>` → `Circle`: the scope name drops generic arguments.
fn strip_generics(name: String) -> String {
    match name.split_once('<') {
        Some((base, _)) => base.trim().to_string(),
        None => name,
    }
}
