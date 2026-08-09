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
//! The binary resolves its provider from the `CODEATLAS_ENRICH_PROVIDER`
//! env var; an explicit spec always wins over the default. Recognized
//! specs and per-build defaults:
//!
//! - `claude` — the real Claude API provider ([`claude`], `network`
//!   builds only). This is also the **default** in a shipped `network`
//!   build when the env var is unset.
//! - `fake:<path>` / `fail` — offline test backends (below).
//! - Unset in a **test build** (`test-provider` feature, enabled by the
//!   self dev-dependency in `Cargo.toml`) — an error: tests must pick a
//!   backend explicitly, so none can fall through to a provider that
//!   opens sockets (the no-network-in-tests rule).
//! - Unset in a **sealed build** (`--no-default-features`, ADR-0006) — a
//!   clear "enrichment is not available in this build" error; no
//!   networking code exists to select.
//!
//! The test backends, compiled in only for test builds:
//!
//! - `fake:<path>` — canned typed responses from a JSON file mapping
//!   node ID → summary
//! - `fail` — a provider that errors on every call (failure injection,
//!   spec story 14)
//!
//! Neither backend can open a socket; no test performs network I/O.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::map::{KnowledgeGraph, NodeId, NodeKind, Provenance};

#[cfg(feature = "network")]
pub mod claude;

/// The env var the CLI resolves its enrichment provider from.
pub const PROVIDER_ENV: &str = "CODEATLAS_ENRICH_PROVIDER";

/// The most summary slots a single provider request may carry (spec:
/// bounded prompts — the model never sees the whole serialized graph, and a
/// request's size cannot grow with the repository). 25 slots keep the
/// prompt at a few KB and the structured response comfortably inside one
/// completion; larger repos simply make more requests.
pub const BATCH_SIZE: usize = 25;

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

/// A typed enrichment request: the slots to fill, nothing else. Never the
/// whole serialized graph (spec: bounded prompts).
#[derive(Debug)]
pub struct EnrichmentRequest {
    pub project: String,
    pub slots: Vec<SummarySlot>,
}

/// A typed enrichment response: prose per node ID. Slots absent from the
/// map keep their mechanical summary.
#[derive(Debug, Default)]
pub struct EnrichmentResponse {
    pub summaries: BTreeMap<String, String>,
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

/// Bumped whenever the store format (including the hash definition)
/// changes; a store with another version is ignored, which merely costs a
/// re-enrichment.
const STORE_VERSION: u32 = 1;

/// The carry-over store (ADR-0005): enrichment prose keyed by node identity
/// (the node ID, which embeds the repo-relative path) plus a hash of the
/// node's file content. Annotations re-attach for free while the content is
/// unchanged and expire the moment it changes — stale prose never describes
/// new code.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct AnnotationStore {
    version: u32,
    annotations: BTreeMap<String, Annotation>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Annotation {
    content_hash: String,
    summary: String,
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
    /// provenance) without any provider call. Everything else stays
    /// structural — an edited file's nodes revert and will be re-selected
    /// by the next `--enrich`.
    pub fn reattach(&self, root: &Path, graph: &mut KnowledgeGraph) {
        if self.annotations.is_empty() {
            return;
        }
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
}

/// Rebuilds the store from the enriched graph — every `llm`-provenance node
/// becomes one annotation keyed by its current file hash — and writes it
/// deterministically (sorted keys, pretty, trailing newline). Rebuilding
/// from the graph is self-pruning: annotations for deleted nodes or
/// no-longer-matching content simply cease to exist.
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
    let store = AnnotationStore {
        version: STORE_VERSION,
        annotations,
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
    /// No structural-provenance node needed a summary (empty map, or every
    /// summary already enriched/carried over): no provider was resolved and
    /// no request was made.
    NothingToEnrich,
    /// The provider ran; this many nodes were enriched.
    Enriched(usize),
}

/// The `--enrich` step, run after the structural map is already saved:
/// resolve a provider, fill the summary slots of structural-provenance
/// nodes, and re-save the map. When nothing needs enrichment this succeeds
/// without even resolving a provider (a repo with everything carried over
/// must not demand credentials). Any error leaves the saved structural map
/// untouched.
pub fn run(root: &Path, graph: &mut KnowledgeGraph, model: Option<&str>) -> Result<Outcome> {
    if collect_slots(graph).is_empty() {
        return Ok(Outcome::NothingToEnrich);
    }
    let provider = resolve_provider(model)?;
    let count = fill_slots(graph, provider.as_ref())?;
    save_store(root, graph)?;
    crate::scan::save(root, graph)?;
    Ok(Outcome::Enriched(count))
}

/// The summary slots the provider would be asked to fill: only
/// `structural`-provenance nodes are selected (ADR-0005 — enriched nodes
/// are never re-purchased).
fn collect_slots(graph: &KnowledgeGraph) -> Vec<SummarySlot> {
    graph
        .nodes
        .iter()
        .filter(|n| n.provenance == Provenance::Structural)
        .map(|n| SummarySlot {
            node: n.id.clone(),
            kind: n.kind,
            name: n.name.clone(),
            path: n.path.clone(),
            mechanical_summary: n.summary.clone(),
        })
        .collect()
}

/// Fills summary slots through the provider in batches of at most
/// [`BATCH_SIZE`] slots per request (spec: bounded prompts). Only answered,
/// non-blank slots change, flipping to `llm` provenance; a blank or
/// whitespace-only answer is treated as unanswered, so the mechanical
/// fallback is never replaced by a hole. Unanswered slots keep their
/// mechanical summary. Any batch error fails the whole step: the caller
/// never saves a partially-purchased run.
pub fn fill_slots(graph: &mut KnowledgeGraph, provider: &dyn EnrichmentProvider) -> Result<usize> {
    let slots = collect_slots(graph);
    if slots.is_empty() {
        return Ok(0);
    }
    let mut summaries: BTreeMap<String, String> = BTreeMap::new();
    for batch in slots.chunks(BATCH_SIZE) {
        let request = EnrichmentRequest {
            project: graph.project.name.clone(),
            slots: batch.to_vec(),
        };
        summaries.extend(provider.enrich(&request)?.summaries);
    }

    let mut count = 0;
    for node in &mut graph.nodes {
        if node.provenance != Provenance::Structural {
            continue;
        }
        if let Some(summary) = summaries.get(node.id.as_str()) {
            if summary.trim().is_empty() {
                continue;
            }
            node.summary = summary.clone();
            node.provenance = Provenance::Llm;
            count += 1;
        }
    }
    Ok(count)
}

/// Resolves the provider the CLI will use, from [`PROVIDER_ENV`]. `model`
/// is forwarded to the Claude provider (`--model`); the offline test
/// backends ignore it. Without a usable provider this fails with a clear
/// message: the structural map has already been written by the time this
/// runs, so `--enrich` degrades cleanly (spec story 14).
pub fn resolve_provider(model: Option<&str>) -> Result<Box<dyn EnrichmentProvider>> {
    provider_from_spec(std::env::var(PROVIDER_ENV).ok().as_deref(), model)
}

/// Provider selection separated from the env read so the precedence rules
/// are unit-testable. An explicit [`PROVIDER_ENV`] spec always wins; with
/// no spec the default depends on the build (see [`default_provider`]).
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
        Some(other) => Err(anyhow!("unknown enrichment provider {other:?}")),
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

    use super::*;
    use crate::map::{Node, NodeKind, Project};
    use std::cell::RefCell;

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

    /// Canned answers plus a recording of every request's slot IDs.
    struct Fake {
        answers: BTreeMap<String, String>,
        requested: RefCell<Vec<String>>,
    }

    impl EnrichmentProvider for Fake {
        fn enrich(&self, request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
            self.requested
                .borrow_mut()
                .extend(request.slots.iter().map(|s| s.node.as_str().to_string()));
            Ok(EnrichmentResponse {
                summaries: self.answers.clone(),
            })
        }
    }

    #[test]
    fn only_structural_slots_are_offered_and_answers_land_in_them() {
        let mut graph = graph();
        let fake = Fake {
            answers: BTreeMap::from([(
                "function:src/a.ts:go".to_string(),
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
                "file:src/a.ts".to_string(),
                "function:src/a.ts:go".to_string()
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
                    summaries: request
                        .slots
                        .iter()
                        .map(|s| (s.node.as_str().to_string(), format!("Prose for {}", s.name)))
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
                ("file:src/a.ts".to_string(), "".to_string()),
                ("function:src/a.ts:go".to_string(), " \n\t ".to_string()),
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
    fn an_answer_for_an_unoffered_node_is_ignored() {
        let mut graph = graph();
        let fake = Fake {
            answers: BTreeMap::from([(
                "file:src/b.ts".to_string(),
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

    /// Canned typed responses from a JSON file: `{ "<node-id>": "<summary>" }`.
    pub struct CannedProvider {
        pub path: PathBuf,
    }

    impl EnrichmentProvider for CannedProvider {
        fn enrich(&self, _request: &EnrichmentRequest) -> Result<EnrichmentResponse> {
            let raw = fs::read_to_string(&self.path)
                .with_context(|| format!("cannot read canned responses {:?}", self.path))?;
            let summaries: BTreeMap<String, String> = serde_json::from_str(&raw)?;
            Ok(EnrichmentResponse { summaries })
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
