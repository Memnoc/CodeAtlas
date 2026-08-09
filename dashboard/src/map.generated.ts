/* Generated from contract/map.schema.json by json-schema-to-typescript. DO NOT EDIT — run `npm run generate` in dashboard/. */

/**
 * Whether a node's descriptive fields were produced mechanically or by LLM
 * enrichment (ADR-0005).
 */
export type Provenance = "structural" | "llm";
/**
 * Typed node ID — the map's identity primitive. Format:
 * `<kind-prefix>:<path>` for files, `<kind-prefix>:<path>:<symbol>` for
 * symbols, e.g. `file:src/main.ts` or `function:src/main.ts:main`. All ID
 * minting goes through the constructors here; nothing else formats IDs.
 */
export type NodeId = string;
export type EdgeKind = "contains" | "imports" | "exports" | "calls";
export type NodeKind = "file" | "function" | "class";

export interface KnowledgeGraph {
  /**
   * Mechanically projected domain flows. Optional in the contract (older
   * maps omit it); always emitted by the CLI.
   */
  domain_flows?: DomainFlow[];
  edges: Edge[];
  /**
   * Directory-derived layers every file node is assigned to. Optional in
   * the contract (older maps omit it); always emitted by the CLI.
   */
  layers?: Layer[];
  nodes: Node[];
  project: Project;
  /**
   * The guided tour: an ordered walk over the file nodes. Optional in the
   * contract (older maps omit it); always emitted by the CLI.
   */
  tour?: TourStep[];
  /**
   * Semver version of the map contract this file conforms to.
   */
  version: string;
}
/**
 * A call chain rooted at an entry point — a function nothing else calls.
 * The chain and its domain are structural facts; the name is the enrichable
 * slot — mechanically it renders the chain, enrichment may relabel it.
 */
export interface DomainFlow {
  /**
   * The domain this flow belongs to: the top-level directory of the root
   * function's file, or `root` for files at the repository root.
   */
  domain: string;
  /**
   * Stable flow ID derived from the root function's node ID, e.g.
   * `flow:function:src/main.ts:main`.
   */
  id: string;
  /**
   * Mechanical or enriched display name; provenance says which.
   */
  name: string;
  provenance: Provenance;
  /**
   * Function node IDs along the chain, root first, in deterministic
   * depth-first call order.
   */
  steps: NodeId[];
}
export interface Edge {
  kind: EdgeKind;
  source: NodeId;
  target: NodeId;
  /**
   * Fixed weight determined by `kind`; see [`EdgeKind::weight`].
   */
  weight: number;
}
/**
 * A horizontal grouping of files. Membership is structural (each file node's
 * `layer` field); the name is the enrichable slot — mechanically it is the
 * deriving directory, enrichment may relabel it.
 */
export interface Layer {
  /**
   * Stable layer ID: the top-level directory that derived it, or `root`
   * for files at the repository root.
   */
  id: string;
  /**
   * Mechanical or enriched display name; provenance says which.
   */
  name: string;
  provenance: Provenance;
}
export interface Node {
  /**
   * Typed node ID — the map's identity primitive. Format:
   * `<kind-prefix>:<path>` for files, `<kind-prefix>:<path>:<symbol>` for
   * symbols, e.g. `file:src/main.ts` or `function:src/main.ts:main`. All ID
   * minting goes through the constructors here; nothing else formats IDs.
   */
  id: string;
  kind: NodeKind;
  /**
   * ID of the layer this file node belongs to; absent on symbol nodes,
   * which inherit their file's layer through containment.
   */
  layer?: string | null;
  name: string;
  /**
   * Repo-relative path with forward slashes.
   */
  path: string;
  provenance: Provenance;
  range?: Range | null;
  /**
   * Mechanical or enriched description; provenance says which.
   */
  summary: string;
}
/**
 * 1-based inclusive line range within the node's file.
 */
export interface Range {
  end_line: number;
  start_line: number;
}
export interface Project {
  name: string;
}
/**
 * One stop on the guided tour. The step's position comes from topology
 * scoring and is structural; the label is the enrichable slot — mechanical
 * by default, enrichment may narrate it.
 */
export interface TourStep {
  /**
   * Mechanical or enriched narration; provenance says which.
   */
  label: string;
  /**
   * Typed node ID — the map's identity primitive. Format:
   * `<kind-prefix>:<path>` for files, `<kind-prefix>:<path>:<symbol>` for
   * symbols, e.g. `file:src/main.ts` or `function:src/main.ts:main`. All ID
   * minting goes through the constructors here; nothing else formats IDs.
   */
  node: string;
  provenance: Provenance;
}
