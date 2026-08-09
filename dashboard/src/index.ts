// The dashboard's map types come exclusively from the generated contract
// types (ADR-0003). No map shape is ever declared by hand on this side of
// the language border.
export type {
  Edge,
  EdgeKind,
  KnowledgeGraph,
  Node,
  NodeId,
  NodeKind,
  Project,
  Provenance,
  Range,
} from "./map.generated.js";
