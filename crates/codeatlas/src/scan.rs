use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use ignore::WalkBuilder;

use crate::map::{KnowledgeGraph, MAP_CONTRACT_VERSION, Node, NodeKind, Project, Provenance};

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

    let nodes = paths
        .into_iter()
        .map(|path| {
            let name = path.rsplit('/').next().unwrap_or(&path).to_string();
            Node {
                id: format!("file:{path}"),
                kind: NodeKind::File,
                name,
                path,
                provenance: Provenance::Structural,
            }
        })
        .collect();

    let project_name = root
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "project".to_string());

    Ok(KnowledgeGraph {
        version: MAP_CONTRACT_VERSION.to_string(),
        project: Project { name: project_name },
        nodes,
        edges: Vec::new(),
    })
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
