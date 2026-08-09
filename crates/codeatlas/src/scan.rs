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

    resolve_imports(&paths, &facts, &mut edges);
    resolve_calls(&paths, &facts, &mut edges);

    let project_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".to_string());

    Ok(KnowledgeGraph {
        version: MAP_CONTRACT_VERSION.to_string(),
        project: Project { name: project_name },
        nodes,
        edges,
    })
}

/// What one file contributes to the cross-file resolution phase.
struct FileFacts {
    path: String,
    imports: Vec<parsers::Import>,
    calls: Vec<parsers::Call>,
    /// Final (post-dedup) names of this file's function nodes.
    functions: HashSet<String>,
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
    }
}

/// Resolves each file's import specifiers against the scanned file set and
/// emits `imports` edges between file nodes. Unresolvable imports (bare
/// packages, files outside the map) are dropped, never emitted dangling.
fn resolve_imports(paths: &[String], facts: &[FileFacts], edges: &mut Vec<Edge>) {
    let files: HashSet<String> = paths.iter().cloned().collect();
    let mut seen: HashSet<(NodeId, NodeId)> = HashSet::new();
    for file in facts {
        let Some(parser) = parsers::for_extension(extension_of(&file.path)) else {
            continue;
        };
        for import in &file.imports {
            let Some(target) = parser.resolve_import(&file.path, &import.specifier, &files) else {
                continue;
            };
            let edge = (NodeId::file(&file.path), NodeId::file(&target));
            if seen.insert(edge.clone()) {
                edges.push(Edge::new(edge.0, edge.1, EdgeKind::Imports));
            }
        }
    }
}

/// Connects invocations to function nodes and emits `calls` edges. A callee
/// resolves within its own file first, then through a named import whose
/// module resolved into the map. Anything else — member calls, packages,
/// files outside the map — is dropped, never emitted dangling.
fn resolve_calls(paths: &[String], facts: &[FileFacts], edges: &mut Vec<Edge>) {
    let files: HashSet<String> = paths.iter().cloned().collect();
    let fn_index: HashMap<&str, &HashSet<String>> = facts
        .iter()
        .map(|f| (f.path.as_str(), &f.functions))
        .collect();
    let mut seen: HashSet<(NodeId, NodeId)> = HashSet::new();
    for file in facts {
        let Some(parser) = parsers::for_extension(extension_of(&file.path)) else {
            continue;
        };
        // Local binding name → (defining file, exported name).
        let mut bindings: HashMap<&str, (String, &str)> = HashMap::new();
        for import in &file.imports {
            if let Some(target) = parser.resolve_import(&file.path, &import.specifier, &files) {
                for name in &import.names {
                    bindings.insert(name.local.as_str(), (target.clone(), &name.imported));
                }
            }
        }
        for call in &file.calls {
            let target = if file.functions.contains(&call.callee) {
                Some((file.path.as_str(), call.callee.as_str()))
            } else {
                bindings
                    .get(call.callee.as_str())
                    .filter(|(target_file, imported)| {
                        fn_index
                            .get(target_file.as_str())
                            .is_some_and(|fns| fns.contains(*imported))
                    })
                    .map(|(target_file, imported)| (target_file.as_str(), *imported))
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
                NodeId::symbol(NodeKind::Function, target_file, target_fn),
            );
            if seen.insert(edge.clone()) {
                edges.push(Edge::new(edge.0, edge.1, EdgeKind::Calls));
            }
        }
    }
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
