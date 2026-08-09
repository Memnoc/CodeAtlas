//! Mechanical semantics: the zero-LLM projections that make the map complete.
//! Layers, domain flows, and the tour are computed here deterministically
//! from the structural graph alone — enrichment (ADR-0004) may later relabel
//! what this module creates, never create anything itself.

use std::collections::{BTreeMap, BTreeSet};

use crate::map::{
    DomainFlow, EdgeKind, KnowledgeGraph, Layer, NodeId, NodeKind, Provenance, TourStep,
};

/// Runs every mechanical projection over a freshly built structural graph.
pub fn apply(graph: &mut KnowledgeGraph) {
    assign_layers(graph);
    graph.domain_flows = project_flows(graph);
    graph.tour = build_tour(graph);
}

/// The layer (and domain) files at the repository root fall into.
const ROOT_LAYER: &str = "root";

/// The top-level directory of a repo-relative path, or [`ROOT_LAYER`] for
/// paths at the repository root — the single rule layers and domains share.
fn top_directory(path: &str) -> &str {
    match path.split_once('/') {
        Some((top, _)) => top,
        None => ROOT_LAYER,
    }
}

/// Assigns every file node to exactly one layer derived from its top-level
/// directory (root-level files share the `root` layer) and records the layer
/// list, sorted by ID.
fn assign_layers(graph: &mut KnowledgeGraph) {
    let mut ids: Vec<String> = Vec::new();
    for node in &mut graph.nodes {
        if node.kind != NodeKind::File {
            continue;
        }
        let layer = top_directory(&node.path);
        if !ids.iter().any(|id| id == layer) {
            ids.push(layer.to_string());
        }
        node.layer = Some(layer.to_string());
    }
    ids.sort_unstable();
    graph.layers = ids
        .into_iter()
        .map(|id| Layer {
            name: id.clone(),
            id,
            provenance: Provenance::Structural,
        })
        .collect();
}

/// Projects domain flows: one flow per entry point — a function no other
/// function calls that itself calls at least one function — walking the
/// `calls` edges depth-first with sorted neighbors, so the chain is
/// deterministic. The flow's domain is the top-level directory of the root
/// function's file.
fn project_flows(graph: &KnowledgeGraph) -> Vec<DomainFlow> {
    let mut callees: BTreeMap<&NodeId, BTreeSet<&NodeId>> = BTreeMap::new();
    let mut called: BTreeSet<&NodeId> = BTreeSet::new();
    for edge in graph.edges.iter().filter(|e| e.kind == EdgeKind::Calls) {
        callees
            .entry(&edge.source)
            .or_default()
            .insert(&edge.target);
        called.insert(&edge.target);
    }
    let name_of: BTreeMap<&NodeId, &str> = graph
        .nodes
        .iter()
        .map(|n| (&n.id, n.name.as_str()))
        .collect();

    let mut flows = Vec::new();
    // Nodes are path-sorted, so roots — and therefore flows — come out in a
    // deterministic order without further sorting.
    for root in &graph.nodes {
        if root.kind != NodeKind::Function
            || called.contains(&root.id)
            || !callees.contains_key(&root.id)
        {
            continue;
        }

        // Depth-first pre-order over the call chain, cycle-safe.
        let mut steps: Vec<&NodeId> = Vec::new();
        let mut visited: BTreeSet<&NodeId> = BTreeSet::new();
        let mut stack: Vec<&NodeId> = vec![&root.id];
        while let Some(id) = stack.pop() {
            if !visited.insert(id) {
                continue;
            }
            steps.push(id);
            if let Some(next) = callees.get(id) {
                // Reversed so the smallest ID is walked first.
                stack.extend(next.iter().rev());
            }
        }

        let name = steps
            .iter()
            .map(|id| name_of.get(*id).copied().unwrap_or(id.as_str()))
            .collect::<Vec<_>>()
            .join(" → ");
        flows.push(DomainFlow {
            id: format!("flow:{}", root.id.as_str()),
            name,
            domain: top_directory(&root.path).to_string(),
            steps: steps.into_iter().cloned().collect(),
            provenance: Provenance::Structural,
        });
    }
    flows
}

/// Builds the tour: every file node, ordered by a topology score so entry
/// points open the walk and the most-imported foundation modules close it.
/// Score = import fan-out − import fan-in + number of entry-point functions
/// the file contains; ties break on path, so the order is deterministic.
fn build_tour(graph: &KnowledgeGraph) -> Vec<TourStep> {
    let mut fan_in: BTreeMap<&NodeId, i64> = BTreeMap::new();
    let mut fan_out: BTreeMap<&NodeId, i64> = BTreeMap::new();
    for edge in graph.edges.iter().filter(|e| e.kind == EdgeKind::Imports) {
        *fan_out.entry(&edge.source).or_default() += 1;
        *fan_in.entry(&edge.target).or_default() += 1;
    }

    // Entry-point functions per file: functions nothing calls that start a
    // call chain — the same roots the domain flows grow from.
    let mut calls_out: BTreeSet<&NodeId> = BTreeSet::new();
    let mut called: BTreeSet<&NodeId> = BTreeSet::new();
    for edge in graph.edges.iter().filter(|e| e.kind == EdgeKind::Calls) {
        calls_out.insert(&edge.source);
        called.insert(&edge.target);
    }
    let mut entry_fns: BTreeMap<&str, i64> = BTreeMap::new();
    for node in &graph.nodes {
        if node.kind == NodeKind::Function
            && calls_out.contains(&node.id)
            && !called.contains(&node.id)
        {
            *entry_fns.entry(node.path.as_str()).or_default() += 1;
        }
    }

    let mut files: Vec<(i64, &str, &NodeId)> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .map(|node| {
            let fan_out = fan_out.get(&node.id).copied().unwrap_or(0);
            let fan_in = fan_in.get(&node.id).copied().unwrap_or(0);
            let entries = entry_fns.get(node.path.as_str()).copied().unwrap_or(0);
            (fan_out - fan_in + entries, node.path.as_str(), &node.id)
        })
        .collect();
    files.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(b.1)));

    files
        .into_iter()
        .map(|(_, path, id)| {
            let entries = entry_fns.get(path).copied().unwrap_or(0);
            let prefix = if entries > 0 { "Entry point: " } else { "" };
            TourStep {
                node: id.clone(),
                label: format!(
                    "{prefix}{path} — fan-in {}, fan-out {}",
                    fan_in.get(id).copied().unwrap_or(0),
                    fan_out.get(id).copied().unwrap_or(0),
                ),
                provenance: Provenance::Structural,
            }
        })
        .collect()
}
