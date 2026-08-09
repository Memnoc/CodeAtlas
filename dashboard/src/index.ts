// The dashboard's map types come exclusively from the generated contract
// types (ADR-0003). No map shape is ever declared by hand on this side of
// the language border.
export type {
  DomainFlow,
  Edge,
  EdgeKind,
  KnowledgeGraph,
  Layer,
  Node,
  NodeId,
  NodeKind,
  Project,
  Provenance,
  Range,
  TourStep,
} from "./map.generated.js";
