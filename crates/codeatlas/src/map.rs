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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct Node {
    /// Typed ID, e.g. `file:src/main.ts` or `function:src/main.ts:main`.
    pub id: String,
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

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    File,
    Function,
    Class,
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
    pub source: String,
    pub target: String,
    pub kind: EdgeKind,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Contains,
}
