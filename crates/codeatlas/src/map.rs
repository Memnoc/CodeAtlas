//! The map contract types. These structs are the single source of truth for
//! the CodeAtlas map format (ADR-0003): the JSON Schema and the dashboard's
//! TypeScript types are generated from them, never written by hand.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Semver version of the map contract carried by every emitted map.
/// 0.2.0: added optional `layers`, `domain_flows`, `tour`, and `Node.layer`.
/// 0.3.0: tightened validation — semver pattern on `version`, kind-prefix
/// pattern on node IDs, 1-based minimum on ranges — and gave the schema a
/// versioned `$id`. Tightening is breaking for producers that emitted
/// ill-formed values, which standard 0.x semver permits on a minor bump;
/// the only known producer (this binary) already conformed.
/// 0.3.1: documentation only — `tour` now states that it is a bounded,
/// curated walk rather than one step per file. No shape change; maps and
/// producers valid under 0.3.0 stay valid.
/// 0.4.0: added optional `Node.significance` (ADR-0010). A new optional
/// field is a backward-compatible extension: maps written under 0.3.1 stay
/// valid, and a consumer that ignores the field reads them as before.
pub const MAP_CONTRACT_VERSION: &str = "0.4.0";

/// The published contract schema: the schemars-derived schema for
/// [`KnowledgeGraph`] plus a stable, versioned `$id`. This is the single
/// generation point — the `schema` subcommand prints it and the contract
/// tests walk it, so the committed artifact can never diverge from what
/// tests saw.
pub fn contract_schema() -> serde_json::Value {
    let schema = schemars::schema_for!(KnowledgeGraph);
    let mut value = schema.to_value();
    value.as_object_mut().unwrap().insert(
        "$id".to_string(),
        serde_json::Value::String(format!("urn:codeatlas:map-contract:{MAP_CONTRACT_VERSION}")),
    );
    value
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeGraph {
    /// Semver version of the map contract this file conforms to.
    #[schemars(pattern(r"^\d+\.\d+\.\d+$"))]
    pub version: String,
    pub project: Project,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    /// Directory-derived layers every file node is assigned to. Optional in
    /// the contract (older maps omit it); always emitted by the CLI.
    #[serde(default)]
    pub layers: Vec<Layer>,
    /// Mechanically projected domain flows. Optional in the contract (older
    /// maps omit it); always emitted by the CLI.
    #[serde(default)]
    pub domain_flows: Vec<DomainFlow>,
    /// The guided tour: a bounded, ordered walk over the file nodes that
    /// carry the architecture — a newcomer-sized reading order, not one
    /// step per file, so its length does not grow with the repository. The
    /// contract sets no length limit; each producer picks its own (this
    /// CLI's selection and ordering rules are documented on
    /// `semantics::build_tour`). Optional in the contract (older maps omit
    /// it); always emitted by the CLI, though a repository whose files
    /// neither import nor call one another has nothing to walk and gets an
    /// empty tour.
    #[serde(default)]
    pub tour: Vec<TourStep>,
}

/// One stop on the guided tour. The step's position comes from topology
/// scoring and is structural; the label is the enrichable slot — mechanical
/// by default, enrichment may narrate it.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct TourStep {
    /// The node this step visits.
    pub node: NodeId,
    /// Mechanical or enriched narration; provenance says which.
    pub label: String,
    pub provenance: Provenance,
}

/// A call chain rooted at an entry point — a function nothing else calls.
/// The chain and its domain are structural facts; the name is the enrichable
/// slot — mechanically it renders the chain, enrichment may relabel it.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct DomainFlow {
    /// Stable flow ID derived from the root function's node ID, e.g.
    /// `flow:function:src/main.ts:main`.
    pub id: String,
    /// Mechanical or enriched display name; provenance says which.
    pub name: String,
    /// The domain this flow belongs to: the top-level directory of the root
    /// function's file, or `root` for files at the repository root.
    pub domain: String,
    /// Function node IDs along the chain, root first, in deterministic
    /// depth-first call order.
    pub steps: Vec<NodeId>,
    pub provenance: Provenance,
}

/// A horizontal grouping of files. Membership is structural (each file node's
/// `layer` field); the name is the enrichable slot — mechanically it is the
/// deriving directory, enrichment may relabel it.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Layer {
    /// Stable layer ID: the top-level directory that derived it, or `root`
    /// for files at the repository root.
    pub id: String,
    /// Mechanical or enriched display name; provenance says which.
    pub name: String,
    pub provenance: Provenance,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    pub name: String,
}

/// Typed node ID — the map's identity primitive. Format:
/// `<kind-prefix>:<path>` for files, `<kind-prefix>:<path>:<symbol>` for
/// symbols, e.g. `file:src/main.ts` or `function:src/main.ts:main`. All ID
/// minting goes through the constructors here; nothing else formats IDs.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[serde(transparent)]
// The pattern spells out every `NodeKind::id_prefix` — the schema is the
// public contract, so the closed prefix set is stated here, not inferred.
pub struct NodeId(#[schemars(pattern(r"^(file|function|class):"))] String);

impl NodeId {
    pub fn file(path: &str) -> Self {
        Self(format!("{}:{path}", NodeKind::File.id_prefix()))
    }

    pub fn symbol(kind: NodeKind, path: &str, name: &str) -> Self {
        Self(format!("{}:{path}:{name}", kind.id_prefix()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Node {
    /// Typed ID, e.g. `file:src/main.ts` or `function:src/main.ts:main`.
    pub id: NodeId,
    pub kind: NodeKind,
    pub name: String,
    /// Repo-relative path with forward slashes.
    pub path: String,
    /// Mechanical or enriched description; provenance says which.
    pub summary: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Range>,
    /// ID of the layer this file node belongs to; absent on symbol nodes,
    /// which inherit their file's layer through containment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<String>,
    /// How much this file matters: import fan-in + import fan-out + 1 if the
    /// file hosts an entry point (ADR-0010). Absent on symbol nodes — it is a
    /// file-level number — and absent from maps written before contract
    /// 0.4.0, which is the only reason it is optional: a producer that
    /// publishes it publishes it for every file, zeros included. A consumer
    /// ranking files reads this number rather than deriving one of its own,
    /// so the tour, the default drill view and the rankings cannot disagree
    /// about the same repository.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub significance: Option<u32>,
    pub provenance: Provenance,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    File,
    Function,
    Class,
}

impl NodeKind {
    /// The ID prefix for this kind — the single definition of the
    /// kind → prefix mapping used by [`NodeId`].
    pub const fn id_prefix(self) -> &'static str {
        match self {
            NodeKind::File => "file",
            NodeKind::Function => "function",
            NodeKind::Class => "class",
        }
    }
}

/// 1-based inclusive line range within the node's file.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Range {
    #[schemars(range(min = 1))]
    pub start_line: u32,
    #[schemars(range(min = 1))]
    pub end_line: u32,
}

/// Whether a node's descriptive fields were produced mechanically or by LLM
/// enrichment (ADR-0005).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Provenance {
    Structural,
    Llm,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Edge {
    pub source: NodeId,
    pub target: NodeId,
    pub kind: EdgeKind,
    /// Fixed weight determined by `kind`; see [`EdgeKind::weight`].
    pub weight: f64,
}

impl Edge {
    /// The only way edges are built: the weight always comes from the kind.
    pub fn new(source: NodeId, target: NodeId, kind: EdgeKind) -> Self {
        Self {
            source,
            target,
            kind,
            weight: kind.weight(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
    Imports,
    Exports,
    Calls,
}

impl EdgeKind {
    /// The fixed weight every edge of this kind carries — part of the map
    /// contract, defined here on the schema types so producers and consumers
    /// share one table. Higher weight = tighter structural coupling:
    /// containment is definitional, a call is a direct runtime dependency,
    /// an import couples whole files, an export merely publishes a symbol.
    pub const fn weight(self) -> f64 {
        match self {
            EdgeKind::Contains => 1.0,
            EdgeKind::Calls => 0.9,
            EdgeKind::Imports => 0.7,
            EdgeKind::Exports => 0.5,
        }
    }
}
