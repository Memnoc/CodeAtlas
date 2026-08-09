//! The map contract types. These structs are the single source of truth for
//! the CodeAtlas map format (ADR-0003): the JSON Schema and the dashboard's
//! TypeScript types are generated from them, never written by hand.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Semver version of the map contract carried by every emitted map.
pub const MAP_CONTRACT_VERSION: &str = "0.1.0";

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct KnowledgeGraph {
    /// Semver version of the map contract this file conforms to.
    pub version: String,
    pub project: Project,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Project {
    pub name: String,
}

/// Typed node ID — the map's identity primitive. Format:
/// `<kind-prefix>:<path>` for files, `<kind-prefix>:<path>:<symbol>` for
/// symbols, e.g. `file:src/main.ts` or `function:src/main.ts:main`. All ID
/// minting goes through the constructors here; nothing else formats IDs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct NodeId(String);

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
    pub start_line: u32,
    pub end_line: u32,
}

/// Whether a node's descriptive fields were produced mechanically or by LLM
/// enrichment (ADR-0005).
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
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
