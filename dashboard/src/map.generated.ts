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
   * The guided tour: a bounded, ordered walk over the file nodes that
   * carry the architecture — a newcomer-sized reading order, not one
   * step per file, so its length does not grow with the repository. The
   * contract sets no length limit; each producer picks its own (this
   * CLI's selection and ordering rules are documented on
   * `semantics::build_tour`). Optional in the contract (older maps omit
   * it); always emitted by the CLI, though a repository whose files
   * neither import nor call one another has nothing to walk and gets an
   * empty tour.
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
 * `layer` field); the name and the description are the enrichable slots —
 * mechanically the name is the deriving directory and the description a
 * directory sentence; enrichment may relabel either, each under its own
 * provenance.
 */
export interface Layer {
  /**
   * What the layer's files *are*, in prose. The scan publishes the
   * mechanical sentence; enrichment may replace it (ticket 07). Optional
   * in the contract (maps before 0.5.0 omit it); always emitted by the
   * CLI.
   */
  description?: LayerDescription | null;
  /**
   * Stable layer ID: the top-level directory that derived it, or `root`
   * for files at the repository root.
   */
  id: string;
  /**
   * Mechanical or enriched display name; provenance says which.
   */
  name: string;
  /**
   * Whether a node's descriptive fields were produced mechanically or by LLM
   * enrichment (ADR-0005).
   */
  provenance: "structural" | "llm";
}
/**
 * A layer's prose description with provenance of its own — separate from
 * the name's, because the two are separate purchases: a layer with a
 * mechanical name and an enriched description (or the reverse) must be
 * badged truthfully per part, and one provenance covering both would lie
 * about half the card.
 */
export interface LayerDescription {
  provenance: Provenance;
  /**
   * Mechanical or enriched prose; `provenance` says which.
   */
  text: string;
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
   * How much this file matters: import fan-in + import fan-out + 1 if the
   * file hosts an entry point (ADR-0010). Absent on symbol nodes — it is a
   * file-level number — and absent from maps written before contract
   * 0.4.0, which is the only reason it is optional: a producer that
   * publishes it publishes it for every file, zeros included. A consumer
   * ranking files reads this number rather than deriving one of its own,
   * so the tour, the default drill view and the rankings cannot disagree
   * about the same repository.
   */
  significance?: number | null;
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
