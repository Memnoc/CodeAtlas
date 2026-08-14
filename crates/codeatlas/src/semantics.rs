//! Mechanical semantics: the zero-LLM projections that make the map complete.
//! Layers, domain flows, and the tour are computed here deterministically
//! from the structural graph alone — enrichment (ADR-0004) may later relabel
//! what this module creates, never create anything itself.

use std::collections::{BTreeMap, BTreeSet};

use crate::map::{
    DomainFlow, EdgeKind, KnowledgeGraph, Layer, LayerDescription, NodeId, NodeKind, Provenance,
    TourStep,
};

/// Runs every mechanical projection over a freshly built structural graph.
pub fn apply(graph: &mut KnowledgeGraph) {
    assign_layers(graph);
    publish_significance(graph);
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

/// The mechanical sentence describing a layer — the exact text the dashboard
/// synthesised for a region card before the contract carried a description,
/// so a map with this description renders as one without it did. Published
/// by the scan under `structural` provenance; enrichment may replace it
/// (ticket 07), never this function.
fn describe_layer(id: &str) -> String {
    if id == ROOT_LAYER {
        "Files at the repository root".to_string()
    } else {
        format!("Files under {id}/")
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
            description: Some(LayerDescription {
                text: describe_layer(&id),
                provenance: Provenance::Structural,
            }),
            id,
            provenance: Provenance::Structural,
        })
        .collect();
}

/// Import degree, in one walk of the edges: how many `imports` edges arrive
/// at each file node (fan-in) and how many leave it (fan-out). Significance
/// and the tour's reading order are both built from these two counts.
fn import_degree(graph: &KnowledgeGraph) -> (BTreeMap<&NodeId, u32>, BTreeMap<&NodeId, u32>) {
    let mut fan_in: BTreeMap<&NodeId, u32> = BTreeMap::new();
    let mut fan_out: BTreeMap<&NodeId, u32> = BTreeMap::new();
    for edge in graph.edges.iter().filter(|e| e.kind == EdgeKind::Imports) {
        *fan_out.entry(&edge.source).or_default() += 1;
        *fan_in.entry(&edge.target).or_default() += 1;
    }
    (fan_in, fan_out)
}

/// The paths of files hosting an entry point: a function nothing calls that
/// starts a call chain — the same roots the domain flows grow from. How many
/// a file holds never matters, only whether it holds one.
fn entry_point_files(graph: &KnowledgeGraph) -> BTreeSet<&str> {
    let mut calls_out: BTreeSet<&NodeId> = BTreeSet::new();
    let mut called: BTreeSet<&NodeId> = BTreeSet::new();
    for edge in graph.edges.iter().filter(|e| e.kind == EdgeKind::Calls) {
        calls_out.insert(&edge.source);
        called.insert(&edge.target);
    }
    graph
        .nodes
        .iter()
        .filter(|n| {
            n.kind == NodeKind::Function && calls_out.contains(&n.id) && !called.contains(&n.id)
        })
        .map(|n| n.path.as_str())
        .collect()
}

/// Publishes each file node's significance — and holds the only copy of the
/// formula (ADR-0010): import fan-in + import fan-out + 1 if the file hosts
/// an entry point. The entry-point term is one point for the file, not one
/// per function, so a test module full of independent test functions cannot
/// out-rank a module the codebase depends on.
///
/// Every file gets a number, zeros included: a zero says nothing imports the
/// file, it imports nothing, and no call chain starts in it — a fact, not an
/// absence. Symbol nodes get none; significance is a file-level number they
/// would only blur.
///
/// This is what "which files matter" means, computed once so that no
/// consumer has to answer it again: [`build_tour`] selects on it here, and
/// publishing it in the map is what lets the dashboard rank the same way
/// instead of deriving a second answer in a second language.
fn publish_significance(graph: &mut KnowledgeGraph) {
    // Computed against the whole graph first, because writing the numbers
    // borrows the nodes mutably.
    let published: Vec<Option<u32>> = {
        let (fan_in, fan_out) = import_degree(graph);
        let entry_points = entry_point_files(graph);
        graph
            .nodes
            .iter()
            .map(|node| {
                (node.kind == NodeKind::File).then(|| {
                    fan_in.get(&node.id).copied().unwrap_or(0)
                        + fan_out.get(&node.id).copied().unwrap_or(0)
                        + u32::from(entry_points.contains(node.path.as_str()))
                })
            })
            .collect()
    };
    for (node, significance) in graph.nodes.iter_mut().zip(published) {
        node.significance = significance;
    }
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

/// The most stops a guided tour may have. A tour is a newcomer's first
/// sitting, not an inventory: the walk stays this long whether the
/// repository holds fifty files or five thousand.
pub const TOUR_MAX_STEPS: usize = 12;

/// A file the tour might stop at, carrying both scores [`build_tour`] ranks
/// it by: `significance` — the published number, not one of the tour's own —
/// decides whether it makes the walk at all, and `reading_order` decides
/// where in the walk it lands.
struct Candidate<'g> {
    significance: u32,
    reading_order: i64,
    path: &'g str,
    id: &'g NodeId,
}

/// Builds the guided tour: a bounded, curated walk over the files that
/// carry the architecture. Two deterministic rules, applied in order.
///
/// **Selection — which files are worth a newcomer's time.** Each file is
/// ranked by the significance the map publishes for it
/// ([`publish_significance`], ADR-0010) — the tour reads that number rather
/// than deriving one of its own, so the walk cannot disagree with the
/// dashboard about which files matter. A file scoring zero — nothing imports
/// it, it imports nothing, no call chain starts in it — is off the tour: it
/// teaches nothing about how the pieces connect. The highest-scoring
/// [`TOUR_MAX_STEPS`] survive, ties on path.
///
/// **Order — the sequence the survivors are walked in.** Reading order is
/// `fan-out − fan-in + the same entry-point bonus`, descending, ties on
/// path: composition roots and entry points open the walk, and the
/// most-imported foundation modules close it. This is the ranking that
/// used to select the tour as well, which is why isolated files (scoring
/// zero rather than negative) led it — selection now settles that.
///
/// `pub(crate)` for enrichment carry-over (ADR-0005): the mechanical label
/// is the derivation a stored tour annotation is keyed against, and only
/// this function can recompute it once an enriched label occupies the slot.
/// It reads only structural facts (paths, kinds, edges, published
/// significance), which enrichment never edits, so the recomputation is
/// valid on an enriched graph.
pub(crate) fn build_tour(graph: &KnowledgeGraph) -> Vec<TourStep> {
    let (fan_in, fan_out) = import_degree(graph);
    let entry_point_files = entry_point_files(graph);

    let mut candidates: Vec<Candidate<'_>> = graph
        .nodes
        .iter()
        .filter(|n| n.kind == NodeKind::File)
        .filter_map(|node| {
            let fan_out = i64::from(fan_out.get(&node.id).copied().unwrap_or(0));
            let fan_in = i64::from(fan_in.get(&node.id).copied().unwrap_or(0));
            let entry_bonus = i64::from(entry_point_files.contains(node.path.as_str()));
            // Selection reads the published number; a file that carries none
            // came from a producer that published no significance at all, and
            // is off the walk rather than silently rescored here.
            let significance = node.significance.unwrap_or(0);
            (significance > 0).then_some(Candidate {
                significance,
                reading_order: fan_out - fan_in + entry_bonus,
                path: node.path.as_str(),
                id: &node.id,
            })
        })
        .collect();
    candidates.sort_by(|a, b| b.significance.cmp(&a.significance).then(a.path.cmp(b.path)));
    candidates.truncate(TOUR_MAX_STEPS);
    candidates.sort_by(|a, b| {
        b.reading_order
            .cmp(&a.reading_order)
            .then(a.path.cmp(b.path))
    });

    candidates
        .into_iter()
        .map(|Candidate { path, id, .. }| {
            let prefix = if entry_point_files.contains(path) {
                "Entry point: "
            } else {
                ""
            };
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
