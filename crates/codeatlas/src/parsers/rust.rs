//! Rust extraction via the compiled-in tree-sitter grammar.
//!
//! Import resolution is deliberately conservative (ticket 04): `mod foo;`
//! resolves to the sibling `foo.rs` / `foo/mod.rs`, and `crate::` / `self::` /
//! `super::` use-paths resolve against the enclosing `src/` layout by trying
//! progressively shorter module prefixes.
//!
//! A path may also name a crate outright (ticket 18) — `use codeatlas::map`
//! from an integration test, or one workspace member naming another — which
//! resolves against the crate roots in the scanned tree. That one form falls
//! back to the named crate's root module when no submodule matches, because
//! what follows a crate name is as often a re-exported item as a module; see
//! [`Rust::resolve_import`] for the full candidate order.
//!
//! Everything else — external crates, `std`, paths that land on no scanned
//! file — is dropped, never dangling.

use std::collections::HashSet;
use std::path::Path;

use tree_sitter::Node as TsNode;

use super::{Analysis, Call, Import, ImportedName, Parser, Symbol, SymbolKind};

/// Marker prefix distinguishing `mod foo;` declarations from use-paths in the
/// import specifier channel.
const MOD_PREFIX: &str = "mod ";

/// Cargo's manifest, which is where a crate's name actually lives.
const MANIFEST: &str = "Cargo.toml";

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

    /// Candidate order, fixed so that a path with several possible answers
    /// always produces the same one:
    ///
    /// 1. `mod foo;` — the sibling file or directory declaring the module.
    /// 2. `crate::` / `self::` / `super::` — anchored in the importer's own
    ///    crate by walking the enclosing `src/` layout.
    /// 3. A crate in the scanned tree whose name is the path's first
    ///    segment: the importer's own crate named from an integration test,
    ///    or a workspace sibling. See [`src_dir_of_crate`].
    /// 4. Everything else — external crates, `std` — resolves to nothing,
    ///    never to a guess.
    fn resolve_import(
        &self,
        importer: &str,
        specifier: &str,
        files: &HashSet<String>,
        root: &std::path::Path,
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
            // A crate named outright. `crate::` cannot cross the tests/
            // boundary, so an integration test has no other way to name its
            // own library, and neither has one workspace member naming
            // another. Crates outside the scanned tree find nothing here.
            crate_name => {
                let src = src_dir_of_crate(importer, crate_name, files, root)?;
                segments.remove(0);
                if segments.is_empty() {
                    return module_file(&src, files);
                }
                // What follows a crate name is as likely to be an item as a
                // module: `pub use` in lib.rs is the ordinary way a crate
                // publishes anything, so `atlas_tools::helper` names a
                // function in the root module, not `helper.rs`. Falling back
                // to the root module keeps the dependency visible rather
                // than dropping it for being re-exported.
                //
                // Deliberately not done for `crate::`/`self::`/`super::`: an
                // importer already inside the crate has no separate file to
                // fall back to, and pointing it at its own root module would
                // manufacture noise rather than recover a real edge.
                return resolve_segments(&src, &segments, files)
                    .or_else(|| module_file(&src, files));
            }
        };
        segments.remove(0);
        if segments.is_empty() {
            return module_file(&base, files);
        }
        resolve_segments(&base, &segments, files)
    }
}

/// The `src` directory of the scanned crate called `name`, if the tree holds
/// one.
///
/// Names are compared after Cargo's own normalisation, since the package
/// `atlas-engine` is the path segment `atlas_engine`. A crate that is not in
/// the scanned tree — `serde`, `std` — matches no candidate and so resolves
/// to nothing, which is the point: the map only ever points at files it
/// contains.
///
/// A scanned crate whose name collides with an external one wins,
/// deliberately: the alternative is a denylist of every name on crates.io,
/// and an edge to a file the reader can open beats no edge.
///
/// One tree can hold several crates of one name — vendored copies, or the
/// miniature fixture repositories this very repository keeps under
/// `tests/fixtures/`. The nearest one to the importer wins, measured in
/// shared leading path segments, so a workspace resolves within itself
/// instead of reaching into a vendored namesake. Exact ties fall back to path
/// order, which makes the answer the same on every run.
fn src_dir_of_crate(
    importer: &str,
    name: &str,
    files: &HashSet<String>,
    root: &Path,
) -> Option<String> {
    let wanted = normalized_crate_name(name);
    crate_src_dirs(files)
        .into_iter()
        .filter(|src| {
            crate_name_of(src, root).is_some_and(|found| normalized_crate_name(&found) == wanted)
        })
        .max_by_key(|src| {
            // `max_by_key` keeps the last maximum, so the path order that
            // breaks ties has to be reversed to leave the earliest path
            // winning.
            (
                shared_segments(importer, src),
                std::cmp::Reverse(src.clone()),
            )
        })
}

/// How many leading `/`-separated segments two paths agree on — how near one
/// file is to another in the tree.
fn shared_segments(one: &str, other: &str) -> usize {
    one.split('/')
        .zip(other.split('/'))
        .take_while(|(a, b)| a == b)
        .count()
}

/// Every `src` directory in the scanned tree that a crate root sits on — one
/// holding `lib.rs` or `main.rs`. Sorted and deduplicated for determinism.
fn crate_src_dirs(files: &HashSet<String>) -> Vec<String> {
    let mut dirs: Vec<String> = files
        .iter()
        .filter_map(|path| {
            let src = path
                .strip_suffix("/lib.rs")
                .or_else(|| path.strip_suffix("/main.rs"))?;
            // The last segment, not a suffix match: `foo/notsrc/lib.rs` is
            // not a crate root.
            (src.rsplit('/').next() == Some("src")).then(|| src.to_string())
        })
        .collect();
    dirs.sort();
    dirs.dedup();
    dirs
}

/// The crate name a `src` directory publishes: its manifest's `[package]
/// name`, falling back to the name of the directory the `src` sits in.
///
/// The manifest has to be authoritative, because a package's name is free to
/// differ from its directory and only the manifest knows. The fallback covers
/// a crate whose manifest was not scanned; it cannot cover a crate at the
/// scan root, which has no directory name to fall back on — the commonest
/// Rust layout of all, and the reason this reads the file at all.
fn crate_name_of(src_dir: &str, root: &Path) -> Option<String> {
    let crate_dir = src_dir.strip_suffix("src")?.trim_end_matches('/');
    let manifest = if crate_dir.is_empty() {
        MANIFEST.to_string()
    } else {
        format!("{crate_dir}/{MANIFEST}")
    };
    std::fs::read_to_string(root.join(manifest))
        .ok()
        .and_then(|toml| package_name(&toml))
        .or_else(|| {
            crate_dir
                .rsplit('/')
                .next()
                .filter(|dir| !dir.is_empty())
                .map(str::to_string)
        })
}

/// `name` from the manifest's `[package]` table.
///
/// Hand-rolled rather than a TOML dependency: one key from one table does not
/// justify widening the audit surface ADR-0006 exists to keep narrow. Only
/// the quoted form Cargo writes is understood; anything else falls back to
/// the directory name in [`crate_name_of`].
fn package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    manifest.lines().find_map(|line| {
        let line = line.trim();
        if let Some(header) = line.strip_prefix('[') {
            // `[package]` exactly — `[package.metadata]` is another table.
            in_package = header.starts_with("package]");
            return None;
        }
        in_package.then(|| quoted_name(line)).flatten()
    })
}

/// `name = "atlas-engine"` → `atlas-engine`. Any other key, or an unquoted
/// value, is not this line's business.
fn quoted_name(line: &str) -> Option<String> {
    let value = line.strip_prefix("name")?.trim_start().strip_prefix('=')?;
    let inner = value.trim_start().strip_prefix('"')?;
    Some(inner[..inner.find('"')?].to_string())
}

/// Cargo's normalisation: the package `atlas-engine` is written
/// `atlas_engine` in a path.
fn normalized_crate_name(crate_name: &str) -> String {
    crate_name.replace('-', "_")
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
