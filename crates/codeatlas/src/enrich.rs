//! Enrichment core (ADR-0004, ADR-0005): the provider trait that abstracts
//! the LLM, the carry-over annotation store, and the `--enrich` pipeline.
//!
//! Enrichment relabels reality; it never creates it. The structural scan
//! computes a mechanical summary for every node; enrichment may replace that
//! prose through a typed request/response exchange with a provider, flipping
//! the node's provenance to `llm`. A node the provider does not answer for —
//! or any provider failure — leaves the mechanical summary in place: the map
//! is always complete and schema-valid, enriched or not.
//!
//! # Provider selection
//!
//! The binary resolves its provider from `--provider` if given, else from
//! the `CODEATLAS_ENRICH_PROVIDER` env var, else from the build's default.
//! [`recognised_specs`] is the single source for what a build accepts —
//! every message that names the alternatives renders from it, so none can
//! offer a spec the binary cannot select. Recognised specs and per-build
//! defaults:
//!
//! - `claude` — the real Claude API provider ([`claude`], `network`
//!   builds only). This is also the **default** in a shipped `network`
//!   build when the env var is unset.
//! - `fake:<path>` / `fail` — offline test backends (below).
//! - Unset in a **test build** (`test-provider` feature, enabled by the
//!   self dev-dependency in `Cargo.toml`) — an error: tests must pick a
//!   backend explicitly, so none can fall through to a provider that
//!   opens sockets (the no-network-in-tests rule).
//! - Unset in a build with no backend compiled in (`--no-default-features`,
//!   ADR-0006's sealed build) — a clear "enrichment is not available in this
//!   build" error; there is nothing to select.
//!
//! The test backends, compiled in only for test builds:
//!
//! - `fake:<path>` — canned typed responses from a JSON file mapping
//!   slot key → text (see [`EnrichmentSlot::key`]), e.g.
//!   `summary:<node-id>`, `layer-name:<layer-id>`, `flow-name:<flow-id>`,
//!   `tour-label:<node-id>`
//! - `fail` — a provider that errors on every call (failure injection,
//!   spec story 14)
//!
//! Neither backend can open a socket; no test performs network I/O.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::map::{EdgeKind, KnowledgeGraph, NodeId, NodeKind, Provenance};

#[cfg(feature = "network")]
pub mod claude;

/// The env var the CLI resolves its enrichment provider from.
pub const PROVIDER_ENV: &str = "CODEATLAS_ENRICH_PROVIDER";

/// The most slots a single provider request may carry (spec: bounded
/// prompts — the model never sees the whole serialized graph, and a
/// request's size cannot grow with the repository). 25 slots keep the
/// prompt at a few KB and the structured response comfortably inside one
/// completion; larger repos simply make more requests.
pub const BATCH_SIZE: usize = 25;

/// The most step function names a flow slot carries into a prompt. A flow's
/// chain walks everything reachable from its entry point, so its length
/// grows with the repository; the slot stays bounded by naming only the
/// opening of the chain (plus the total count).
pub const FLOW_SLOT_STEP_NAMES: usize = 10;

/// One node whose summary slot the provider is asked to fill.
#[derive(Debug, Clone)]
pub struct SummarySlot {
    pub node: NodeId,
    pub kind: NodeKind,
    pub name: String,
    /// Repo-relative path of the node's file.
    pub path: String,
    /// The mechanical summary the slot currently holds — the fallback the
    /// provider's prose would replace.
    pub mechanical_summary: String,
}

/// One layer whose display-name slot the provider is asked to fill. The
/// slot carries mechanically summarized topology only: the deriving
/// directory (the layer ID) and how many files the layer holds — never the
/// member list, never edges.
#[derive(Debug, Clone)]
pub struct LayerSlot {
    /// The layer ID: the top-level directory that derived it (`root` for
    /// repository-root files) — also its mechanical name.
    pub id: String,
    /// How many file nodes the layer contains.
    pub member_files: usize,
}

/// One domain flow whose name slot the provider is asked to fill: the
/// domain, the entry point, and the opening step function names — at most
/// [`FLOW_SLOT_STEP_NAMES`] of them, so the slot cannot grow with the
/// call graph.
#[derive(Debug, Clone)]
pub struct FlowSlot {
    /// The flow ID, e.g. `flow:function:src/main.ts:main`.
    pub id: String,
    /// Top-level directory the flow's entry point lives in.
    pub domain: String,
    /// Name of the entry-point function the flow grows from.
    pub entry: String,
    /// Function names along the chain, entry first — truncated to
    /// [`FLOW_SLOT_STEP_NAMES`].
    pub step_names: Vec<String>,
    /// Total steps in the chain (may exceed `step_names.len()`).
    pub step_count: usize,
}

/// One tour stop whose label slot the provider is asked to fill: the
/// node's path, its import fan-in/out — the same numbers the mechanical
/// label cites — and that mechanical label as the fallback being replaced.
#[derive(Debug, Clone)]
pub struct TourSlot {
    /// The file node this step visits.
    pub node: NodeId,
    /// Repo-relative path of that file.
    pub path: String,
    pub fan_in: usize,
    pub fan_out: usize,
    /// The mechanical narration the provider's prose would replace.
    pub mechanical_label: String,
}

/// Every kind of slot enrichment can fill — a typed request never carries
/// anything but these. Each variant addresses its answer through a
/// distinct [`key`](Self::key) prefix, so an answer can only ever land in
/// the slot kind it was written for.
#[derive(Debug, Clone)]
pub enum EnrichmentSlot {
    NodeSummary(SummarySlot),
    LayerName(LayerSlot),
    FlowName(FlowSlot),
    TourLabel(TourSlot),
}

impl EnrichmentSlot {
    /// The address a provider's answer for this slot must carry. Prefixing
    /// by slot kind makes the namespaces collision-proof: node IDs, layer
    /// IDs (bare directory names), flow IDs, and tour node IDs can never
    /// claim one another's answers — `summary:file:src/a.ts` and
    /// `tour-label:file:src/a.ts` are distinct keys.
    pub fn key(&self) -> String {
        match self {
            Self::NodeSummary(s) => summary_key(s.node.as_str()),
            Self::LayerName(s) => layer_key(&s.id),
            Self::FlowName(s) => flow_key(&s.id),
            Self::TourLabel(s) => tour_key(s.node.as_str()),
        }
    }
}

fn summary_key(node_id: &str) -> String {
    format!("summary:{node_id}")
}

fn layer_key(layer_id: &str) -> String {
    format!("layer-name:{layer_id}")
}

fn flow_key(flow_id: &str) -> String {
    format!("flow-name:{flow_id}")
}

fn tour_key(node_id: &str) -> String {
    format!("tour-label:{node_id}")
}

/// A typed enrichment request: the slots to fill, nothing else. Never the
/// whole serialized graph (spec: bounded prompts).
#[derive(Debug)]
pub struct EnrichmentRequest {
    pub project: String,
    pub slots: Vec<EnrichmentSlot>,
}

/// A typed enrichment response: prose per slot key (see
/// [`EnrichmentSlot::key`]). Slots absent from the map keep their
/// mechanical text.
#[derive(Debug, Default)]
pub struct EnrichmentResponse {
    pub answers: BTreeMap<String, String>,
}

/// The second test seam (with the map contract): everything that can fill
/// enrichment slots — the Claude API, a local model, a canned fake — sits
/// behind this trait (ADR-0004). Typed request in, typed response out; any
/// error degrades the run, never the map.
pub trait EnrichmentProvider {
    fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse>;
}

/// The annotation store's file name under [`crate::scan::OUTPUT_DIR`]. The
/// store is internal — NOT part of the map contract — but deterministic
/// (sorted keys) and versioned so its format can evolve.
pub const ANNOTATIONS_FILE: &str = "annotations.json";

/// Bumped whenever the store format (including the hash definitions)
/// changes; a store with another version is ignored, which merely costs a
/// re-enrichment. 2: added the semantic sections (`layers`, `flows`,
/// `tour`) keyed by derivation-input hashes.
const STORE_VERSION: u32 = 2;

/// The carry-over store (ADR-0005): enrichment prose keyed by identity
/// plus a hash of what derived it. Node annotations key on the node ID
/// (which embeds the repo-relative path) plus the node's file content
/// hash. Semantic annotations — layers, flows, tour steps — are not
/// file-backed, so they key on their semantic identity (layer ID, flow ID,
/// tour node ID) plus a hash of the mechanical inputs that derived them:
/// the layer's sorted member set, the flow's step ID chain, the tour
/// step's mechanical label (path + import fan-in/out + entry-point
/// status). Annotations re-attach for free while their derivation is
/// unchanged and expire the moment it changes — stale prose never
/// describes new code or a new shape.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AnnotationStore {
    version: u32,
    annotations: BTreeMap<String, Annotation>,
    #[serde(default)]
    layers: BTreeMap<String, SemanticAnnotation>,
    #[serde(default)]
    flows: BTreeMap<String, SemanticAnnotation>,
    #[serde(default)]
    tour: BTreeMap<String, SemanticAnnotation>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Annotation {
    content_hash: String,
    summary: String,
}

/// An enriched semantic label: the text plus the hash of the mechanical
/// inputs it was purchased for (see [`semantic_hashes`]).
#[derive(Debug, Serialize, Deserialize)]
struct SemanticAnnotation {
    inputs_hash: String,
    text: String,
}

/// The derivation-input hashes of every semantic element in the graph,
/// computed from structural facts only (membership, step chains, edges) —
/// never from the enrichable name/label slots — so they are identical
/// before and after enrichment.
struct SemanticHashes {
    /// Layer ID → hash of the sorted member file paths.
    layers: BTreeMap<String, String>,
    /// Flow ID → hash of the step node-ID chain, in order.
    flows: BTreeMap<String, String>,
    /// Tour node ID → hash of the recomputed mechanical label (path +
    /// import fan-in/out + entry-point status).
    tour: BTreeMap<String, String>,
}

fn semantic_hashes(graph: &KnowledgeGraph) -> SemanticHashes {
    let mut members: BTreeMap<&str, Vec<&str>> = graph
        .layers
        .iter()
        .map(|l| (l.id.as_str(), Vec::new()))
        .collect();
    for node in graph.nodes.iter().filter(|n| n.kind == NodeKind::File) {
        if let Some(layer) = &node.layer
            && let Some(paths) = members.get_mut(layer.as_str())
        {
            paths.push(node.path.as_str());
        }
    }
    let layers = members
        .into_iter()
        .map(|(id, mut paths)| {
            paths.sort_unstable();
            (id.to_string(), content_hash(paths.join("\n").as_bytes()))
        })
        .collect();

    let flows = graph
        .domain_flows
        .iter()
        .map(|flow| {
            let steps: Vec<&str> = flow.steps.iter().map(NodeId::as_str).collect();
            (flow.id.clone(), content_hash(steps.join("\n").as_bytes()))
        })
        .collect();

    let tour = crate::semantics::build_tour(graph)
        .into_iter()
        .map(|step| {
            (
                step.node.as_str().to_string(),
                content_hash(step.label.as_bytes()),
            )
        })
        .collect();

    SemanticHashes {
        layers,
        flows,
        tour,
    }
}

impl AnnotationStore {
    /// Loads the store from `.codeatlas/`. A missing, unreadable, corrupt,
    /// or other-version store is an empty one: carry-over degrades, the
    /// scan never breaks.
    pub fn load(root: &Path) -> Self {
        let path = root.join(crate::scan::OUTPUT_DIR).join(ANNOTATIONS_FILE);
        fs::read_to_string(path)
            .ok()
            .and_then(|raw| serde_json::from_str::<Self>(&raw).ok())
            .filter(|store| store.version == STORE_VERSION)
            .unwrap_or_default()
    }

    /// Re-attaches stored annotations to the freshly rebuilt structural
    /// graph: a node whose ID is in the store and whose file content still
    /// matches the recorded hash gets its enriched summary back (and `llm`
    /// provenance) without any provider call; a layer, flow, or tour step
    /// whose derivation-input hash still matches gets its enriched name or
    /// label back the same way. Everything else stays structural — an
    /// edited file's nodes and a changed derivation's labels revert and
    /// will be re-selected by the next `--enrich`.
    pub fn reattach(&self, root: &Path, graph: &mut KnowledgeGraph) {
        if !self.annotations.is_empty() {
            let mut hashes = HashCache::new(root);
            for node in &mut graph.nodes {
                let Some(annotation) = self.annotations.get(node.id.as_str()) else {
                    continue;
                };
                if hashes
                    .of(&node.path)
                    .is_some_and(|h| h == annotation.content_hash)
                {
                    node.summary = annotation.summary.clone();
                    node.provenance = Provenance::Llm;
                }
            }
        }

        if self.layers.is_empty() && self.flows.is_empty() && self.tour.is_empty() {
            return;
        }
        let hashes = semantic_hashes(graph);
        for layer in &mut graph.layers {
            if let Some(a) = self.layers.get(&layer.id)
                && hashes.layers.get(&layer.id) == Some(&a.inputs_hash)
            {
                layer.name = a.text.clone();
                layer.provenance = Provenance::Llm;
            }
        }
        for flow in &mut graph.domain_flows {
            if let Some(a) = self.flows.get(&flow.id)
                && hashes.flows.get(&flow.id) == Some(&a.inputs_hash)
            {
                flow.name = a.text.clone();
                flow.provenance = Provenance::Llm;
            }
        }
        for step in &mut graph.tour {
            if let Some(a) = self.tour.get(step.node.as_str())
                && hashes.tour.get(step.node.as_str()) == Some(&a.inputs_hash)
            {
                step.label = a.text.clone();
                step.provenance = Provenance::Llm;
            }
        }
    }
}

/// Rebuilds the store from the enriched graph — every `llm`-provenance
/// node becomes one annotation keyed by its current file hash, and every
/// `llm`-provenance layer, flow, and tour step one semantic annotation
/// keyed by its current derivation-input hash — and writes it
/// deterministically (sorted keys, pretty, trailing newline). Rebuilding
/// from the graph is self-pruning: annotations for deleted elements or
/// no-longer-matching derivations simply cease to exist.
fn save_store(root: &Path, graph: &KnowledgeGraph) -> Result<()> {
    let mut hashes = HashCache::new(root);
    let annotations: BTreeMap<String, Annotation> = graph
        .nodes
        .iter()
        .filter(|n| n.provenance == Provenance::Llm)
        .filter_map(|n| {
            let content_hash = hashes.of(&n.path)?;
            Some((
                n.id.as_str().to_string(),
                Annotation {
                    content_hash,
                    summary: n.summary.clone(),
                },
            ))
        })
        .collect();
    let semantic = semantic_hashes(graph);
    let layers: BTreeMap<String, SemanticAnnotation> = graph
        .layers
        .iter()
        .filter(|l| l.provenance == Provenance::Llm)
        .filter_map(|l| {
            Some((
                l.id.clone(),
                SemanticAnnotation {
                    inputs_hash: semantic.layers.get(&l.id)?.clone(),
                    text: l.name.clone(),
                },
            ))
        })
        .collect();
    let flows: BTreeMap<String, SemanticAnnotation> = graph
        .domain_flows
        .iter()
        .filter(|f| f.provenance == Provenance::Llm)
        .filter_map(|f| {
            Some((
                f.id.clone(),
                SemanticAnnotation {
                    inputs_hash: semantic.flows.get(&f.id)?.clone(),
                    text: f.name.clone(),
                },
            ))
        })
        .collect();
    let tour: BTreeMap<String, SemanticAnnotation> = graph
        .tour
        .iter()
        .filter(|s| s.provenance == Provenance::Llm)
        .filter_map(|s| {
            Some((
                s.node.as_str().to_string(),
                SemanticAnnotation {
                    inputs_hash: semantic.tour.get(s.node.as_str())?.clone(),
                    text: s.label.clone(),
                },
            ))
        })
        .collect();
    let store = AnnotationStore {
        version: STORE_VERSION,
        annotations,
        layers,
        flows,
        tour,
    };
    let dir = root.join(crate::scan::OUTPUT_DIR);
    fs::create_dir_all(&dir)?;
    let mut json = serde_json::to_string_pretty(&store)?;
    json.push('\n');
    fs::write(dir.join(ANNOTATIONS_FILE), json)?;
    Ok(())
}

/// Per-run cache of file content hashes, read repo-relative so the hash is
/// deterministic for identical content wherever the repo lives. Unreadable
/// files hash to `None`, which never matches: their annotations expire.
struct HashCache<'a> {
    root: &'a Path,
    by_path: BTreeMap<String, Option<String>>,
}

impl<'a> HashCache<'a> {
    fn new(root: &'a Path) -> Self {
        Self {
            root,
            by_path: BTreeMap::new(),
        }
    }

    fn of(&mut self, path: &str) -> Option<String> {
        self.by_path
            .entry(path.to_string())
            .or_insert_with(|| {
                fs::read(self.root.join(path))
                    .ok()
                    .map(|b| content_hash(&b))
            })
            .clone()
    }
}

/// FNV-1a 64-bit over the file's bytes. Not cryptographic — this is cache
/// invalidation, not security — but deterministic across platforms and
/// dependency-free. The `fnv1a64:` prefix names the algorithm so a future
/// change is a visible format change (see [`STORE_VERSION`]).
fn content_hash(bytes: &[u8]) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("fnv1a64:{hash:016x}")
}

/// What the `--enrich` step did — the CLI words its success message from
/// this, so "no provider was needed" and "the provider answered" stay
/// distinguishable.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// No structural-provenance slot needed filling (empty map, or every
    /// summary, layer name, flow name, and tour label already enriched or
    /// carried over): no provider was resolved and no request was made.
    NothingToEnrich,
    /// The provider ran; this many slots were enriched.
    Enriched(usize),
}

/// The `--enrich` step, run after the structural map is already saved:
/// resolve a provider, fill the summary slots of structural-provenance
/// nodes, and re-save the map. When nothing needs enrichment this succeeds
/// without even resolving a provider (a repo with everything carried over
/// must not demand credentials). Any error leaves the saved structural map
/// untouched.
pub fn run(root: &Path, graph: &mut KnowledgeGraph, choice: ProviderChoice<'_>) -> Result<Outcome> {
    if collect_slots(graph).is_empty() {
        return Ok(Outcome::NothingToEnrich);
    }
    let provider = resolve_provider(choice)?;
    let count = fill_slots(graph, provider.as_ref())?;
    save_store(root, graph)?;
    crate::scan::save(root, graph)?;
    Ok(Outcome::Enriched(count))
}

/// The slots the provider would be asked to fill: only
/// `structural`-provenance nodes, layers, flows, and tour steps are
/// selected (ADR-0005 — enriched slots are never re-purchased). Every slot
/// carries mechanically summarized topology only — member counts, step
/// names, fan-in/out — never the serialized graph.
fn collect_slots(graph: &KnowledgeGraph) -> Vec<EnrichmentSlot> {
    let mut slots: Vec<EnrichmentSlot> = graph
        .nodes
        .iter()
        .filter(|n| n.provenance == Provenance::Structural)
        .map(|n| {
            EnrichmentSlot::NodeSummary(SummarySlot {
                node: n.id.clone(),
                kind: n.kind,
                name: n.name.clone(),
                path: n.path.clone(),
                mechanical_summary: n.summary.clone(),
            })
        })
        .collect();

    let mut member_files: BTreeMap<&str, usize> = BTreeMap::new();
    for node in graph.nodes.iter().filter(|n| n.kind == NodeKind::File) {
        if let Some(layer) = &node.layer {
            *member_files.entry(layer.as_str()).or_default() += 1;
        }
    }
    slots.extend(
        graph
            .layers
            .iter()
            .filter(|l| l.provenance == Provenance::Structural)
            .map(|l| {
                EnrichmentSlot::LayerName(LayerSlot {
                    id: l.id.clone(),
                    member_files: member_files.get(l.id.as_str()).copied().unwrap_or(0),
                })
            }),
    );

    let name_of: BTreeMap<&NodeId, &str> = graph
        .nodes
        .iter()
        .map(|n| (&n.id, n.name.as_str()))
        .collect();
    slots.extend(
        graph
            .domain_flows
            .iter()
            .filter(|f| f.provenance == Provenance::Structural)
            .map(|f| {
                let step_names: Vec<String> = f
                    .steps
                    .iter()
                    .take(FLOW_SLOT_STEP_NAMES)
                    .map(|id| name_of.get(id).copied().unwrap_or(id.as_str()).to_string())
                    .collect();
                EnrichmentSlot::FlowName(FlowSlot {
                    id: f.id.clone(),
                    domain: f.domain.clone(),
                    entry: step_names.first().cloned().unwrap_or_default(),
                    step_names,
                    step_count: f.steps.len(),
                })
            }),
    );

    // Import fan-in/out per node — the same numbers the mechanical tour
    // labels cite.
    let mut fan_in: BTreeMap<&NodeId, usize> = BTreeMap::new();
    let mut fan_out: BTreeMap<&NodeId, usize> = BTreeMap::new();
    for edge in graph.edges.iter().filter(|e| e.kind == EdgeKind::Imports) {
        *fan_out.entry(&edge.source).or_default() += 1;
        *fan_in.entry(&edge.target).or_default() += 1;
    }
    let path_of: BTreeMap<&NodeId, &str> = graph
        .nodes
        .iter()
        .map(|n| (&n.id, n.path.as_str()))
        .collect();
    slots.extend(
        graph
            .tour
            .iter()
            .filter(|s| s.provenance == Provenance::Structural)
            .map(|s| {
                EnrichmentSlot::TourLabel(TourSlot {
                    node: s.node.clone(),
                    path: path_of
                        .get(&s.node)
                        .copied()
                        .unwrap_or(s.node.as_str())
                        .to_string(),
                    fan_in: fan_in.get(&s.node).copied().unwrap_or(0),
                    fan_out: fan_out.get(&s.node).copied().unwrap_or(0),
                    mechanical_label: s.label.clone(),
                })
            }),
    );

    slots
}

/// An answer usable for a slot: present and not blank — a blank or
/// whitespace-only answer is treated as unanswered, so a mechanical
/// fallback is never replaced by a hole.
fn answered<'a>(answers: &'a BTreeMap<String, String>, key: &str) -> Option<&'a str> {
    answers
        .get(key)
        .map(String::as_str)
        .filter(|text| !text.trim().is_empty())
}

/// Fills enrichment slots through the provider in batches of at most
/// [`BATCH_SIZE`] slots per request (spec: bounded prompts). Only
/// answered, non-blank slots change, flipping to `llm` provenance;
/// unanswered slots keep their mechanical text. Answers are addressed by
/// slot key (see [`EnrichmentSlot::key`]), so an answer can only land in
/// the slot it was written for. Any batch error fails the whole step: the
/// caller never saves a partially-purchased run.
pub fn fill_slots(graph: &mut KnowledgeGraph, provider: &dyn EnrichmentProvider) -> Result<usize> {
    let slots = collect_slots(graph);
    if slots.is_empty() {
        return Ok(0);
    }
    let mut answers: BTreeMap<String, String> = BTreeMap::new();
    for batch in slots.chunks(BATCH_SIZE) {
        let request = EnrichmentRequest {
            project: graph.project.name.clone(),
            slots: batch.to_vec(),
        };
        answers.extend(provider.enrich(&request)?.answers);
    }

    let mut count = 0;
    for node in &mut graph.nodes {
        if node.provenance != Provenance::Structural {
            continue;
        }
        if let Some(text) = answered(&answers, &summary_key(node.id.as_str())) {
            node.summary = text.to_string();
            node.provenance = Provenance::Llm;
            count += 1;
        }
    }
    for layer in &mut graph.layers {
        if layer.provenance != Provenance::Structural {
            continue;
        }
        if let Some(text) = answered(&answers, &layer_key(&layer.id)) {
            layer.name = text.to_string();
            layer.provenance = Provenance::Llm;
            count += 1;
        }
    }
    for flow in &mut graph.domain_flows {
        if flow.provenance != Provenance::Structural {
            continue;
        }
        if let Some(text) = answered(&answers, &flow_key(&flow.id)) {
            flow.name = text.to_string();
            flow.provenance = Provenance::Llm;
            count += 1;
        }
    }
    for step in &mut graph.tour {
        if step.provenance != Provenance::Structural {
            continue;
        }
        if let Some(text) = answered(&answers, &tour_key(step.node.as_str())) {
            step.label = text.to_string();
            step.provenance = Provenance::Llm;
            count += 1;
        }
    }
    Ok(count)
}

/// How the caller wants enrichment performed: which backend, and which model
/// within it. One type rather than two positional `Option<&str>` parameters
/// of the same type travelling together from the CLI down to
/// [`resolve_provider`], where a caller swapping them would compile and be
/// wrong.
#[derive(Debug, Clone, Copy)]
pub struct ProviderChoice<'a> {
    /// An explicit spec from `--provider`. `None` falls back to
    /// [`PROVIDER_ENV`], and then to the build's default.
    pub spec: Option<&'a str>,
    /// Forwarded to the Claude provider (`--model`); other backends ignore
    /// it.
    pub model: Option<&'a str>,
}

/// The provider specs this build can select, in the order they are offered
/// to a reader. Built from the compiled-in set rather than hardcoded,
/// because each feature adds its own — and naming a spec the binary cannot
/// select is worse than naming none.
pub fn recognised_specs() -> Vec<&'static str> {
    #[allow(unused_mut)]
    let mut specs: Vec<&'static str> = Vec::new();
    #[cfg(feature = "network")]
    specs.push("claude");
    #[cfg(feature = "test-provider")]
    {
        specs.push("fake:<path>");
        specs.push("fail");
    }
    specs
}

/// The one sentence that describes what this build can select, rendered from
/// [`recognised_specs`] and shared by every message that has to say it.
///
/// **Derived from the list, never from a feature name.** Keying the empty
/// case on `not(feature = "network")` reads correctly today and becomes a lie
/// the moment a second backend sits behind a second feature (ADR-0008): a
/// build with the CLI provider and no HTTP client would announce itself as
/// having no backend while one was working.
fn recognised_sentence() -> String {
    let specs = recognised_specs();
    if specs.is_empty() {
        return "This build recognises none: it was compiled without any \
                enrichment backend (ADR-0006 sealed build)."
            .to_string();
    }
    format!("This build recognises: {}.", specs.join(", "))
}

/// `--provider`'s help text. A function rather than a literal so the help a
/// reader sees describes the binary they are holding: a build with no backend
/// compiled in should say so, which is the whole point of the flag existing
/// there at all.
pub fn provider_help() -> String {
    #[allow(unused_mut)]
    let mut help = format!(
        "Enrichment backend, overriding {PROVIDER_ENV}. {}",
        recognised_sentence()
    );
    // The same condition [`default_provider`] uses, so the help cannot claim
    // a default the binary would not actually pick.
    #[cfg(all(feature = "network", not(feature = "test-provider")))]
    help.push_str(
        " Defaults to `claude`, the Claude API — credentials from \
         ANTHROPIC_API_KEY, or an `ant auth login` profile.",
    );
    help
}

/// `--model`'s help text. Build-aware for the same reason as
/// [`provider_help`]: where the Claude provider was not compiled in, this
/// flag modifies something absent, and a flag whose help describes a thing
/// that is not there is worse than one that says so.
pub fn model_help() -> String {
    if recognised_specs().contains(&"claude") {
        return "Model for the Claude enrichment provider (default: \
                claude-opus-5). Ignored by every other backend."
            .to_string();
    }
    format!(
        "Model for the Claude enrichment provider, which this build did not \
         compile in — so this flag has nothing to modify. {}",
        recognised_sentence()
    )
}

/// Told a name is wrong, a reader should also be told a right one — not
/// being told was the failure that kept the second credential path invisible.
fn unknown_provider(spec: &str) -> anyhow::Error {
    anyhow!(
        "unknown enrichment provider {spec:?}. {} The structural map was \
         written without enrichment.",
        recognised_sentence()
    )
}

/// Resolves the provider the CLI will use. An explicit `--provider` spec
/// wins; failing that [`PROVIDER_ENV`]; failing that the build's default.
/// Without a usable provider this fails with a clear message: the structural
/// map has already been written by the time this runs, so `--enrich`
/// degrades cleanly (spec story 14).
pub fn resolve_provider(choice: ProviderChoice<'_>) -> Result<Box<dyn EnrichmentProvider>> {
    let from_env = std::env::var(PROVIDER_ENV).ok();
    let spec = choice.spec.or(from_env.as_deref());
    provider_from_spec(spec, choice.model)
}

/// Selection by spec, separated from where the spec came from — the flag,
/// the environment, or nowhere — so it is unit-testable without touching a
/// process-global. Which surface wins is [`resolve_provider`]'s job; with no
/// spec at all the default depends on the build (see [`default_provider`]).
fn provider_from_spec(
    spec: Option<&str>,
    model: Option<&str>,
) -> Result<Box<dyn EnrichmentProvider>> {
    #[cfg(not(feature = "network"))]
    let _ = model;
    match spec {
        #[cfg(feature = "test-provider")]
        Some(spec) if spec.starts_with("fake:") => Ok(Box::new(test_provider::CannedProvider {
            path: spec["fake:".len()..].into(),
        })),
        #[cfg(feature = "test-provider")]
        Some("fail") => Ok(Box::new(test_provider::FailingProvider)),
        #[cfg(feature = "network")]
        Some("claude") => Ok(Box::new(claude::ClaudeProvider::new(model))),
        Some(other) => Err(unknown_provider(other)),
        None => default_provider(model),
    }
}

/// The provider used when [`PROVIDER_ENV`] is unset.
///
/// - **Shipped `network` build** — the Claude API provider (ADR-0004): the
///   one real backend is the default, no configuration needed.
/// - **Test build (`test-provider`)** — no default: tests must select a
///   backend explicitly, so no test can ever fall through to a provider
///   that opens sockets (the no-network-in-tests rule).
/// - **Sealed build (`--no-default-features`)** — no networking code exists
///   (ADR-0006); enrichment is simply not available.
fn default_provider(model: Option<&str>) -> Result<Box<dyn EnrichmentProvider>> {
    #[cfg(all(feature = "network", not(feature = "test-provider")))]
    {
        Ok(Box::new(claude::ClaudeProvider::new(model)))
    }
    #[cfg(feature = "test-provider")]
    {
        let _ = model;
        Err(anyhow!(
            "no enrichment provider is configured: this is a test build, which \
             has no default provider — set {PROVIDER_ENV}; the structural map \
             was written without enrichment"
        ))
    }
    #[cfg(not(any(feature = "network", feature = "test-provider")))]
    {
        let _ = model;
        Err(anyhow!(
            "enrichment is not available in this build: it was compiled without \
             the `network` feature (ADR-0006 sealed build), so no LLM backend \
             exists; the structural map was written without enrichment"
        ))
    }
}

#[cfg(test)]
mod tests {
    //! Tests at the provider-trait seam: a fake provider returns canned
    //! typed responses (or errors), and the assertions are about what
    //! reaches the provider and what lands in the graph's slots.

    use std::cell::RefCell;

    use super::*;
    use crate::map::{DomainFlow, Layer, Node, NodeKind, Project, TourStep};

    /// The recognised set is what `--provider`'s help offers and what an
    /// unknown-spec error lists, so it is worth pinning per configuration
    /// rather than only through the rendered help — clap wraps that at the
    /// terminal width.
    #[test]
    fn the_recognised_specs_are_the_ones_this_build_compiled_in() {
        let specs = recognised_specs();

        // Always true in a `cargo test` build: the self dev-dependency turns
        // `test-provider` on for every configuration.
        assert!(specs.contains(&"fail"), "test backends missing: {specs:?}");

        assert_eq!(
            specs.contains(&"claude"),
            cfg!(feature = "network"),
            "the Claude provider is offered exactly when it is compiled in: \
             {specs:?}"
        );
    }

    #[test]
    fn an_unknown_spec_is_reported_with_the_alternatives() {
        let message = unknown_provider("nope").to_string();

        assert!(
            message.contains("nope"),
            "must name the bad spec: {message}"
        );
        for spec in recognised_specs() {
            assert!(
                message.contains(spec),
                "must list {spec}, which this build accepts: {message}"
            );
        }
    }

    #[test]
    fn an_unrecognised_spec_never_falls_through_to_the_default() {
        // Selecting *which* surface a spec came from is `resolve_provider`'s
        // job and is covered at the CLI boundary, because it reads a
        // process-global. What is testable here is that a bad spec is an
        // error rather than a silent fall-through to whatever the build's
        // default happens to be — which on a shipped binary would open a
        // socket the reader never asked for.
        assert!(
            provider_from_spec(Some("fail"), None).is_ok(),
            "a recognised spec must be selectable"
        );
        assert!(
            provider_from_spec(Some("definitely-not-a-provider"), None).is_err(),
            "an unknown spec must not fall through to the default"
        );
    }

    /// The list a reader is offered names the Claude backend exactly when the
    /// build has it. Asserted on the rendered sentence rather than on
    /// [`recognised_specs`] alone, because a rendering that silently dropped
    /// the list would leave that test green.
    #[test]
    fn the_offered_list_names_claude_exactly_when_it_is_compiled_in() {
        assert_eq!(
            recognised_sentence().contains("claude"),
            cfg!(feature = "network"),
            "the offered list must match the build: {}",
            recognised_sentence()
        );
    }

    /// Every message that names alternatives renders the one sentence, so a
    /// build cannot end up describing its backends two different ways — which
    /// is how `--enrich` and `--model` came to claim a Claude provider that a
    /// sealed build does not have.
    #[test]
    fn every_message_that_lists_backends_renders_the_same_sentence() {
        let sentence = recognised_sentence();
        assert!(
            provider_help().contains(&sentence),
            "--provider help must render the shared sentence: {}",
            provider_help()
        );
        let error = unknown_provider("nope").to_string();
        assert!(
            error.contains(&sentence),
            "the unknown-spec error must render the shared sentence: {error}"
        );
    }

    /// `--model` modifies the Claude provider and nothing else, so where that
    /// provider was not compiled in the flag has to say so rather than
    /// describing itself as though it worked.
    #[test]
    fn model_help_admits_when_the_claude_provider_is_absent() {
        assert_eq!(
            model_help().contains("did not compile in"),
            !cfg!(feature = "network"),
            "--model must admit an absent provider, and only then: {}",
            model_help()
        );
    }

    fn node(id: NodeId, kind: NodeKind, name: &str, path: &str, provenance: Provenance) -> Node {
        Node {
            id,
            kind,
            name: name.into(),
            path: path.into(),
            summary: format!("Mechanical summary of {name}"),
            range: None,
            layer: None,
            provenance,
        }
    }

    fn graph() -> KnowledgeGraph {
        KnowledgeGraph {
            version: crate::map::MAP_CONTRACT_VERSION.into(),
            project: Project {
                name: "demo".into(),
            },
            nodes: vec![
                node(
                    NodeId::file("src/a.ts"),
                    NodeKind::File,
                    "a.ts",
                    "src/a.ts",
                    Provenance::Structural,
                ),
                node(
                    NodeId::symbol(NodeKind::Function, "src/a.ts", "go"),
                    NodeKind::Function,
                    "go",
                    "src/a.ts",
                    Provenance::Structural,
                ),
                node(
                    NodeId::file("src/b.ts"),
                    NodeKind::File,
                    "b.ts",
                    "src/b.ts",
                    Provenance::Llm,
                ),
            ],
            edges: Vec::new(),
            layers: Vec::new(),
            domain_flows: Vec::new(),
            tour: Vec::new(),
        }
    }

    /// [`graph`] plus one mechanical layer, flow, and tour step — the
    /// semantic slots ticket 06 derives and this ticket enriches.
    fn graph_with_semantics() -> KnowledgeGraph {
        let mut graph = graph();
        graph.layers = vec![Layer {
            id: "src".into(),
            name: "src".into(),
            provenance: Provenance::Structural,
        }];
        graph.domain_flows = vec![DomainFlow {
            id: "flow:function:src/a.ts:go".into(),
            name: "go".into(),
            domain: "src".into(),
            steps: vec![NodeId::symbol(NodeKind::Function, "src/a.ts", "go")],
            provenance: Provenance::Structural,
        }];
        graph.tour = vec![TourStep {
            node: NodeId::file("src/a.ts"),
            label: "Entry point: src/a.ts — fan-in 0, fan-out 0".into(),
            provenance: Provenance::Structural,
        }];
        graph
    }

    /// Canned answers plus a recording of every request's slot keys.
    struct Fake {
        answers: BTreeMap<String, String>,
        requested: RefCell<Vec<String>>,
    }

    impl EnrichmentProvider for Fake {
        fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
            self.requested
                .borrow_mut()
                .extend(request.slots.iter().map(EnrichmentSlot::key));
            Ok(EnrichmentResponse {
                answers: self.answers.clone(),
            })
        }
    }

    #[test]
    fn only_structural_slots_are_offered_and_answers_land_in_them() {
        let mut graph = graph();
        let fake = Fake {
            answers: BTreeMap::from([(
                "summary:function:src/a.ts:go".to_string(),
                "Runs the whole show.".to_string(),
            )]),
            requested: RefCell::new(Vec::new()),
        };

        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 1);

        // The already-enriched node was never offered to the provider.
        assert_eq!(
            *fake.requested.borrow(),
            vec![
                "summary:file:src/a.ts".to_string(),
                "summary:function:src/a.ts:go".to_string()
            ],
            "request must contain exactly the structural slots"
        );

        // The answered slot is filled and flipped; the unanswered slot
        // keeps its mechanical fallback.
        let go = &graph.nodes[1];
        assert_eq!(go.summary, "Runs the whole show.");
        assert_eq!(go.provenance, Provenance::Llm);
        let a = &graph.nodes[0];
        assert_eq!(a.summary, "Mechanical summary of a.ts");
        assert_eq!(a.provenance, Provenance::Structural);
    }

    #[test]
    fn semantic_slots_are_offered_and_answers_land_in_the_right_slots() {
        let mut graph = graph_with_semantics();
        let fake = Fake {
            answers: BTreeMap::from([
                ("layer-name:src".to_string(), "Application core".to_string()),
                (
                    "flow-name:flow:function:src/a.ts:go".to_string(),
                    "Greeting flow".to_string(),
                ),
                (
                    "tour-label:file:src/a.ts".to_string(),
                    "Start here: the app's front door.".to_string(),
                ),
            ]),
            requested: RefCell::new(Vec::new()),
        };

        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 3, "three semantic answers must count as enrichment");

        // The semantic slots were offered alongside the node slots.
        let requested = fake.requested.borrow();
        for key in [
            "layer-name:src",
            "flow-name:flow:function:src/a.ts:go",
            "tour-label:file:src/a.ts",
        ] {
            assert!(
                requested.iter().any(|k| k == key),
                "slot {key} was never offered: {requested:?}"
            );
        }

        // Each answer landed in its own slot and flipped its provenance.
        assert_eq!(graph.layers[0].name, "Application core");
        assert_eq!(graph.layers[0].provenance, Provenance::Llm);
        assert_eq!(graph.domain_flows[0].name, "Greeting flow");
        assert_eq!(graph.domain_flows[0].provenance, Provenance::Llm);
        assert_eq!(graph.tour[0].label, "Start here: the app's front door.");
        assert_eq!(graph.tour[0].provenance, Provenance::Llm);

        // No node was touched by a semantic answer.
        for node in &graph.nodes[..2] {
            assert!(node.summary.starts_with("Mechanical summary"));
            assert_eq!(node.provenance, Provenance::Structural);
        }
    }

    #[test]
    fn slot_addressing_is_collision_proof_across_kinds() {
        // `file:src/a.ts` identifies both a node summary slot and a tour
        // label slot; the prefixed keys keep the namespaces apart. And an
        // answer under a bare node ID (the pre-slot-kind format) matches
        // nothing at all.
        let mut graph = graph_with_semantics();
        let fake = Fake {
            answers: BTreeMap::from([
                (
                    "summary:file:src/a.ts".to_string(),
                    "Node prose.".to_string(),
                ),
                (
                    "tour-label:file:src/a.ts".to_string(),
                    "Tour prose.".to_string(),
                ),
                (
                    "function:src/a.ts:go".to_string(),
                    "Unprefixed — must land nowhere.".to_string(),
                ),
            ]),
            requested: RefCell::new(Vec::new()),
        };

        fill_slots(&mut graph, &fake).unwrap();

        assert_eq!(graph.nodes[0].summary, "Node prose.");
        assert_eq!(graph.tour[0].label, "Tour prose.");
        assert_eq!(
            graph.nodes[1].summary, "Mechanical summary of go",
            "an unprefixed answer must not land in any slot"
        );
        assert_eq!(graph.nodes[1].provenance, Provenance::Structural);
        // The layer and flow got no answers: mechanical, structural.
        assert_eq!(graph.layers[0].name, "src");
        assert_eq!(graph.layers[0].provenance, Provenance::Structural);
        assert_eq!(graph.domain_flows[0].name, "go");
        assert_eq!(graph.domain_flows[0].provenance, Provenance::Structural);
    }

    #[test]
    fn enriched_semantic_slots_are_not_reoffered() {
        let mut graph = graph_with_semantics();
        graph.layers[0].provenance = Provenance::Llm;
        graph.domain_flows[0].provenance = Provenance::Llm;
        graph.tour[0].provenance = Provenance::Llm;

        let fake = Fake {
            answers: BTreeMap::from([
                ("layer-name:src".to_string(), "MUST NOT APPLY".to_string()),
                (
                    "flow-name:flow:function:src/a.ts:go".to_string(),
                    "MUST NOT APPLY".to_string(),
                ),
                (
                    "tour-label:file:src/a.ts".to_string(),
                    "MUST NOT APPLY".to_string(),
                ),
            ]),
            requested: RefCell::new(Vec::new()),
        };

        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 0);

        // Carried-over semantics are never re-sent (ADR-0005) …
        for key in fake.requested.borrow().iter() {
            assert!(
                key.starts_with("summary:"),
                "an enriched semantic slot was re-offered: {key}"
            );
        }
        // … and an uninvited answer for them never lands.
        assert_eq!(graph.layers[0].name, "src");
        assert_eq!(graph.domain_flows[0].name, "go");
        assert_eq!(
            graph.tour[0].label,
            "Entry point: src/a.ts — fan-in 0, fan-out 0"
        );
    }

    #[test]
    fn flow_slots_stay_bounded_however_long_the_chain_grows() {
        /// Records every slot offered, answering nothing.
        struct Recording {
            slots: RefCell<Vec<EnrichmentSlot>>,
        }
        impl EnrichmentProvider for Recording {
            fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
                self.slots
                    .borrow_mut()
                    .extend(request.slots.iter().cloned());
                Ok(EnrichmentResponse::default())
            }
        }

        let mut graph = graph_with_semantics();
        graph.domain_flows[0].steps = (0..40)
            .map(|i| NodeId::symbol(NodeKind::Function, "src/a.ts", &format!("f{i}")))
            .collect();

        let provider = Recording {
            slots: RefCell::new(Vec::new()),
        };
        fill_slots(&mut graph, &provider).unwrap();

        let slots = provider.slots.borrow();
        let flow = slots
            .iter()
            .find_map(|s| match s {
                EnrichmentSlot::FlowName(f) => Some(f),
                _ => None,
            })
            .expect("the flow slot must be offered");
        assert_eq!(
            flow.step_names.len(),
            FLOW_SLOT_STEP_NAMES,
            "a flow slot must not grow with the call chain"
        );
        assert_eq!(flow.step_count, 40, "the total step count is still stated");
    }

    #[test]
    fn requests_are_batched_to_at_most_batch_size_slots() {
        /// Echoes an answer for every offered slot while recording the size
        /// of each request — the bounded-prompt property observed at the
        /// provider seam.
        struct Batching {
            request_sizes: RefCell<Vec<usize>>,
        }
        impl EnrichmentProvider for Batching {
            fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
                self.request_sizes.borrow_mut().push(request.slots.len());
                Ok(EnrichmentResponse {
                    answers: request
                        .slots
                        .iter()
                        .map(|s| (s.key(), format!("Prose for {}", s.key())))
                        .collect(),
                })
            }
        }

        let total = 2 * BATCH_SIZE + 3;
        let mut graph = graph();
        graph.nodes = (0..total)
            .map(|i| {
                let path = format!("src/f{i}.ts");
                node(
                    NodeId::file(&path),
                    NodeKind::File,
                    &format!("f{i}.ts"),
                    &path,
                    Provenance::Structural,
                )
            })
            .collect();

        let provider = Batching {
            request_sizes: RefCell::new(Vec::new()),
        };
        let count = fill_slots(&mut graph, &provider).unwrap();
        assert_eq!(count, total);

        let sizes = provider.request_sizes.borrow();
        assert!(
            sizes.iter().all(|&n| n <= BATCH_SIZE),
            "a request exceeded the batch bound {BATCH_SIZE}: {sizes:?}"
        );
        assert_eq!(
            sizes.iter().sum::<usize>(),
            total,
            "every slot must be offered exactly once: {sizes:?}"
        );
        assert_eq!(sizes.len(), total.div_ceil(BATCH_SIZE));
        for n in &graph.nodes {
            assert_eq!(n.provenance, Provenance::Llm);
        }
    }

    #[test]
    fn blank_answers_never_replace_the_mechanical_summary() {
        let mut graph = graph();
        let fake = Fake {
            answers: BTreeMap::from([
                ("summary:file:src/a.ts".to_string(), "".to_string()),
                (
                    "summary:function:src/a.ts:go".to_string(),
                    " \n\t ".to_string(),
                ),
            ]),
            requested: RefCell::new(Vec::new()),
        };

        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 0, "blank answers must not count as enrichment");

        // Both slots keep the mechanical fallback and stay structural, so
        // the next --enrich re-selects them.
        for node in &graph.nodes[..2] {
            assert!(node.summary.starts_with("Mechanical summary"));
            assert_eq!(node.provenance, Provenance::Structural);
        }
    }

    #[test]
    fn blank_semantic_answers_keep_the_mechanical_labels() {
        let mut graph = graph_with_semantics();
        let fake = Fake {
            answers: BTreeMap::from([
                ("layer-name:src".to_string(), "".to_string()),
                (
                    "flow-name:flow:function:src/a.ts:go".to_string(),
                    "  ".to_string(),
                ),
                ("tour-label:file:src/a.ts".to_string(), "\n\t".to_string()),
            ]),
            requested: RefCell::new(Vec::new()),
        };

        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 0, "blank answers must not count as enrichment");

        assert_eq!(graph.layers[0].name, "src");
        assert_eq!(graph.layers[0].provenance, Provenance::Structural);
        assert_eq!(graph.domain_flows[0].name, "go");
        assert_eq!(graph.domain_flows[0].provenance, Provenance::Structural);
        assert_eq!(
            graph.tour[0].label,
            "Entry point: src/a.ts — fan-in 0, fan-out 0"
        );
        assert_eq!(graph.tour[0].provenance, Provenance::Structural);
    }

    #[test]
    fn an_answer_for_an_unoffered_node_is_ignored() {
        let mut graph = graph();
        let fake = Fake {
            answers: BTreeMap::from([(
                "summary:file:src/b.ts".to_string(),
                "Sneaky overwrite.".to_string(),
            )]),
            requested: RefCell::new(Vec::new()),
        };

        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 0);
        assert_eq!(graph.nodes[2].summary, "Mechanical summary of b.ts");
    }

    #[test]
    fn a_provider_error_propagates_and_leaves_every_slot_untouched() {
        struct Failing;
        impl EnrichmentProvider for Failing {
            fn enrich(&self, _request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
                Err(anyhow!("boom"))
            }
        }

        let mut graph = graph();
        let err = fill_slots(&mut graph, &Failing).unwrap_err();
        assert!(err.to_string().contains("boom"));
        for node in &graph.nodes {
            assert!(node.summary.starts_with("Mechanical summary"));
        }
        assert_eq!(graph.nodes[0].provenance, Provenance::Structural);
    }
}

/// Offline provider backends compiled in only for test builds; see the
/// module docs. No code here can open a socket.
#[cfg(feature = "test-provider")]
mod test_provider {
    use super::*;
    use anyhow::Context;
    use std::path::PathBuf;

    /// Canned typed responses from a JSON file keyed by slot address:
    /// `{ "<slot-key>": "<text>" }`, where a slot key is
    /// `summary:<node-id>`, `layer-name:<layer-id>`, `flow-name:<flow-id>`,
    /// or `tour-label:<node-id>` (see [`EnrichmentSlot::key`]). A key with
    /// no prefix — or the wrong prefix — never matches any slot.
    pub struct CannedProvider {
        pub path: PathBuf,
    }

    impl EnrichmentProvider for CannedProvider {
        fn enrich(&self, _request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
            let raw = fs::read_to_string(&self.path)
                .with_context(|| format!("cannot read canned responses {:?}", self.path))?;
            let answers: BTreeMap<String, String> = serde_json::from_str(&raw)?;
            Ok(EnrichmentResponse { answers })
        }
    }

    /// Errors on every call — failure injection for spec story 14.
    pub struct FailingProvider;

    impl EnrichmentProvider for FailingProvider {
        fn enrich(&self, _request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
            Err(anyhow!("injected provider failure"))
        }
    }
}
