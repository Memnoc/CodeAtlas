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
//!   `summary:<node-id>`, `layer-name:<layer-id>`,
//!   `layer-description:<layer-id>`, `flow-name:<flow-id>`,
//!   `tour-label:<node-id>`
//! - `fail` — a provider that errors on every call (failure injection,
//!   spec story 14)
//!
//! Neither backend can open a socket; no test performs network I/O.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::map::{EdgeKind, KnowledgeGraph, NodeId, NodeKind, Provenance};

#[cfg(feature = "agent-cli")]
pub mod agent_cli;
/// Questions about a map, answered from a bounded slice of it (ADR-0009).
/// Ungated: selection is pure and reaches nothing, and a build with no
/// backend simply cannot get as far as asking.
pub mod ask;
#[cfg(feature = "network")]
pub mod claude;
/// What the model is asked and how its answer is read — shared by every
/// backend, so `docs/SECURITY.md`'s statement of what a model receives has
/// one place to be true rather than one per transport.
#[cfg(any(feature = "network", feature = "agent-cli"))]
mod prompt;

/// The env var the CLI resolves its enrichment provider from.
pub const PROVIDER_ENV: &str = "CODEATLAS_ENRICH_PROVIDER";

/// The most slots a single provider request may carry (spec: bounded
/// prompts — the model never sees the whole serialized graph, and a
/// request's size cannot grow with the repository). 25 slots keep the
/// prompt at a few KB and the structured response comfortably inside one
/// completion; larger repos simply make more requests.
pub const BATCH_SIZE: usize = 25;

/// How many batches are in flight at once.
///
/// Deliberately small. The ceiling here is not this machine — it is the
/// reader's rate limit, and the cost of guessing too high is their money or
/// their subscription allowance, not a slow run: a backend that starts
/// refusing turns a long enrichment into a failed one. Four takes a measured
/// 44-minute run on this repository (64 batches at ~41s) to something under
/// twelve, which is enough of a win not to need pushing.
///
/// Every backend this can drive is a network call or a process spawn, so the
/// threads are asleep almost all of the time; this is not sized against cores
/// and must not be.
pub const ENRICH_CONCURRENCY: usize = 4;

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
    /// A structural layer's prose description (ticket 07). Carries the same
    /// [`LayerSlot`] the name slot does — the layer ID and its file count,
    /// never the member list — because the two are the same bounded question
    /// about the same topology, asked for different prose.
    LayerDescription(LayerSlot),
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
            Self::LayerDescription(s) => layer_description_key(&s.id),
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

fn layer_description_key(layer_id: &str) -> String {
    format!("layer-description:{layer_id}")
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

    /// Answers a question about the map (ADR-0009), from the bounded slice
    /// the question carries and nothing else.
    ///
    /// Defaulted rather than required. Both real backends implement it, but
    /// the trait has a dozen implementors between the test doubles and the
    /// offline backends, and making every one of them able to answer
    /// questions in order to compile would be a tax paid for nothing. A
    /// backend that has not implemented questions says so plainly here, so
    /// the absence surfaces as a message rather than as an empty answer.
    fn ask(&self, _question: &ask::Question) -> Result<ask::Answer> {
        Err(anyhow!(
            "this enrichment backend cannot answer questions about the map"
        ))
    }

    /// How this backend names itself where its prose is recorded (ADR-0007).
    ///
    /// The backend answers rather than the selection code, because only the
    /// backend knows the model actually used: `--model` is optional, and each
    /// has its own answer to being given none — the API provider pins
    /// `claude-opus-5`, the CLI provider leaves the choice to the
    /// subscription. Reconstructing that from the spec would be a guess.
    ///
    /// Defaulted for the same reason [`ask`](Self::ask) is: most implementors
    /// are test doubles that never reach a store.
    fn identity(&self) -> ProviderIdentity {
        ProviderIdentity::unnamed()
    }
}

/// A provider ready to be shared between the threads of `serve --ask`.
///
/// The auto-trait bounds live on the boxed selection rather than on
/// [`EnrichmentProvider`] itself: every spec-selectable backend holds a string
/// or a path and is trivially both, and a trait that demanded them of every
/// implementor would be demanding them of implementors outside this crate for
/// no reason of its own.
///
/// `Sync` alone is not confined here, and the comment that used to say so was
/// made false by [`fill_slots_with`] running batches concurrently: the test
/// doubles that record what they were asked used to do it through a `RefCell`,
/// and now do it through a `Mutex`. That is the whole cost, and it is the
/// right side of the trade — a double that cannot cross a thread cannot
/// observe the concurrency it is being used to test.
pub type SelectedProvider = Box<dyn EnrichmentProvider + Send + Sync>;

/// The same provider once `serve --ask` has to hand it to every connection
/// thread. One thread per connection is what makes the bounds necessary at
/// all; [`SelectedProvider`] converts into this directly.
pub type SharedProvider = std::sync::Arc<dyn EnrichmentProvider + Send + Sync>;

/// The annotation store's file name under [`crate::scan::OUTPUT_DIR`]. The
/// store is internal — NOT part of the map contract — but deterministic
/// (sorted keys) and versioned so its format can evolve.
pub const ANNOTATIONS_FILE: &str = "annotations.json";

/// Bumped whenever the store format changes in a way that *invalidates*
/// stored data — a hash definition, a key shape, the meaning of a field. A
/// store with another version is ignored, which costs a re-enrichment, so the
/// bump is a bill and is only worth sending when the data would otherwise be
/// wrong. 2: added the semantic sections (`layers`, `flows`, `tour`) keyed by
/// derivation-input hashes.
///
/// Purely additive optional fields do not bump it. [`ProducedBy`] (ADR-0007)
/// is the first: a store written without it holds annotations that are still
/// correct, and charging every existing repository a re-enrichment to learn
/// one date would be a worse outcome than not knowing the date. The
/// `layer_descriptions` section (ticket 07) is the second, by the same rule:
/// a store without it holds nothing wrong — it merely has no descriptions to
/// offer, and the next `--enrich` buys exactly those — while a bump would
/// discard every purchased summary and name to learn prose the run could
/// have bought incrementally.
const STORE_VERSION: u32 = 2;

/// The carry-over store (ADR-0005): enrichment prose keyed by identity
/// plus a hash of what derived it. Node annotations key on the node ID
/// (which embeds the repo-relative path) plus the node's file content
/// hash. Semantic annotations — layer names and descriptions, flows, tour
/// steps — are not file-backed, so they key on their semantic identity
/// (layer ID, flow ID, tour node ID) plus a hash of the mechanical inputs
/// that derived them: the layer's sorted member set (shared by its name
/// and its description), the flow's step ID chain, the tour step's
/// mechanical label (path + import fan-in/out + entry-point status).
/// Annotations re-attach for free while their derivation is unchanged and
/// expire the moment it changes — stale prose never describes new code or
/// a new shape.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AnnotationStore {
    version: u32,
    /// What produced the prose below (ADR-0007). Optional because a store
    /// written before ticket 30 has none, and such a store must keep
    /// re-attaching rather than be discarded over a missing label.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    produced_by: Option<ProducedBy>,
    annotations: BTreeMap<String, Annotation>,
    #[serde(default)]
    layers: BTreeMap<String, SemanticAnnotation>,
    /// Purchased layer descriptions (ticket 07), keyed by layer ID and
    /// carried by the SAME derivation-input hash the layer's name uses — the
    /// sorted member set. Its own section rather than a second field on the
    /// name's, because the two are separate purchases that expire together
    /// but are bought apart. `default` because a store written before this
    /// section existed must keep re-attaching everything it holds.
    #[serde(default)]
    layer_descriptions: BTreeMap<String, SemanticAnnotation>,
    #[serde(default)]
    flows: BTreeMap<String, SemanticAnnotation>,
    #[serde(default)]
    tour: BTreeMap<String, SemanticAnnotation>,
}

/// Which backend, which model, and when — recorded because a committed store
/// (ADR-0007) puts LLM prose into code review, and a reviewer reading that
/// diff is entitled to know what wrote it.
///
/// One record for the store rather than one per annotation. The store is
/// rebuilt wholesale from the enriched graph on every purchasing run, and a
/// carried-over annotation's original run is not recoverable from the graph —
/// so per-annotation provenance would be honest only for the slots that run
/// happened to buy, and quietly wrong for the rest. This says what the last
/// run to write the store was, which is a claim that stays true.
///
/// The backend's half is held as a whole [`ProviderIdentity`] and flattened
/// into the same JSON object rather than copied across field by field: the
/// two structs otherwise differ by exactly one field, and a hand transcription
/// between them is a place for a third field to be added to one and forgotten
/// in the other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ProducedBy {
    /// Which backend, and which model within it.
    #[serde(flatten)]
    identity: ProviderIdentity,
    /// UTC calendar date, `YYYY-MM-DD`. A date and not a timestamp: this is
    /// read by a person in a diff, and a second-resolution clock would churn
    /// the file on every run that changed nothing else.
    date: String,
}

impl ProducedBy {
    /// What a run through `identity` produced, dated today.
    fn today(identity: ProviderIdentity) -> Self {
        Self {
            identity,
            date: today_utc(),
        }
    }
}

/// How a backend names itself in a written store. Defaulted on the trait, so
/// the dozen test doubles that will never write one owe nothing.
///
/// Serializable because it *is* the provenance record's provider half
/// ([`ProducedBy`]), flattened in place — `provider` and `model` sit directly
/// in `produced_by`, exactly as they read before this type owned them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderIdentity {
    /// The spec a reader would pass to `--provider` to get this backend, e.g.
    /// `claude` or `cli:claude`.
    pub provider: String,
    /// The model it used, where it names one. A CLI backend left on its
    /// subscription's own default names none, and inventing one here would be
    /// a guess presented as a record.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl ProviderIdentity {
    /// The identity of a backend that has not said who it is.
    fn unnamed() -> Self {
        Self {
            provider: "unknown".to_string(),
            model: None,
        }
    }
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
    /// whose derivation-input hash still matches gets its enriched name,
    /// description, or label back the same way. Everything else stays
    /// structural — an
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

        if self.layers.is_empty()
            && self.layer_descriptions.is_empty()
            && self.flows.is_empty()
            && self.tour.is_empty()
        {
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
            // The description rides the same hash (ticket 07): while the
            // membership is unchanged it re-attaches free, and the moment
            // it changes the mechanical sentence stands.
            if let Some(a) = self.layer_descriptions.get(&layer.id)
                && hashes.layers.get(&layer.id) == Some(&a.inputs_hash)
            {
                layer.description = Some(crate::map::LayerDescription {
                    text: a.text.clone(),
                    provenance: Provenance::Llm,
                });
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
///
/// `identity` is the backend this run purchased through; it is recorded with
/// today's date so the committed store says what wrote it (ADR-0007).
fn save_store(root: &Path, graph: &KnowledgeGraph, identity: ProviderIdentity) -> Result<()> {
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
    let layer_descriptions: BTreeMap<String, SemanticAnnotation> = graph
        .layers
        .iter()
        .filter_map(|l| {
            let description = l.description.as_ref()?;
            if description.provenance != Provenance::Llm {
                return None;
            }
            Some((
                l.id.clone(),
                SemanticAnnotation {
                    inputs_hash: semantic.layers.get(&l.id)?.clone(),
                    text: description.text.clone(),
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
        produced_by: Some(ProducedBy::today(identity)),
        annotations,
        layers,
        layer_descriptions,
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

/// Today's UTC date as `YYYY-MM-DD`, for the store's provenance record.
///
/// A clock that cannot be read is not worth failing an enrichment run over,
/// so an unreadable one yields the epoch — a date obviously wrong to a
/// reader, rather than a plausible-looking lie.
fn today_utc() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs());
    civil_date(i64::try_from(seconds / 86_400).unwrap_or(0))
}

/// Days since 1970-01-01 → `YYYY-MM-DD`, by Howard Hinnant's
/// `civil_from_days`. Hand-rolled because ADR-0006 admits no new dependency
/// and a calendar crate would be a lot of supply chain for one line of a JSON
/// file; the algorithm is well known and pinned by
/// [`the_calendar_survives_leap_years_and_century_boundaries`].
///
/// The era arithmetic shifts the year to start in March, which puts the leap
/// day at the end of a 146097-day (400-year) era and makes every case fall
/// out of the same expression.
fn civil_date(days: i64) -> String {
    let shifted = days + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let march_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * march_month + 2) / 5 + 1;
    let month = if march_month < 10 {
        march_month + 3
    } else {
        march_month - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
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

/// What an enrichment run is about to do, worked out before any of it is
/// done: the batches it will send, and therefore what it will cost.
///
/// **Built by the same function that runs it.** [`fill_slots_with`] enriches
/// exactly `requests`, and both it and the estimate reach them through
/// [`Plan::of`], so the number a reader is quoted cannot drift from the number
/// of calls they are charged for. A separate estimator would agree today and
/// be a confident lie the first time batching changed, which is worse than
/// quoting nothing.
pub struct Plan {
    pub requests: Vec<EnrichmentRequest>,
    slots: usize,
}

impl Plan {
    /// The batches needed to fill every structural-provenance slot in `graph`.
    pub fn of(graph: &KnowledgeGraph) -> Self {
        let slots = collect_slots(graph);
        Self {
            requests: slots
                .chunks(BATCH_SIZE)
                .map(|batch| EnrichmentRequest {
                    project: graph.project.name.clone(),
                    slots: batch.to_vec(),
                })
                .collect(),
            slots: slots.len(),
        }
    }

    /// How many slots will be filled.
    pub fn slots(&self) -> usize {
        self.slots
    }

    /// How many provider calls will be made.
    pub fn calls(&self) -> usize {
        self.requests.len()
    }

    /// Characters of prompt this run will send, counted off the real thing:
    /// every backend composes its request through [`prompt::for_enrichment`],
    /// and this measures what that returns. The schema is serialized rather
    /// than guessed at because it is sent on every call and is not small.
    ///
    /// Absent from a sealed build, along with the prompt builder itself. That
    /// is the honest shape: a binary compiled without any backend has no
    /// prompt to measure, and a zero here would be a number rather than an
    /// absence. It is unreachable there in any case — `run` resolves a
    /// provider, and fails, before it estimates anything.
    #[cfg(any(feature = "network", feature = "agent-cli"))]
    pub fn prompt_chars(&self) -> usize {
        self.requests
            .iter()
            .map(|request| {
                let completion = prompt::for_enrichment(request);
                completion.system_prompt.len()
                    + completion.user_message.len()
                    + completion.schema.to_string().len()
            })
            .sum()
    }

    /// One line saying what the run will do, for a reader deciding whether to
    /// let it. Three things it deliberately does not say:
    ///
    /// - **An exact token count.** There is no local Anthropic tokenizer, and
    ///   adding a crate for one is what ADR-0006's bound on the dependency
    ///   surface exists to prevent. So this is characters over a divisor, and
    ///   it is rendered as a range because that is what it is. A single
    ///   figure would be quoted back as though it had been measured.
    /// - **A price.** Rates change, so a number compiled in here is a future
    ///   false claim — and on `cli:claude` there is no monetary cost at all,
    ///   only subscription allowance. Calls and tokens are the facts; what
    ///   they are worth is the reader's to know.
    /// - **A total.** The prompt figure is computed from real prompts. The
    ///   output figure is a guess about how long a model's sentence runs.
    ///   Adding them would launder the second into the first.
    pub fn describe(&self) -> String {
        #[cfg(any(feature = "network", feature = "agent-cli"))]
        {
            let chars = self.prompt_chars();
            format!(
                "{} slots in {} calls: roughly {}–{} tokens of prompt, plus perhaps \
                 {}–{} more coming back",
                self.slots,
                self.calls(),
                thousands(chars / TOKEN_CHARS_HIGH),
                thousands(chars / TOKEN_CHARS_LOW),
                thousands(self.slots * ANSWER_TOKENS_LOW),
                thousands(self.slots * ANSWER_TOKENS_HIGH),
            )
        }
        // A sealed build knows the two exact numbers and has no prompt builder
        // to measure the rest with, so it says the two and stops. Unreachable
        // in practice — nothing gets past `resolve_provider` there — and it is
        // still better for this to be a shorter true sentence than a longer
        // one with an invented figure in it.
        #[cfg(not(any(feature = "network", feature = "agent-cli")))]
        {
            format!("{} slots in {} calls", self.slots, self.calls())
        }
    }
}

/// The character-per-token divisors bracketing the estimate. English prose
/// runs near four and dense punctuated JSON nearer three, and slot payloads
/// are both — so the two are used as the ends of a range rather than one of
/// them being picked and presented as the answer.
#[cfg(any(feature = "network", feature = "agent-cli"))]
const TOKEN_CHARS_LOW: usize = 3;
#[cfg(any(feature = "network", feature = "agent-cli"))]
const TOKEN_CHARS_HIGH: usize = 4;

/// How long one filled slot's answer runs. A guess, and labelled as one
/// wherever it surfaces: the schema bounds an answer to a sentence, which is
/// a shape rather than a length.
#[cfg(any(feature = "network", feature = "agent-cli"))]
const ANSWER_TOKENS_LOW: usize = 25;
#[cfg(any(feature = "network", feature = "agent-cli"))]
const ANSWER_TOKENS_HIGH: usize = 45;

/// `137672` as `138k`. Precision this estimate does not have would read as
/// precision it does.
#[cfg(any(feature = "network", feature = "agent-cli"))]
fn thousands(n: usize) -> String {
    if n < 1000 {
        format!("{n}")
    } else {
        format!("{}k", n / 1000)
    }
}

/// What the `--enrich` step did — the CLI words its success message from
/// this, so "no provider was needed" and "the provider answered" stay
/// distinguishable.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    /// No structural-provenance slot needed filling (empty map, or every
    /// summary, layer name, layer description, flow name, and tour label
    /// already enriched or carried over): no provider was resolved and no
    /// request was made.
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
    let identity = provider.identity();
    // Before the first call, not behind a flag. This is the longest thing
    // CodeAtlas does and the only thing that spends the reader's money or
    // subscription allowance, and a reader who has to know to ask for the
    // figure is a reader who finds it out afterwards — which is exactly how
    // this run was agreed to on an estimate of "about a dozen calls" when the
    // real number was sixty-four.
    eprintln!("enriching: {}", Plan::of(graph).describe());
    // The store is written after every batch, so an interrupted or failed run
    // keeps everything it already paid for: the next `--enrich` reattaches
    // those annotations by content hash and asks only for what is left. The
    // *map* is still written once, at the end, and only on success — so a
    // partially enriched `knowledge-graph.json` is never observable, which is
    // the guarantee the old all-or-nothing version was really defending.
    let count = fill_slots_with(graph, provider.as_ref(), &mut |graph, batch| {
        save_store(root, graph, identity.clone())?;
        // One line per batch, the same on a terminal and in a log. A
        // `\r`-updating line would be prettier on a TTY and unreadable when
        // piped, and the branch would leave one of the two forms asserted by
        // nothing; sixty-four lines over ten minutes is not spam.
        eprintln!(
            "  batch {}/{} — {} slots filled",
            batch.done, batch.total, batch.filled
        );
        Ok(())
    })?;
    crate::scan::save(root, graph)?;
    Ok(Outcome::Enriched(count))
}

/// The slots the provider would be asked to fill: only
/// `structural`-provenance nodes, layers, flows, and tour steps are
/// selected (ADR-0005 — enriched slots are never re-purchased). A layer's
/// description is its own slot under its own provenance, so an enriched
/// name and an unenriched description select independently. Every slot
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
    // The description is the layer's second slot, selected by its own
    // provenance: a layer whose name is already enriched can still owe a
    // description, and the reverse (ticket 07).
    slots.extend(
        graph
            .layers
            .iter()
            .filter(|l| {
                l.description
                    .as_ref()
                    .is_none_or(|d| d.provenance == Provenance::Structural)
            })
            .map(|l| {
                EnrichmentSlot::LayerDescription(LayerSlot {
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
pub fn fill_slots(
    graph: &mut KnowledgeGraph,
    provider: &(dyn EnrichmentProvider + Sync),
) -> Result<usize> {
    fill_slots_with(graph, provider, &mut |_, _| Ok(()))
}

/// A batch that has just landed: where the run has got to, for a caller that
/// wants to save it, say so, or both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Batch {
    /// Batches answered so far, counting this one. Completion order, not
    /// issue order — batches overlap, so `done` says how many are finished
    /// and never which.
    pub done: usize,
    /// Batches in the whole run, known before the first call.
    pub total: usize,
    /// Slots filled so far across every batch.
    pub filled: usize,
}

/// [`fill_slots`], plus a `checkpoint` invoked after each batch's answers have
/// been applied to `graph`.
///
/// **The checkpoint is the point of this function.** Before it existed, every
/// answer accumulated in memory and nothing was durable until all of them had
/// landed — so a `Ctrl-C` thirty minutes into a thirty-five minute run threw
/// away all sixty-four purchases, and so did one transient failure on batch
/// sixty-three. `fill_slots`'s own doc comment defended that as never saving a
/// partially-purchased run, and the intent was right while the conclusion was
/// not: refusing to ship a *half-enriched map* and discarding *sixty-three
/// successful answers* are different things, and they separate cleanly. The
/// caller checkpoints the annotation store, which is keyed and idempotent, and
/// still writes the map exactly once when the whole run has succeeded.
///
/// Batches run [`ENRICH_CONCURRENCY`] at a time. Answers are addressed by slot
/// key, so which order they come back in cannot change the result — but they
/// are applied on this thread as they arrive, in completion order, because
/// `graph` is `&mut` and the checkpoint has to see each batch's work.
pub fn fill_slots_with(
    graph: &mut KnowledgeGraph,
    provider: &(dyn EnrichmentProvider + Sync),
    checkpoint: &mut dyn FnMut(&KnowledgeGraph, Batch) -> Result<()>,
) -> Result<usize> {
    let Plan { requests, .. } = Plan::of(graph);
    if requests.is_empty() {
        return Ok(0);
    }

    let mut done = 0;
    let mut count = 0;
    let mut failure: Option<anyhow::Error> = None;
    let next = AtomicUsize::new(0);
    // Set by the first failure. In-flight batches are allowed to finish and be
    // applied — they are already paid for — but no new one is started, because
    // the realistic reason a batch fails is a rate limit, and racing more
    // requests at a backend that just refused one spends the reader's
    // allowance to make the refusal worse.
    let stop = AtomicBool::new(false);
    let (send, results) = mpsc::channel::<Result<EnrichmentResponse>>();

    thread::scope(|scope| {
        for _ in 0..ENRICH_CONCURRENCY.min(requests.len()) {
            let send = send.clone();
            let next = &next;
            let stop = &stop;
            let requests = &requests;
            scope.spawn(move || {
                loop {
                    if stop.load(Ordering::Relaxed) {
                        return;
                    }
                    let index = next.fetch_add(1, Ordering::Relaxed);
                    let Some(request) = requests.get(index) else {
                        return;
                    };
                    let outcome = provider.enrich(request);
                    let failed = outcome.is_err();
                    if send.send(outcome).is_err() || failed {
                        return;
                    }
                }
            });
        }
        // The loop below ends when every worker has dropped its sender, so
        // this thread must not hold one of its own.
        drop(send);

        for outcome in results {
            match outcome {
                Ok(response) => {
                    count += apply_answers(graph, &response.answers);
                    done += 1;
                    let batch = Batch {
                        done,
                        total: requests.len(),
                        filled: count,
                    };
                    if let Err(err) = checkpoint(graph, batch) {
                        stop.store(true, Ordering::Relaxed);
                        failure.get_or_insert(err);
                    }
                }
                Err(err) => {
                    stop.store(true, Ordering::Relaxed);
                    failure.get_or_insert(err);
                }
            }
        }
    });

    match failure {
        // Whatever succeeded before the failure has already been applied and
        // checkpointed. The error still propagates, so the caller does not
        // write a map it only half enriched — but it says what survived,
        // because "keep what was paid for" is worth nothing to a reader who
        // assumes the failed run cost them everything and does not re-run.
        Some(err) if count > 0 => {
            Err(err.context(format!("{count} slots were answered and saved before this")))
        }
        Some(err) => Err(err),
        None => Ok(count),
    }
}

/// Fills whichever slots `answers` addresses, returning how many changed.
/// Split out of [`fill_slots_with`] so a batch can be applied the moment it
/// lands rather than at the end of the run. Answers are addressed by slot key,
/// so an answer can only reach the slot it was written for and applying two
/// batches in either order gives the same graph.
fn apply_answers(graph: &mut KnowledgeGraph, answers: &BTreeMap<String, String>) -> usize {
    let mut count = 0;
    for node in &mut graph.nodes {
        if node.provenance != Provenance::Structural {
            continue;
        }
        if let Some(text) = answered(answers, &summary_key(node.id.as_str())) {
            node.summary = text.to_string();
            node.provenance = Provenance::Llm;
            count += 1;
        }
    }
    for layer in &mut graph.layers {
        // The name and the description are separate purchases with separate
        // provenance: an enriched name never blocks a description answer,
        // and neither answer can land in the other's slot.
        if layer.provenance == Provenance::Structural
            && let Some(text) = answered(answers, &layer_key(&layer.id))
        {
            layer.name = text.to_string();
            layer.provenance = Provenance::Llm;
            count += 1;
        }
        if layer
            .description
            .as_ref()
            .is_none_or(|d| d.provenance == Provenance::Structural)
            && let Some(text) = answered(answers, &layer_description_key(&layer.id))
        {
            layer.description = Some(crate::map::LayerDescription {
                text: text.to_string(),
                provenance: Provenance::Llm,
            });
            count += 1;
        }
    }
    for flow in &mut graph.domain_flows {
        if flow.provenance != Provenance::Structural {
            continue;
        }
        if let Some(text) = answered(answers, &flow_key(&flow.id)) {
            flow.name = text.to_string();
            flow.provenance = Provenance::Llm;
            count += 1;
        }
    }
    for step in &mut graph.tour {
        if step.provenance != Provenance::Structural {
            continue;
        }
        if let Some(text) = answered(answers, &tour_key(step.node.as_str())) {
            step.label = text.to_string();
            step.provenance = Provenance::Llm;
            count += 1;
        }
    }
    count
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
    specs.push(claude::SPEC);
    #[cfg(feature = "agent-cli")]
    specs.push(agent_cli::SPEC);
    #[cfg(feature = "test-provider")]
    {
        specs.push("fake:<path>");
        specs.push("fail");
    }
    #[cfg(all(feature = "agent-cli", feature = "test-provider"))]
    specs.push("cli-exec:<path>");
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

/// `serve --ask`'s help text. Build-aware for the same reason as
/// [`provider_help`]: a flag that offers to answer questions must not appear
/// unqualified on a binary that has nothing to answer them with — the sealed
/// build's copy has to say so where a reader looks before running it, not
/// only when the server refuses to start.
pub fn ask_help() -> String {
    format!(
        "Answer questions about the map at POST /api/ask, from a bounded \
         slice of the map alone (ADR-0009). Needs an enrichment backend, \
         selected exactly as `scan --enrich` selects one. {}",
        recognised_sentence()
    )
}

/// The specs that actually reach a model, and so the ones `--model` means
/// anything to. The offline test backends ignore it.
///
/// Feature-gated exactly like [`recognised_specs`], and a function rather
/// than a `const` for that reason: an ungated array would put the literal
/// `cli:claude` into a binary compiled without the CLI backend, and ticket
/// 32's sealed byte probe asserts no such string survives. Built from the
/// same conditions as the selectable list, so the two cannot disagree.
// `vec![]` cannot express this: which elements exist is a compile-time
// question, and collapsing the pushes into a literal is exactly what the
// feature gates forbid.
#[allow(clippy::vec_init_then_push)]
fn model_aware_specs() -> Vec<&'static str> {
    #[allow(unused_mut)]
    let mut specs: Vec<&'static str> = Vec::new();
    #[cfg(feature = "network")]
    specs.push(claude::SPEC);
    #[cfg(feature = "agent-cli")]
    specs.push(agent_cli::SPEC);
    specs
}

/// `--model`'s help text. Build-aware for the same reason as
/// [`provider_help`]: where no backend that reads a model was compiled in,
/// this flag modifies nothing, and a flag whose help describes a thing that
/// is not there is worse than one that says so.
///
/// Derived from [`recognised_specs`] rather than from a feature name. An
/// earlier version keyed on the Claude API backend alone and told an
/// `agent-cli`-without-`network` build that the flag had nothing to modify,
/// while the CLI backend was honouring it.
pub fn model_help() -> String {
    let honoured = model_aware_specs();
    if honoured.is_empty() {
        return format!(
            "Model for an enrichment backend. This build compiled none in, so \
             this flag has nothing to modify. {}",
            recognised_sentence()
        );
    }
    // No backend is named outside the honoured list. Prose mentioning the
    // Claude API's default would appear in a build that has no Claude API
    // backend — the same falsehood this function exists to avoid.
    format!(
        "Model for the enrichment backend. Honoured by: {}. A backend with a \
         default of its own keeps it unless this is given.",
        honoured.join(", ")
    )
}

/// Told a name is wrong, a reader should also be told a right one — not
/// being told was the failure that kept the second credential path invisible.
///
/// Says nothing about the structural map. Since ADR-0009 there are two
/// callers, and only one of them has written a map by the time this is
/// raised; the caller that has says so itself.
fn unknown_provider(spec: &str) -> anyhow::Error {
    anyhow!(
        "unknown enrichment provider {spec:?}. {}",
        recognised_sentence()
    )
}

/// Resolves the provider the CLI will use. An explicit `--provider` spec
/// wins; failing that [`PROVIDER_ENV`]; failing that the build's default.
/// Without a usable provider this fails with a clear message: the structural
/// map has already been written by the time this runs, so `--enrich`
/// degrades cleanly (spec story 14).
pub fn resolve_provider(choice: ProviderChoice<'_>) -> Result<SelectedProvider> {
    let from_env = std::env::var(PROVIDER_ENV).ok();
    let spec = choice.spec.or(from_env.as_deref());
    provider_from_spec(spec, choice.model)
}

/// Selection by spec, separated from where the spec came from — the flag,
/// the environment, or nowhere — so it is unit-testable without touching a
/// process-global. Which surface wins is [`resolve_provider`]'s job; with no
/// spec at all the default depends on the build (see [`default_provider`]).
fn provider_from_spec(spec: Option<&str>, model: Option<&str>) -> Result<SelectedProvider> {
    #[cfg(not(any(feature = "network", feature = "agent-cli")))]
    let _ = model;
    match spec {
        #[cfg(feature = "test-provider")]
        Some(spec) if spec.starts_with("fake:") => Ok(Box::new(test_provider::CannedProvider {
            path: spec["fake:".len()..].into(),
        })),
        #[cfg(feature = "test-provider")]
        Some("fail") => Ok(Box::new(test_provider::FailingProvider)),
        #[cfg(feature = "network")]
        Some(spec) if spec == claude::SPEC => Ok(Box::new(claude::ClaudeProvider::new(model))),
        #[cfg(feature = "agent-cli")]
        Some(spec) if spec == agent_cli::SPEC => Ok(Box::new(agent_cli::CliProvider::new(model))),
        // Seam 3's injection point: a stand-in executable so the spawn can be
        // asserted without running the real CLI. Gated exactly as the `fake:`
        // and `fail` backends are, so no shipped binary can run an arbitrary
        // program.
        #[cfg(all(feature = "agent-cli", feature = "test-provider"))]
        Some(spec) if spec.starts_with("cli-exec:") => Ok(Box::new(
            agent_cli::CliProvider::with_program(&spec["cli-exec:".len()..], model),
        )),
        // A `cli:` spec naming anything else is refused by name rather than
        // falling into the generic message, because the reason is specific
        // and worth saying: this is not a general "run that program" hatch
        // (ADR-0008).
        //
        // Gated on the feature, so a build without the CLI backend carries
        // neither this message nor the `cli:claude` literal in it — and falls
        // through to the ordinary unknown-spec error, which is the truthful
        // answer there. The literal matters: ticket 32's sealed byte probe
        // asserts no `claude` program string survives into that binary.
        #[cfg(feature = "agent-cli")]
        Some(other) if other.starts_with("cli:") => Err(anyhow!(
            "unsupported enrichment provider {other:?}: the only CLI backend \
             is `{}`. CodeAtlas does not run an arbitrary program on request",
            agent_cli::SPEC
        )),
        Some(other) => Err(unknown_provider(other)),
        None => default_provider(model),
    }
}

/// The provider used when neither `--provider` nor [`PROVIDER_ENV`] names
/// one. Every configuration is spelled out because getting one wrong is
/// invisible until someone runs that build:
///
/// - **Test build (`test-provider`)** — no default, whatever else is
///   compiled in. Tests must select a backend explicitly, so none can fall
///   through to one that opens a socket or spawns a process.
/// - **`network`** — the Claude API provider (ADR-0004).
/// - **`agent-cli` without `network`** — the CLI provider (ADR-0008). A
///   binary with exactly one backend defaults to it; anything else would
///   mean shipping a build whose only backend must be named by hand.
/// - **Neither** — the sealed build (ADR-0006): enrichment is unavailable,
///   and the message says so without naming a feature, because which
///   feature is missing depends on the build.
fn default_provider(model: Option<&str>) -> Result<SelectedProvider> {
    #[cfg(feature = "test-provider")]
    {
        let _ = model;
        Err(anyhow!(
            "no enrichment provider is configured: this is a test build, which \
             has no default provider — set {PROVIDER_ENV} or pass --provider"
        ))
    }
    #[cfg(all(feature = "network", not(feature = "test-provider")))]
    {
        Ok(Box::new(claude::ClaudeProvider::new(model)))
    }
    #[cfg(all(
        feature = "agent-cli",
        not(feature = "network"),
        not(feature = "test-provider")
    ))]
    {
        Ok(Box::new(agent_cli::CliProvider::new(model)))
    }
    #[cfg(not(any(feature = "network", feature = "agent-cli", feature = "test-provider")))]
    {
        let _ = model;
        Err(anyhow!(
            "this build has no enrichment backend at all: it was compiled \
             without one (ADR-0006 sealed build)"
        ))
    }
}

#[cfg(test)]
mod tests {
    //! Tests at the provider-trait seam: a fake provider returns canned
    //! typed responses (or errors), and the assertions are about what
    //! reaches the provider and what lands in the graph's slots.

    use std::sync::Mutex;
    use std::time::Duration;

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
        // The literal rather than `agent_cli::SPEC`, which does not exist in
        // a build without the feature — and that build is half of what this
        // asserts.
        assert_eq!(
            specs.contains(&"cli:claude"),
            cfg!(feature = "agent-cli"),
            "the CLI provider is offered exactly when it is compiled in: \
             {specs:?}"
        );
    }

    /// The CLI backend is selectable exactly where it was compiled in.
    ///
    /// This is the live control for `tests/enrich.rs`'s sealed refusal of
    /// `cli:claude`, which on its own would also pass in a build that had
    /// merely lost the ability to select anything at all. ADR-0008 is why
    /// the pair has to exist: `tests/sealed.rs` proves the sealed build
    /// links no networking crate, and a subprocess links none, so that probe
    /// cannot see this backend in either direction.
    ///
    /// Selecting is not spawning — `CliProvider::new` constructs a struct and
    /// runs nothing, so this reaches no program and no network.
    #[test]
    fn the_cli_backend_is_selectable_exactly_where_it_is_compiled_in() {
        assert_eq!(
            provider_from_spec(Some("cli:claude"), None).is_ok(),
            cfg!(feature = "agent-cli"),
            "cli:claude must resolve exactly where the backend exists"
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

    /// The list a reader is offered is exactly the list the build can select.
    ///
    /// Parsed back out of the sentence rather than substring-matched. An
    /// earlier version asked whether the sentence contained `"claude"`, which
    /// was equivalent while the API backend was the only spec with that word
    /// in it — and became wrong the moment `cli:claude` existed. The
    /// `--no-default-features --features agent-cli` configuration caught it
    /// on its first run, which is the whole argument for that configuration
    /// existing.
    #[test]
    fn the_offered_list_is_exactly_what_this_build_can_select() {
        let sentence = recognised_sentence();
        let offered: Vec<&str> = match sentence.split_once("recognises: ") {
            Some((_, list)) => list.trim_end_matches('.').split(", ").collect(),
            None => Vec::new(),
        };
        assert_eq!(
            offered,
            recognised_specs(),
            "the rendered list must be the selectable list: {sentence}"
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
        // `serve --ask` offers to answer questions through the same
        // backends, so it has the same way to be wrong about them.
        assert!(
            ask_help().contains(&sentence),
            "--ask help must render the shared sentence: {}",
            ask_help()
        );
        let error = unknown_provider("nope").to_string();
        assert!(
            error.contains(&sentence),
            "the unknown-spec error must render the shared sentence: {error}"
        );
    }

    /// `--model` means something only to a backend that reaches a model, so
    /// where none was compiled in the flag has to say so rather than
    /// describing itself as though it worked — and where one *was*, it must
    /// not claim otherwise.
    ///
    /// Keyed on both backends, not on `network` alone. The first version of
    /// this asked only about the API backend and told an
    /// `agent-cli`-without-`network` build that the flag had nothing to
    /// modify while the CLI backend was honouring it.
    #[test]
    fn model_help_admits_only_a_genuinely_absent_backend() {
        let none_compiled_in = !cfg!(feature = "network") && !cfg!(feature = "agent-cli");
        assert_eq!(
            model_help().contains("compiled none in"),
            none_compiled_in,
            "--model must admit an absent backend, and only then: {}",
            model_help()
        );
        // Parsed out of the honoured list rather than substring-matched
        // against the whole text: `claude` is a substring of `cli:claude`,
        // and prose about one backend would otherwise read as a claim about
        // the other.
        let help = model_help();
        let honoured: Vec<&str> = match help.split_once("Honoured by: ") {
            Some((_, rest)) => rest
                .split_once(". ")
                .map_or(rest, |(list, _)| list)
                .split(", ")
                .collect(),
            None => Vec::new(),
        };
        assert_eq!(
            honoured,
            model_aware_specs(),
            "--model must name exactly the backends that read it: {help}"
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
            significance: None,
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
            description: Some(crate::map::LayerDescription {
                text: "Files under src/".into(),
                provenance: Provenance::Structural,
            }),
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

    /// [`graph_with_semantics`] with the file nodes made members of `src`,
    /// so the layer's derivation-input hash is over a real member set and a
    /// membership change is expressible.
    fn layered_graph() -> KnowledgeGraph {
        let mut graph = graph_with_semantics();
        graph.nodes[0].layer = Some("src".into());
        graph.nodes[2].layer = Some("src".into());
        graph
    }

    /// Canned answers plus a recording of every request's slot keys.
    struct Fake {
        answers: BTreeMap<String, String>,
        requested: Mutex<Vec<String>>,
    }

    impl EnrichmentProvider for Fake {
        fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
            self.requested
                .lock()
                .unwrap()
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
            requested: Mutex::new(Vec::new()),
        };

        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 1);

        // The already-enriched node was never offered to the provider.
        assert_eq!(
            *fake.requested.lock().unwrap(),
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
            requested: Mutex::new(Vec::new()),
        };

        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 3, "three semantic answers must count as enrichment");

        // The semantic slots were offered alongside the node slots.
        let requested = fake.requested.lock().unwrap();
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
        // label slot, and `src` identifies both of the layer's slots; the
        // prefixed keys keep the namespaces apart. And an answer under a
        // bare node ID (the pre-slot-kind format) matches nothing at all.
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
                    "layer-description:src".to_string(),
                    "Layer prose.".to_string(),
                ),
                (
                    "function:src/a.ts:go".to_string(),
                    "Unprefixed — must land nowhere.".to_string(),
                ),
            ]),
            requested: Mutex::new(Vec::new()),
        };

        fill_slots(&mut graph, &fake).unwrap();

        assert_eq!(graph.nodes[0].summary, "Node prose.");
        assert_eq!(graph.tour[0].label, "Tour prose.");
        assert_eq!(
            graph.nodes[1].summary, "Mechanical summary of go",
            "an unprefixed answer must not land in any slot"
        );
        assert_eq!(graph.nodes[1].provenance, Provenance::Structural);
        // The description answer reached the description alone: the layer's
        // *name* got no `layer-name:` answer, so it stays mechanical even
        // though an answer addressed to the same layer ID existed.
        let description = graph.layers[0].description.as_ref().unwrap();
        assert_eq!(description.text, "Layer prose.");
        assert_eq!(description.provenance, Provenance::Llm);
        assert_eq!(graph.layers[0].name, "src");
        assert_eq!(graph.layers[0].provenance, Provenance::Structural);
        // The flow got no answer: mechanical, structural.
        assert_eq!(graph.domain_flows[0].name, "go");
        assert_eq!(graph.domain_flows[0].provenance, Provenance::Structural);
    }

    #[test]
    fn enriched_semantic_slots_are_not_reoffered() {
        let mut graph = graph_with_semantics();
        graph.layers[0].provenance = Provenance::Llm;
        graph.layers[0].description.as_mut().unwrap().provenance = Provenance::Llm;
        graph.domain_flows[0].provenance = Provenance::Llm;
        graph.tour[0].provenance = Provenance::Llm;

        let fake = Fake {
            answers: BTreeMap::from([
                ("layer-name:src".to_string(), "MUST NOT APPLY".to_string()),
                (
                    "layer-description:src".to_string(),
                    "MUST NOT APPLY".to_string(),
                ),
                (
                    "flow-name:flow:function:src/a.ts:go".to_string(),
                    "MUST NOT APPLY".to_string(),
                ),
                (
                    "tour-label:file:src/a.ts".to_string(),
                    "MUST NOT APPLY".to_string(),
                ),
            ]),
            requested: Mutex::new(Vec::new()),
        };

        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 0);

        // Carried-over semantics are never re-sent (ADR-0005) …
        for key in fake.requested.lock().unwrap().iter() {
            assert!(
                key.starts_with("summary:"),
                "an enriched semantic slot was re-offered: {key}"
            );
        }
        // … and an uninvited answer for them never lands.
        assert_eq!(graph.layers[0].name, "src");
        assert_eq!(
            graph.layers[0].description.as_ref().unwrap().text,
            "Files under src/"
        );
        assert_eq!(graph.domain_flows[0].name, "go");
        assert_eq!(
            graph.tour[0].label,
            "Entry point: src/a.ts — fan-in 0, fan-out 0"
        );
    }

    #[test]
    fn a_description_answer_lands_in_the_description_and_never_in_the_name() {
        let mut graph = graph_with_semantics();
        let fake = Fake {
            answers: BTreeMap::from([(
                "layer-description:src".to_string(),
                "Owns the application's whole runtime.".to_string(),
            )]),
            requested: Mutex::new(Vec::new()),
        };

        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 1, "a description answer must count as enrichment");

        assert!(
            fake.requested
                .lock()
                .unwrap()
                .iter()
                .any(|k| k == "layer-description:src"),
            "the description slot was never offered: {:?}",
            fake.requested.lock().unwrap()
        );

        let layer = &graph.layers[0];
        let description = layer.description.as_ref().expect("description missing");
        assert_eq!(description.text, "Owns the application's whole runtime.");
        assert_eq!(description.provenance, Provenance::Llm);
        // Never the name: it keeps its mechanical text and its own
        // provenance — the two are separate purchases.
        assert_eq!(layer.name, "src");
        assert_eq!(layer.provenance, Provenance::Structural);
    }

    #[test]
    fn name_and_description_are_separate_purchases_that_coexist_on_one_layer() {
        let mut graph = graph_with_semantics();
        let fake = Fake {
            answers: BTreeMap::from([
                ("layer-name:src".to_string(), "Application core".to_string()),
                (
                    "layer-description:src".to_string(),
                    "Everything the app runs, from entry to helpers.".to_string(),
                ),
            ]),
            requested: Mutex::new(Vec::new()),
        };

        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 2, "the name and the description are two purchases");

        let layer = &graph.layers[0];
        assert_eq!(layer.name, "Application core");
        assert_eq!(layer.provenance, Provenance::Llm);
        let description = layer.description.as_ref().unwrap();
        assert_eq!(
            description.text,
            "Everything the app runs, from entry to helpers."
        );
        assert_eq!(description.provenance, Provenance::Llm);
    }

    #[test]
    fn an_enriched_half_of_a_layer_is_not_reoffered_while_the_other_half_is() {
        // Name already purchased, description still mechanical: only the
        // description may be offered, and only its answer may land.
        let mut graph = graph_with_semantics();
        graph.layers[0].name = "Application core".into();
        graph.layers[0].provenance = Provenance::Llm;
        let fake = Fake {
            answers: BTreeMap::from([
                ("layer-name:src".to_string(), "MUST NOT APPLY".to_string()),
                (
                    "layer-description:src".to_string(),
                    "Fresh prose.".to_string(),
                ),
            ]),
            requested: Mutex::new(Vec::new()),
        };
        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 1);
        assert!(
            !fake
                .requested
                .lock()
                .unwrap()
                .iter()
                .any(|k| k == "layer-name:src"),
            "an enriched name was re-offered"
        );
        assert_eq!(graph.layers[0].name, "Application core");
        assert_eq!(
            graph.layers[0].description.as_ref().unwrap().text,
            "Fresh prose."
        );

        // And the reverse: description purchased, name still mechanical.
        let mut graph = graph_with_semantics();
        graph.layers[0].description = Some(crate::map::LayerDescription {
            text: "Bought prose.".into(),
            provenance: Provenance::Llm,
        });
        let fake = Fake {
            answers: BTreeMap::from([
                ("layer-name:src".to_string(), "New name".to_string()),
                (
                    "layer-description:src".to_string(),
                    "MUST NOT APPLY".to_string(),
                ),
            ]),
            requested: Mutex::new(Vec::new()),
        };
        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 1);
        assert!(
            !fake
                .requested
                .lock()
                .unwrap()
                .iter()
                .any(|k| k == "layer-description:src"),
            "an enriched description was re-offered"
        );
        assert_eq!(graph.layers[0].name, "New name");
        assert_eq!(
            graph.layers[0].description.as_ref().unwrap().text,
            "Bought prose.",
            "a carried-over description was overwritten"
        );
    }

    #[test]
    fn the_plan_counts_a_description_slot_for_every_structural_layer() {
        // graph_with_semantics: two structural nodes, one structural layer —
        // which contributes TWO slots, its name and its description — one
        // flow, one tour step. Six slots, so `--dry-run` quotes what a run
        // will really buy.
        let graph = graph_with_semantics();
        let plan = Plan::of(&graph);
        assert_eq!(
            plan.slots(),
            6,
            "the estimate must count the description slot"
        );
        let descriptions: Vec<&LayerSlot> = plan
            .requests
            .iter()
            .flat_map(|r| &r.slots)
            .filter_map(|s| match s {
                EnrichmentSlot::LayerDescription(l) => Some(l),
                _ => None,
            })
            .collect();
        assert_eq!(descriptions.len(), 1, "one structural layer, one slot");
        assert_eq!(descriptions[0].id, "src");
    }

    #[test]
    fn a_purchased_description_is_stored_on_the_hash_the_name_uses_and_reattaches() {
        let root = TempRoot::new("description-carry-over");
        let mut graph = layered_graph();
        graph.layers[0].name = "Application core".into();
        graph.layers[0].provenance = Provenance::Llm;
        graph.layers[0].description = Some(crate::map::LayerDescription {
            text: "Purchased prose about src.".into(),
            provenance: Provenance::Llm,
        });
        save_store(root.path(), &graph, ProviderIdentity::unnamed()).unwrap();

        // Stored under its own section, keyed by layer ID, on the SAME
        // derivation-input hash the name annotation carries — ADR-0005
        // applied to one more slot, never a new carry-over mechanism.
        let written: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(
                root.path()
                    .join(crate::scan::OUTPUT_DIR)
                    .join(ANNOTATIONS_FILE),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            written["layer_descriptions"]["src"]["text"],
            "Purchased prose about src."
        );
        assert_eq!(
            written["layer_descriptions"]["src"]["inputs_hash"],
            written["layers"]["src"]["inputs_hash"],
            "the description must ride the name's derivation hash: {written}"
        );

        // A fresh mechanical graph with unchanged membership gets both back
        // without any provider.
        let mut fresh = layered_graph();
        AnnotationStore::load(root.path()).reattach(root.path(), &mut fresh);
        assert_eq!(fresh.layers[0].name, "Application core");
        assert_eq!(fresh.layers[0].provenance, Provenance::Llm);
        let description = fresh.layers[0].description.as_ref().unwrap();
        assert_eq!(description.text, "Purchased prose about src.");
        assert_eq!(description.provenance, Provenance::Llm);
    }

    #[test]
    fn a_changed_membership_expires_the_description_exactly_as_it_expires_the_name() {
        let root = TempRoot::new("description-expiry");
        let mut graph = layered_graph();
        graph.layers[0].name = "Application core".into();
        graph.layers[0].provenance = Provenance::Llm;
        graph.layers[0].description = Some(crate::map::LayerDescription {
            text: "Purchased prose about src.".into(),
            provenance: Provenance::Llm,
        });
        save_store(root.path(), &graph, ProviderIdentity::unnamed()).unwrap();

        // Same layer, one more member file: the derivation hash moves, and
        // both annotations expire together.
        let mut changed = layered_graph();
        let mut extra = node(
            NodeId::file("src/new.ts"),
            NodeKind::File,
            "new.ts",
            "src/new.ts",
            Provenance::Structural,
        );
        extra.layer = Some("src".into());
        changed.nodes.push(extra);
        AnnotationStore::load(root.path()).reattach(root.path(), &mut changed);

        assert_eq!(changed.layers[0].name, "src", "the stale name must expire");
        assert_eq!(changed.layers[0].provenance, Provenance::Structural);
        let description = changed.layers[0].description.as_ref().unwrap();
        assert_eq!(
            description.text, "Files under src/",
            "stale prose must never describe a changed membership"
        );
        assert_eq!(description.provenance, Provenance::Structural);
    }

    /// The store-version decision this ticket records: `layer_descriptions`
    /// is a purely additive optional section, so [`STORE_VERSION`] stays at
    /// 2 — a bump is a bill (every committed store would be discarded and
    /// every repository charged a re-enrichment), and a store without the
    /// section holds annotations that are still *correct*. What the decision
    /// must buy is asserted here: a store written before this ticket keeps
    /// re-attaching everything it holds.
    #[test]
    fn a_store_written_before_descriptions_existed_keeps_reattaching_what_it_holds() {
        let root = TempRoot::new("pre-description-store");
        fs::create_dir_all(root.path().join("src")).unwrap();
        fs::write(root.path().join("src/a.ts"), b"node body").unwrap();

        let mut graph = layered_graph();
        // Significance puts src/a.ts on the recomputed tour, so the stored
        // tour label has a derivation to match.
        graph.nodes[0].significance = Some(1);
        let hashes = semantic_hashes(&graph);
        let dir = root.path().join(crate::scan::OUTPUT_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(ANNOTATIONS_FILE),
            format!(
                r#"{{
  "version": {STORE_VERSION},
  "annotations": {{
    "file:src/a.ts": {{"content_hash": "{}", "summary": "Old node prose."}}
  }},
  "layers": {{
    "src": {{"inputs_hash": "{}", "text": "Old layer name"}}
  }},
  "flows": {{
    "flow:function:src/a.ts:go": {{"inputs_hash": "{}", "text": "Old flow name"}}
  }},
  "tour": {{
    "file:src/a.ts": {{"inputs_hash": "{}", "text": "Old tour label"}}
  }}
}}"#,
                content_hash(b"node body"),
                hashes.layers["src"],
                hashes.flows["flow:function:src/a.ts:go"],
                hashes.tour["file:src/a.ts"],
            ),
        )
        .unwrap();

        AnnotationStore::load(root.path()).reattach(root.path(), &mut graph);

        assert_eq!(graph.nodes[0].summary, "Old node prose.");
        assert_eq!(graph.nodes[0].provenance, Provenance::Llm);
        assert_eq!(graph.layers[0].name, "Old layer name");
        assert_eq!(graph.layers[0].provenance, Provenance::Llm);
        assert_eq!(graph.domain_flows[0].name, "Old flow name");
        assert_eq!(graph.domain_flows[0].provenance, Provenance::Llm);
        assert_eq!(graph.tour[0].label, "Old tour label");
        assert_eq!(graph.tour[0].provenance, Provenance::Llm);
        // The description the store never heard of stays mechanical —
        // present, structural, free for the next --enrich to buy.
        let description = graph.layers[0].description.as_ref().unwrap();
        assert_eq!(description.text, "Files under src/");
        assert_eq!(description.provenance, Provenance::Structural);
    }

    #[test]
    fn flow_slots_stay_bounded_however_long_the_chain_grows() {
        /// Records every slot offered, answering nothing.
        struct Recording {
            slots: Mutex<Vec<EnrichmentSlot>>,
        }
        impl EnrichmentProvider for Recording {
            fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
                self.slots
                    .lock()
                    .unwrap()
                    .extend(request.slots.iter().cloned());
                Ok(EnrichmentResponse::default())
            }
        }

        let mut graph = graph_with_semantics();
        graph.domain_flows[0].steps = (0..40)
            .map(|i| NodeId::symbol(NodeKind::Function, "src/a.ts", &format!("f{i}")))
            .collect();

        let provider = Recording {
            slots: Mutex::new(Vec::new()),
        };
        fill_slots(&mut graph, &provider).unwrap();

        let slots = provider.slots.lock().unwrap();
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
            request_sizes: Mutex<Vec<usize>>,
        }
        impl EnrichmentProvider for Batching {
            fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
                self.request_sizes.lock().unwrap().push(request.slots.len());
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
            request_sizes: Mutex::new(Vec::new()),
        };
        let count = fill_slots(&mut graph, &provider).unwrap();
        assert_eq!(count, total);

        let sizes = provider.request_sizes.lock().unwrap();
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
            requested: Mutex::new(Vec::new()),
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
                ("layer-description:src".to_string(), " \t ".to_string()),
                (
                    "flow-name:flow:function:src/a.ts:go".to_string(),
                    "  ".to_string(),
                ),
                ("tour-label:file:src/a.ts".to_string(), "\n\t".to_string()),
            ]),
            requested: Mutex::new(Vec::new()),
        };

        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 0, "blank answers must not count as enrichment");

        assert_eq!(graph.layers[0].name, "src");
        assert_eq!(graph.layers[0].provenance, Provenance::Structural);
        // A refused description never replaces the mechanical sentence — the
        // reader must never meet an empty card.
        let description = graph.layers[0].description.as_ref().unwrap();
        assert_eq!(description.text, "Files under src/");
        assert_eq!(description.provenance, Provenance::Structural);
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
            requested: Mutex::new(Vec::new()),
        };

        let count = fill_slots(&mut graph, &fake).unwrap();
        assert_eq!(count, 0);
        assert_eq!(graph.nodes[2].summary, "Mechanical summary of b.ts");
    }

    /// The store's date is computed here rather than taken from a crate
    /// (ADR-0006 admits no new dependency), so the calendar is pinned against
    /// dates checked by hand — the leap day, the year boundary either side of
    /// it, and the 2000 century leap that the naive `%4` rule gets wrong.
    #[test]
    fn the_calendar_survives_leap_years_and_century_boundaries() {
        for (days, expected) in [
            (0, "1970-01-01"),
            (-1, "1969-12-31"),
            (11_016, "2000-02-29"),
            (19_723, "2024-01-01"),
            (19_782, "2024-02-29"),
            (19_783, "2024-03-01"),
            (20_678, "2026-08-13"),
        ] {
            assert_eq!(civil_date(days), expected, "day {days} since the epoch");
        }
    }

    /// The clock is a process-global, so what is assertable here is the shape
    /// the store commits to — four digits, two, two — and that a real run
    /// produces a date from this century rather than the epoch fallback.
    #[test]
    fn todays_date_is_an_iso_calendar_date() {
        let today = today_utc();
        let parts: Vec<&str> = today.split('-').collect();
        assert_eq!(parts.len(), 3, "not an ISO date: {today}");
        assert_eq!(
            (parts[0].len(), parts[1].len(), parts[2].len()),
            (4, 2, 2),
            "not zero-padded: {today}"
        );
        assert!(
            today.as_str() > "2025-01-01",
            "the clock was not read: {today}"
        );
    }

    /// Prose committed into a repository (ADR-0007) enters code review, and a
    /// reviewer is entitled to know what wrote it. The backend answers rather
    /// than the selection code, because the model is often the backend's own
    /// default and nothing else knows it.
    #[test]
    fn the_written_store_records_the_provider_the_model_and_the_date() {
        struct Named;
        impl EnrichmentProvider for Named {
            fn enrich(&self, _request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
                Ok(EnrichmentResponse::default())
            }
            fn identity(&self) -> ProviderIdentity {
                ProviderIdentity {
                    provider: "a-backend".to_string(),
                    model: Some("a-model".to_string()),
                }
            }
        }

        let root = TempRoot::new("store-provenance");
        // The clock is read once by `save_store` and bracketed here, rather
        // than read a second time at assert time: a run straddling UTC
        // midnight would compare two different days and fail for no reason.
        let opened = today_utc();
        save_store(root.path(), &graph(), Named.identity()).unwrap();
        let closed = today_utc();

        let written: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(
                root.path()
                    .join(crate::scan::OUTPUT_DIR)
                    .join(ANNOTATIONS_FILE),
            )
            .unwrap(),
        )
        .unwrap();
        assert_eq!(written["produced_by"]["provider"], "a-backend");
        assert_eq!(written["produced_by"]["model"], "a-model");
        let date = written["produced_by"]["date"].as_str().unwrap();
        assert!(
            date == opened || date == closed,
            "the store's date is not the date of the run that wrote it: \
             {date} is neither {opened} nor {closed}"
        );
        assert_eq!(
            written["version"], STORE_VERSION,
            "the provenance fields are additive; a bump would discard every \
             store already written"
        );
    }

    /// A backend that names no model must not have one invented for it: the
    /// CLI provider leaves an unasked-for model to the subscription's own
    /// default and genuinely does not know which one answered.
    #[test]
    fn a_backend_with_no_model_records_none_rather_than_a_guess() {
        struct Modelless;
        impl EnrichmentProvider for Modelless {
            fn enrich(&self, _request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
                Ok(EnrichmentResponse::default())
            }
            fn identity(&self) -> ProviderIdentity {
                ProviderIdentity {
                    provider: "a-backend".to_string(),
                    model: None,
                }
            }
        }

        let root = TempRoot::new("store-no-model");
        save_store(root.path(), &graph(), Modelless.identity()).unwrap();

        let written: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(
                root.path()
                    .join(crate::scan::OUTPUT_DIR)
                    .join(ANNOTATIONS_FILE),
            )
            .unwrap(),
        )
        .unwrap();
        // The key, by name, in the object that would carry it. Searching the
        // whole serialized store for the substring `model` would say the same
        // thing today and would go on to fail on any future field — or any
        // annotation prose — that happened to contain those five letters.
        let produced_by = written["produced_by"]
            .as_object()
            .unwrap_or_else(|| panic!("the store recorded nothing about its producer: {written}"));
        assert!(
            !produced_by.contains_key("model"),
            "an absent model must be absent, not null: {written}"
        );
        assert!(
            produced_by.contains_key("provider"),
            "the object checked for an absent `model` must be the one that \
             would carry it: {written}"
        );
    }

    /// A store written before ADR-0007's fields must still load and still
    /// carry over. Asserted against a store that really lacks them rather
    /// than against serde's defaults, because what is at stake is a file
    /// already sitting in somebody's repository.
    #[test]
    fn a_store_without_the_provenance_fields_still_loads() {
        let root = TempRoot::new("store-old-shape");
        let dir = root.path().join(crate::scan::OUTPUT_DIR);
        fs::create_dir_all(&dir).unwrap();
        fs::write(root.path().join("src-a.ts"), b"contents").unwrap();
        let hash = content_hash(b"contents");
        fs::write(
            dir.join(ANNOTATIONS_FILE),
            format!(
                r#"{{"version": {STORE_VERSION},
                    "annotations": {{
                      "file:src-a.ts": {{
                        "content_hash": "{hash}",
                        "summary": "Prose from before ticket 30."
                      }}
                    }}}}"#
            ),
        )
        .unwrap();

        let mut graph = graph();
        graph.nodes[0].id = NodeId::file("src-a.ts");
        graph.nodes[0].path = "src-a.ts".into();
        AnnotationStore::load(root.path()).reattach(root.path(), &mut graph);

        assert_eq!(graph.nodes[0].summary, "Prose from before ticket 30.");
        assert_eq!(graph.nodes[0].provenance, Provenance::Llm);
    }

    /// A scratch directory for the tests that write a store. Hand-rolled for
    /// the reason `agent_cli::ScratchDir` records: `tempfile` is a
    /// dev-dependency of the integration tests, and the unit tests here have
    /// never needed one.
    struct TempRoot(std::path::PathBuf);

    impl TempRoot {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "codeatlas-{name}-{}-{:?}",
                std::process::id(),
                std::thread::current().id()
            ));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
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

    /// A graph of `files` file nodes and nothing else, so the batch count is
    /// `files / BATCH_SIZE` and the arithmetic in these tests is legible.
    fn files_graph(files: usize) -> KnowledgeGraph {
        let mut graph = graph();
        graph.layers.clear();
        graph.domain_flows.clear();
        graph.tour.clear();
        graph.nodes = (0..files)
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
        graph
    }

    /// Answers every slot it is offered, counting calls. `after` batches in,
    /// it starts failing — which is how a rate limit arrives partway through a
    /// long run, and the case in which throwing away what was already bought
    /// costs the reader real money.
    struct FailsAfter {
        after: usize,
        calls: Mutex<usize>,
    }

    impl FailsAfter {
        fn new(after: usize) -> Self {
            Self {
                after,
                calls: Mutex::new(0),
            }
        }
        fn calls(&self) -> usize {
            *self.calls.lock().unwrap()
        }
    }

    impl EnrichmentProvider for FailsAfter {
        fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
            let mine = {
                let mut calls = self.calls.lock().unwrap();
                *calls += 1;
                *calls
            };
            if mine > self.after {
                return Err(anyhow!("injected failure on call {mine}"));
            }
            Ok(EnrichmentResponse {
                answers: request
                    .slots
                    .iter()
                    .map(|s| (s.key(), format!("Prose for {}", s.key())))
                    .collect(),
            })
        }
    }

    #[test]
    fn a_failed_batch_does_not_discard_the_batches_that_succeeded() {
        // The defect this is about: every answer used to accumulate in memory
        // and nothing was durable until all of them landed, so one failure on
        // the last batch of a sixty-four batch run threw away sixty-three
        // purchases. The provider still fails and the error still propagates —
        // what changed is that the work already bought survives it.
        let mut graph = files_graph(4 * BATCH_SIZE);
        let provider = FailsAfter::new(2);
        let mut checkpointed: Vec<usize> = Vec::new();

        let mut reported: Vec<Batch> = Vec::new();
        let err = fill_slots_with(&mut graph, &provider, &mut |graph, batch| {
            checkpointed.push(
                graph
                    .nodes
                    .iter()
                    .filter(|n| n.provenance == Provenance::Llm)
                    .count(),
            );
            reported.push(batch);
            Ok(())
        })
        .unwrap_err();

        assert!(format!("{err:#}").contains("injected failure"));
        // The reader is told what survived. Without this they assume a failed
        // run cost them everything, do not re-run, and the checkpoint buys
        // them nothing.
        assert!(
            format!("{err:#}").contains(&format!("{} slots were answered", 2 * BATCH_SIZE)),
            "the failure does not say what was saved: {err:#}"
        );
        // Two batches answered, so two checkpoints, and the last of them saw
        // every node those two batches covered. Asserting only "a checkpoint
        // happened" would pass over one that ran before any answer was
        // applied and saved nothing.
        assert_eq!(checkpointed.len(), 2, "checkpoints: {checkpointed:?}");
        assert_eq!(
            checkpointed.last().copied(),
            Some(2 * BATCH_SIZE),
            "the last checkpoint must hold both answered batches: {checkpointed:?}"
        );
        // Progress *advances*, and knows the whole size of the run from the
        // first report. A single report, or one that counted only itself,
        // satisfies "something was reported" and tells the reader nothing.
        assert_eq!(
            reported.iter().map(|b| b.done).collect::<Vec<_>>(),
            vec![1, 2],
            "progress did not advance: {reported:?}"
        );
        assert!(
            reported.iter().all(|b| b.total == 4),
            "the total must be known before the run ends: {reported:?}"
        );
    }

    #[test]
    fn a_resumed_run_asks_only_for_what_it_does_not_already_have() {
        // What the checkpoint buys. Half the graph is already enriched — the
        // state a re-run finds after `AnnotationStore::reattach` has restored
        // the previous run's work — so only the remainder may be bought again.
        let half = 2 * BATCH_SIZE;
        let mut graph = files_graph(4 * BATCH_SIZE);
        for node in graph.nodes.iter_mut().take(half) {
            node.provenance = Provenance::Llm;
        }

        let provider = FailsAfter::new(usize::MAX);
        let count = fill_slots(&mut graph, &provider).unwrap();

        assert_eq!(count, half, "only the unenriched half may be filled");
        assert_eq!(
            provider.calls(),
            half / BATCH_SIZE,
            "a resumed run re-bought work it already had"
        );
    }

    #[test]
    fn batches_run_concurrently() {
        // Asserted on calls *in flight*, which is the only observation that
        // can fail: elapsed time is flaky, and the finished map is identical
        // either way — a result assertion passes over a sequential run and
        // proves nothing about the thing this test is named for.
        //
        // Each call parks until `ENRICH_CONCURRENCY` of them are parked
        // together, so a sequential implementation cannot reach the barrier
        // and times out rather than passing.
        #[derive(Default)]
        struct Flight {
            now: usize,
            /// Monotonic. Waiting on this rather than on `now` is the
            /// difference between a test and a deadlock: the last arrival
            /// decrements `now` on its way out, so a waiter re-checking it
            /// sees the count fall back below the target and sleeps forever.
            /// A high-water mark never goes backwards. The first draft of
            /// this test did wait on `now`, and hung for three ten-second
            /// timeouts while still reporting a pass.
            peak: usize,
        }
        struct Barrier {
            flight: Mutex<Flight>,
            reached: std::sync::Condvar,
        }
        impl EnrichmentProvider for Barrier {
            fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
                {
                    let mut flight = self.flight.lock().unwrap();
                    flight.now += 1;
                    flight.peak = flight.peak.max(flight.now);
                    self.reached.notify_all();
                    while flight.peak < ENRICH_CONCURRENCY {
                        let (guard, timeout) = self
                            .reached
                            .wait_timeout(flight, Duration::from_secs(5))
                            .unwrap();
                        flight = guard;
                        if timeout.timed_out() {
                            break;
                        }
                    }
                    flight.now -= 1;
                }
                Ok(EnrichmentResponse {
                    answers: request
                        .slots
                        .iter()
                        .map(|s| (s.key(), format!("Prose for {}", s.key())))
                        .collect(),
                })
            }
        }

        let batches = 2 * ENRICH_CONCURRENCY;
        let mut graph = files_graph(batches * BATCH_SIZE);
        let provider = Barrier {
            flight: Mutex::new(Flight::default()),
            reached: std::sync::Condvar::new(),
        };

        let count = fill_slots(&mut graph, &provider).unwrap();

        assert_eq!(count, batches * BATCH_SIZE);
        assert_eq!(
            provider.flight.lock().unwrap().peak,
            ENRICH_CONCURRENCY,
            "batches did not overlap: the run is still sequential"
        );
    }

    #[test]
    fn the_estimate_predicts_exactly_what_the_run_spends() {
        // The criterion the whole estimate rests on. A figure quoted to a
        // reader before they agree to spend has to be the figure they are
        // charged, and the only way to keep it so is for both to come out of
        // `Plan::of` — an estimator of its own would agree today and be a
        // confident lie the first time batching changed.
        let mut graph = files_graph(3 * BATCH_SIZE + 4);
        let plan = Plan::of(&graph);
        let predicted = plan.calls();
        let slots = plan.slots();

        let provider = FailsAfter::new(usize::MAX);
        let filled = fill_slots(&mut graph, &provider).unwrap();

        assert_eq!(
            provider.calls(),
            predicted,
            "the run made a call the estimate did not predict"
        );
        assert_eq!(
            filled, slots,
            "the estimate counted slots the run did not fill"
        );
    }

    #[test]
    #[cfg(any(feature = "network", feature = "agent-cli"))]
    fn the_estimate_measures_the_prompt_a_backend_will_really_send() {
        // Not a proxy for the prompt — the prompt. Every backend composes its
        // request through `prompt::for_enrichment`, and so does this, so a
        // change to the system prompt or the schema moves the estimate with
        // it rather than leaving it quietly stale.
        let graph = files_graph(BATCH_SIZE);
        let plan = Plan::of(&graph);
        assert_eq!(plan.calls(), 1);
        let real = prompt::for_enrichment(&plan.requests[0]);
        let expected =
            real.system_prompt.len() + real.user_message.len() + real.schema.to_string().len();
        assert_eq!(plan.prompt_chars(), expected);
        // And it is a real prompt, not an empty one being compared to itself.
        assert!(
            plan.prompt_chars() > 1000,
            "a batch of {BATCH_SIZE} slots cannot be this small: {}",
            plan.prompt_chars()
        );
    }

    #[test]
    #[cfg(any(feature = "network", feature = "agent-cli"))]
    fn the_estimate_is_a_range_and_never_a_price() {
        // Three things this project has been bitten by, kept out by assertion
        // rather than by intention: a single figure that reads as measured, a
        // rate that goes stale, and one total that launders the guessed half
        // of the estimate into the computed half.
        let graph = files_graph(4 * BATCH_SIZE);
        let described = Plan::of(&graph).describe();

        // *Both* figures, counted rather than searched for. The first version
        // of this asserted `contains('–')`, which the output range satisfies
        // on its own — collapsing the prompt figure to a single number left it
        // green. An estimate is two ranges or it is not this estimate.
        assert_eq!(
            described.matches('–').count(),
            2,
            "both figures must be ranges: {described}"
        );
        assert!(
            described.contains(&format!("{} slots", 4 * BATCH_SIZE))
                && described.contains("in 4 calls"),
            "the two exactly-known numbers must be stated: {described}"
        );
        for money in ['$', '£', '€'] {
            assert!(
                !described.contains(money),
                "a price cannot survive a rate change: {described}"
            );
        }
        for word in ["cost", "USD", "price"] {
            assert!(
                !described.to_lowercase().contains(&word.to_lowercase()),
                "{word:?} promises something this cannot know: {described}"
            );
        }
    }

    #[test]
    fn concurrency_does_not_change_the_result() {
        // Answers are addressed by slot key, so completion order cannot reach
        // the graph — but "should hold by construction" is how the other
        // eleven defects in this project got in.
        let build = || {
            let mut graph = files_graph(3 * BATCH_SIZE + 7);
            let filled = fill_slots(&mut graph, &FailsAfter::new(usize::MAX)).unwrap();
            (filled, serde_json::to_string(&graph).unwrap())
        };
        let (first_count, first) = build();
        let (second_count, second) = build();
        assert_eq!(first_count, second_count);
        assert_eq!(first, second, "two runs of the same input disagreed");
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
    /// `summary:<node-id>`, `layer-name:<layer-id>`,
    /// `layer-description:<layer-id>`, `flow-name:<flow-id>`, or
    /// `tour-label:<node-id>` (see [`EnrichmentSlot::key`]). A key with
    /// no prefix — or the wrong prefix — never matches any slot.
    pub struct CannedProvider {
        pub path: PathBuf,
    }

    /// The reserved key holding a canned answer to any question, and the
    /// one holding the node IDs it claims to rest on (whitespace-separated).
    /// Prefixed like every other key in the file, so they cannot collide
    /// with a slot address.
    const ASK_ANSWER: &str = "ask:answer";
    const ASK_CITATIONS: &str = "ask:citations";
    /// The reserved keys scripting what this backend claims the exchange
    /// spent (ticket 09). Both present reads as a measurement; anything less
    /// is a backend that reports nothing — which is also the default, so
    /// every canned file written before usage existed scripts the absent
    /// case by construction.
    const ASK_INPUT_TOKENS: &str = "ask:input_tokens";
    const ASK_OUTPUT_TOKENS: &str = "ask:output_tokens";

    impl CannedProvider {
        fn canned(&self) -> Result<BTreeMap<String, String>> {
            let raw = fs::read_to_string(&self.path)
                .with_context(|| format!("cannot read canned responses {:?}", self.path))?;
            Ok(serde_json::from_str(&raw)?)
        }
    }

    impl EnrichmentProvider for CannedProvider {
        fn enrich(&self, _request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
            Ok(EnrichmentResponse {
                answers: self.canned()?,
            })
        }

        /// Answers from [`ASK_ANSWER`], citing [`ASK_CITATIONS`] verbatim —
        /// including IDs that are not in the map, so seam 4 can watch an
        /// invented citation being dropped.
        fn ask(&self, _question: &ask::Question) -> Result<ask::Answer> {
            let canned = self.canned()?;
            let text = canned.get(ASK_ANSWER).ok_or_else(|| {
                anyhow!("the canned responses {:?} hold no {ASK_ANSWER}", self.path)
            })?;
            let usage = match (canned.get(ASK_INPUT_TOKENS), canned.get(ASK_OUTPUT_TOKENS)) {
                (Some(input), Some(output)) => Some(ask::Usage {
                    input_tokens: input.parse().with_context(|| {
                        format!("{ASK_INPUT_TOKENS} in {:?} is not a count", self.path)
                    })?,
                    output_tokens: output.parse().with_context(|| {
                        format!("{ASK_OUTPUT_TOKENS} in {:?} is not a count", self.path)
                    })?,
                }),
                _ => None,
            };
            Ok(ask::Answer {
                text: text.clone(),
                citations: canned
                    .get(ASK_CITATIONS)
                    .map(|ids| ids.split_whitespace().map(str::to_string).collect())
                    .unwrap_or_default(),
                usage,
            })
        }

        /// Names the backend without its `fake:<path>` argument: the path is
        /// a temp directory that differs every run, and a store recording it
        /// would differ every run with it.
        fn identity(&self) -> ProviderIdentity {
            ProviderIdentity {
                provider: "fake".to_string(),
                model: None,
            }
        }
    }

    /// Errors on every call — failure injection for spec story 14, and for
    /// ADR-0009's requirement that a failed question leaves the server up.
    pub struct FailingProvider;

    impl EnrichmentProvider for FailingProvider {
        fn enrich(&self, _request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
            Err(anyhow!("injected provider failure"))
        }

        fn ask(&self, _question: &ask::Question) -> Result<ask::Answer> {
            Err(anyhow!("injected provider failure"))
        }
    }
}
