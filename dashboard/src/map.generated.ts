/* Generated from contract/map.schema.json by json-schema-to-typescript. DO NOT EDIT — run `npm run generate` in dashboard/. */

export type EdgeKind = "contains" | "imports" | "exports" | "calls";
/**
 * Typed node ID — the map's identity primitive. Format:
 * `<kind-prefix>:<path>` for files, `<kind-prefix>:<path>:<symbol>` for
 * symbols, e.g. `file:src/main.ts` or `function:src/main.ts:main`. All ID
 * minting goes through the constructors here; nothing else formats IDs.
 */
export type NodeId = string;
export type NodeKind = "file" | "function" | "class";
/**
 * Whether a node's descriptive fields were produced mechanically or by LLM
 * enrichment (ADR-0005).
 */
export type Provenance = "structural" | "llm";

export interface KnowledgeGraph {
  edges: Edge[];
  nodes: Node[];
  project: Project;
  /**
   * Semver version of the map contract this file conforms to.
   */
  version: string;
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
export interface Node {
  /**
   * Typed node ID — the map's identity primitive. Format:
   * `<kind-prefix>:<path>` for files, `<kind-prefix>:<path>:<symbol>` for
   * symbols, e.g. `file:src/main.ts` or `function:src/main.ts:main`. All ID
   * minting goes through the constructors here; nothing else formats IDs.
   */
  id: string;
  kind: NodeKind;
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
