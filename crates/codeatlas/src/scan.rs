use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use ignore::WalkBuilder;

use crate::map::{
    Edge, EdgeKind, KnowledgeGraph, MAP_CONTRACT_VERSION, Node, NodeKind, Project, Provenance,
    Range,
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
    for path in paths {
        extract_file(&root, &path, &mut nodes, &mut edges);
    }

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

/// Emits the file node and, where a parser handles the language, its symbol
/// nodes and `contains` edges. Extraction failures degrade to a bare file
/// node; they never fail the scan.
fn extract_file(root: &Path, path: &str, nodes: &mut Vec<Node>, edges: &mut Vec<Edge>) {
    let source = fs::read(root.join(path))
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .unwrap_or_default();
    let line_count = source.lines().count();

    let extension = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    let parser = parsers::for_extension(extension);
    let mut symbols = parser.map(|p| p.parse(&source)).unwrap_or_default();

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

    let functions = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Function)
        .count();
    let classes = symbols
        .iter()
        .filter(|s| s.kind == SymbolKind::Class)
        .count();

    let file_id = format!("file:{path}");
    let file_label = parser.map_or("Plain", |p| p.language_name());
    let mut summary = format!("{file_label} file, {line_count} lines");
    if !symbols.is_empty() {
        summary.push_str(": ");
        let mut parts = Vec::new();
        if functions > 0 {
            parts.push(format!("{functions} {}", plural("function", functions)));
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
        let (kind, id_prefix, label) = match symbol.kind {
            SymbolKind::Function => (NodeKind::Function, "function", "Function"),
            SymbolKind::Class => (NodeKind::Class, "class", "Class"),
        };
        let id = format!("{id_prefix}:{path}:{}", symbol.name);
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
        edges.push(Edge {
            source: file_id.clone(),
            target: id,
            kind: EdgeKind::Contains,
        });
    }
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
