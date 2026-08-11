use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use ignore::WalkBuilder;

use crate::map::{
    Edge, EdgeKind, KnowledgeGraph, MAP_CONTRACT_VERSION, Node, NodeId, NodeKind, Project,
    Provenance, Range,
};
use crate::parsers::{self, SymbolKind};

/// Where all CodeAtlas artifacts live, under the scanned root.
pub const OUTPUT_DIR: &str = ".codeatlas";

/// Directories never worth mapping, ignored even when no gitignore says so.
const DEFAULT_EXCLUDES: &[&str] = &["node_modules", "target", ".git", OUTPUT_DIR];

pub fn scan(root: &Path) -> Result<KnowledgeGraph> {
    let root = root
        .canonicalize()
        .with_context(|| format!("cannot scan {}", root.display()))?;

    let mut paths: Vec<String> = WalkBuilder::new(&root)
        .require_git(false)
        // Hidden files are source too; only gitignore and the default
        // excludes drop entries.
        .hidden(false)
        .filter_entry(|entry| {
            let is_dir = entry.file_type().is_some_and(|t| t.is_dir());
            let name = entry.file_name().to_string_lossy();
            !(is_dir && DEFAULT_EXCLUDES.contains(&name.as_ref()))
        })
        .build()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_some_and(|t| t.is_file()))
        .filter_map(|entry| {
            entry
                .path()
                .strip_prefix(&root)
                .ok()
                .map(|p| p.to_string_lossy().replace('\\', "/"))
        })
        .collect();
    paths.sort();

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let facts: Vec<FileFacts> = paths
        .iter()
        .map(|path| extract_file(&root, path, &mut nodes, &mut edges))
        .collect();

    resolve_imports(&paths, &facts, &mut edges, &root);
    resolve_calls(&paths, &facts, &mut edges, &root);

    let project_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".to_string());

    let mut graph = KnowledgeGraph {
        version: MAP_CONTRACT_VERSION.to_string(),
        project: Project { name: project_name },
        nodes,
        edges,
        layers: Vec::new(),
        domain_flows: Vec::new(),
        tour: Vec::new(),
    };
    crate::semantics::apply(&mut graph);
    Ok(graph)
}

/// What one file contributes to the cross-file resolution phase.
struct FileFacts {
    path: String,
    imports: Vec<parsers::Import>,
    calls: Vec<parsers::Call>,
    /// Final (post-dedup) names of this file's function nodes.
    functions: HashSet<String>,
    /// The subset of `functions` this file exports — an imported callee must
    /// come from here, not merely exist in the defining file.
    exported_functions: HashSet<String>,
    /// Export-clause indirections: aliases and one-level re-exports.
    reexports: Vec<parsers::Reexport>,
}

/// Emits the file node and, where a parser handles the language, its symbol
/// nodes and `contains` edges. Extraction failures degrade to a bare file
/// node; they never fail the scan. Returns the facts cross-file resolution
/// needs.
fn extract_file(
    root: &Path,
    path: &str,
    nodes: &mut Vec<Node>,
    edges: &mut Vec<Edge>,
) -> FileFacts {
    let source = fs::read(root.join(path))
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    let line_count = source.lines().count();

    let parser = parsers::for_extension(extension_of(path));
    let analysis = parser.map(|p| p.parse(&source)).unwrap_or_default();
    let mut symbols = analysis.symbols;

    // Safety net: any symbols still sharing a name within this file get
    // deterministic ordinal suffixes, so node IDs are always unique.
    let mut seen: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for symbol in &mut symbols {
        let n = seen.entry(symbol.name.clone()).or_insert(0);
        *n += 1;
        if *n > 1 {
            symbol.name = format!("{}#{n}", symbol.name);
        }
    }

    let functions: HashSet<String> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .map(|s| s.name.clone())
        .collect();
    let exported_functions: HashSet<String> = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function && s.exported)
        .map(|s| s.name.clone())
        .collect();
    let classes = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .count();

    let file_id = NodeId::file(path);
    let file_label = parser.map_or("Plain", |p| p.language_name());
    let mut summary = format!("{file_label} file, {line_count} lines");
    if !symbols.is_empty() {
        summary.push_str(": ");
        let mut parts = Vec::new();
        if !functions.is_empty() {
            let n = functions.len();
            parts.push(format!("{n} {}", plural("function", n)));
        }
        if classes > 0 {
            parts.push(format!("{classes} {}", plural("class", classes)));
        }
        summary.push_str(&parts.join(", "));
    }

    nodes.push(Node {
        id: file_id.clone(),
        kind: NodeKind::File,
        name: path.rsplit('/').next().unwrap_or(path).to_string(),
        path: path.to_string(),
        summary,
        range: None,
        layer: None, // assigned by the semantics pass
        provenance: Provenance::Structural,
    });

    for symbol in symbols {
        let (kind, label) = match symbol.kind {
            SymbolKind::Function => (NodeKind::Function, "Function"),
            SymbolKind::Class => (NodeKind::Class, "Class"),
        };
        let id = NodeId::symbol(kind, path, &symbol.name);
        nodes.push(Node {
            id: id.clone(),
            kind,
            name: symbol.name.clone(),
            path: path.to_string(),
            summary: format!(
                "{label} {}, lines {}-{}",
                symbol.name, symbol.start_line, symbol.end_line
            ),
            range: Some(Range {
                start_line: symbol.start_line,
                end_line: symbol.end_line,
            }),
            layer: None, // symbols inherit their file's layer via containment
            provenance: Provenance::Structural,
        });
        edges.push(Edge::new(file_id.clone(), id.clone(), EdgeKind::Contains));
        if symbol.exported {
            edges.push(Edge::new(file_id.clone(), id, EdgeKind::Exports));
        }
    }

    FileFacts {
        path: path.to_string(),
        imports: analysis.imports,
        calls: analysis.calls,
        functions,
        exported_functions,
        reexports: analysis.reexports,
    }
}

/// Resolves each file's import specifiers against the scanned file set and
/// emits `imports` edges between file nodes. Unresolvable imports (bare
/// packages, files outside the map) are dropped, never emitted dangling.
///
/// One statement may reach more than one file: in Python a bound name can be
/// a module in its own right, so `from pkg import util, api` lands on
/// `pkg/util.py` and `pkg/__init__.py` at once.
fn resolve_imports(paths: &[String], facts: &[FileFacts], edges: &mut Vec<Edge>, root: &Path) {
    let files: HashSet<String> = paths.iter().cloned().collect();
    let mut seen: HashSet<(NodeId, NodeId)> = HashSet::new();
    for file in facts {
        let Some(parser) = parsers::for_extension(extension_of(&file.path)) else {
            continue;
        };
        for import in &file.imports {
            for target in import_targets(parser, &file.path, import, &files, root) {
                let edge = (NodeId::file(&file.path), NodeId::file(&target));
                if seen.insert(edge.clone()) {
                    edges.push(Edge::new(edge.0, edge.1, EdgeKind::Imports));
                }
            }
        }
    }
}

/// Every file one import statement reaches: the module each bound name is
/// in its own right, plus the specifier's own target for every name that is
/// not a module — and for a statement binding no names at all (`import x`,
/// `#include`, a wildcard), which the specifier answers alone.
fn import_targets(
    parser: &dyn parsers::Parser,
    importer: &str,
    import: &parsers::Import,
    files: &HashSet<String>,
    root: &Path,
) -> Vec<String> {
    let mut targets = Vec::new();
    let mut needs_specifier = import.names.is_empty();
    for name in &import.names {
        match parser.resolve_name_as_module(
            importer,
            &import.specifier,
            &name.imported,
            files,
            root,
        ) {
            Some(module) => targets.push(module),
            None => needs_specifier = true,
        }
    }
    if needs_specifier
        && let Some(target) = parser.resolve_import(importer, &import.specifier, files, root)
    {
        targets.push(target);
    }
    targets
}

/// Connects invocations to function nodes and emits `calls` edges. A callee
/// resolves within its own file first, then within its directory where the
/// language shares one namespace per directory (Go packages), then through a
/// named import whose module resolved into the map — and an imported callee
/// must actually be exported by its defining file, possibly through one
/// level of export alias or re-export. Anything else — member calls,
/// packages, files outside the map — is dropped, never emitted dangling.
fn resolve_calls(paths: &[String], facts: &[FileFacts], edges: &mut Vec<Edge>, root: &Path) {
    let files: HashSet<String> = paths.iter().cloned().collect();
    let facts_by_path: HashMap<&str, &FileFacts> =
        facts.iter().map(|f| (f.path.as_str(), f)).collect();

    // Where an import of `name` from `file` actually lands: the file's own
    // exported function, an aliased local (`export { internal as external }`),
    // or — one level deep — the file a re-export clause forwards to.
    let resolve_exported = |file: &str, name: &str| -> Option<(String, String)> {
        let target = facts_by_path.get(file)?;
        if target.exported_functions.contains(name) {
            return Some((file.to_string(), name.to_string()));
        }
        let reexport = target.reexports.iter().find(|r| r.exported == name)?;
        match &reexport.specifier {
            // Alias of a local symbol; the clause itself exports it.
            None => target
                .exported_functions
                .contains(&reexport.local)
                .then(|| (file.to_string(), reexport.local.clone())),
            // Re-export from another module: follow exactly one hop; the
            // defining file must export the name itself (deeper barrel
            // chains stay unresolved).
            Some(specifier) => {
                let parser = parsers::for_extension(extension_of(file))?;
                let defining = parser.resolve_import(file, specifier, &files, root)?;
                facts_by_path
                    .get(defining.as_str())?
                    .exported_functions
                    .contains(&reexport.local)
                    .then(|| (defining, reexport.local.clone()))
            }
        }
    };

    let mut seen: HashSet<(NodeId, NodeId)> = HashSet::new();
    for file in facts {
        let Some(parser) = parsers::for_extension(extension_of(&file.path)) else {
            continue;
        };
        // Local binding name → candidate (defining file, exported name)
        // pairs, in import order. Most languages bind a name once; C-family
        // includes offer every unresolved callee to every header, so a name
        // may carry several candidates and resolution keeps the first that
        // works, trying later bindings first (a later import shadows an
        // earlier one, matching source semantics).
        let mut bindings: HashMap<&str, Vec<(String, &str)>> = HashMap::new();
        // Local name → the file it names as a *module*, for the receiver of a
        // qualified call. Only populated by bindings that really are modules,
        // so a language with no module-valued names (the C family, whose
        // every callee is offered to every include) never pays for it.
        let mut modules: HashMap<&str, String> = HashMap::new();
        for import in &file.imports {
            // Where the specifier lands; resolved at most once per statement.
            let mut specifier_target = None;
            for local in &import.namespaces {
                let specifier = specifier_target
                    .get_or_insert_with(|| {
                        parser.resolve_import(&file.path, &import.specifier, &files, root)
                    })
                    .clone();
                if let Some(target) = specifier {
                    modules.insert(local.as_str(), target);
                }
            }
            for name in &import.names {
                let module = parser.resolve_name_as_module(
                    &file.path,
                    &import.specifier,
                    &name.imported,
                    &files,
                    root,
                );
                if let Some(module) = &module {
                    // `from pkg import util` binds `util` to the module, so
                    // `util.helper()` reaches into it.
                    modules.insert(name.local.as_str(), module.clone());
                }
                let specifier = specifier_target
                    .get_or_insert_with(|| {
                        parser.resolve_import(&file.path, &import.specifier, &files, root)
                    })
                    .clone();
                // Both candidates where the name is a module, because
                // `from pkg import shadow` can bind the module *and* a
                // symbol of that name in the package initialiser — and only
                // the symbol can be the target of a bare `shadow()`. The
                // specifier is pushed last so the last-first trial below
                // prefers it, which is the opposite of what the import edge
                // wants; an edge points at the module a reader would open,
                // a call points at the function that actually runs.
                for target in module.into_iter().chain(specifier) {
                    bindings
                        .entry(name.local.as_str())
                        .or_default()
                        .push((target, &name.imported));
                }
            }
        }
        // Receiver path → the file it names, memoized: one file often calls
        // several functions through the same module.
        let mut receivers: HashMap<&[String], Option<String>> = HashMap::new();
        for call in &file.calls {
            let target: Option<(String, String)> = if !call.receiver.is_empty() {
                // A qualified call reaches its callee *through* a module, so
                // it never falls back to the unqualified paths below: a
                // same-named local function is a different function, and
                // binding to it would be an invented edge.
                receivers
                    .entry(call.receiver.as_slice())
                    .or_insert_with(|| {
                        receiver_module(parser, &file.path, &call.receiver, &modules, &files, root)
                    })
                    .clone()
                    .and_then(|module| resolve_exported(&module, &call.callee))
            } else if file.functions.contains(&call.callee) {
                Some((file.path.clone(), call.callee.clone()))
            } else if let Some(sibling) = parser
                .directory_shares_scope()
                .then(|| {
                    // Go packages: sibling files in the directory share one
                    // namespace, so the callee may live next door with no
                    // import. Facts are path-sorted, so the match is
                    // deterministic.
                    facts.iter().find(|other| {
                        other.path != file.path
                            && directory_of(&other.path) == directory_of(&file.path)
                            && parser.extensions().contains(&extension_of(&other.path))
                            && other.functions.contains(&call.callee)
                    })
                })
                .flatten()
            {
                Some((sibling.path.clone(), call.callee.clone()))
            } else {
                bindings.get(call.callee.as_str()).and_then(|candidates| {
                    candidates
                        .iter()
                        .rev()
                        .find_map(|(target_file, imported)| resolve_exported(target_file, imported))
                })
            };
            let Some((target_file, target_fn)) = target else {
                continue;
            };
            // The caller must exist as a node too (its name survived dedup).
            if !file.functions.contains(&call.caller) {
                continue;
            }
            let edge = (
                NodeId::symbol(NodeKind::Function, &file.path, &call.caller),
                NodeId::symbol(NodeKind::Function, &target_file, &target_fn),
            );
            if seen.insert(edge.clone()) {
                edges.push(Edge::new(edge.0, edge.1, EdgeKind::Calls));
            }
        }
    }
}

/// The file a qualified call's receiver names as a module, if it names one.
///
/// First the receiver is written back out the way source writes a module
/// path — `["pkg","util"]` → `pkg.util`, `["crate","util"]` → `crate::util` —
/// and looked up among the modules this file's imports actually bound. That
/// is the whole answer in a dotted language, because a receiver nobody
/// imported is a *value*: `logger.info()` is a method call, and following it
/// into a `logger.py` sitting next door would fabricate an edge between two
/// files that have no relationship at all.
///
/// Only where the syntax rules values out — Rust's `::` — is an unbound
/// receiver resolved on sight, which is what lets `crate::util::helper()`
/// work with no `use` statement anywhere.
///
/// So this can never invent an edge: it answers only with a file the map
/// already contains, reached by a name the file itself introduced.
fn receiver_module(
    parser: &dyn parsers::Parser,
    importer: &str,
    receiver: &[String],
    modules: &HashMap<&str, String>,
    files: &HashSet<String>,
    root: &Path,
) -> Option<String> {
    let path = receiver.join(parser.module_path_separator());
    if let Some(target) = modules.get(path.as_str()) {
        return Some(target.clone());
    }
    parser
        .receiver_is_never_a_value()
        .then(|| parser.resolve_import(importer, &path, files, root))
        .flatten()
}

fn directory_of(path: &str) -> &str {
    path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("")
}

fn extension_of(path: &str) -> &str {
    Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default()
}

fn plural(word: &str, n: usize) -> String {
    if n == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}

/// Writes the map to `.codeatlas/knowledge-graph.json` under the scanned root.
pub fn save(root: &Path, graph: &KnowledgeGraph) -> Result<()> {
    let dir = root.join(OUTPUT_DIR);
    fs::create_dir_all(&dir)?;
    let mut json = serde_json::to_string_pretty(graph)?;
    json.push('\n');
    fs::write(dir.join("knowledge-graph.json"), json)?;
    Ok(())
}
