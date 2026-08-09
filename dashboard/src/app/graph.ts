// Pure projection from the map contract to React Flow's node/edge shapes.
// Everything here is deterministic: layer bands laid out left to right,
// nodes gridded inside their layer's group container.
import { Position, type Edge as FlowEdge, type Node as FlowNode } from "@xyflow/react";
import type { KnowledgeGraph, Node as MapNode } from "../index.js";

export type EntityData = { node: MapNode };
export type LayerData = { layerId: string; label: string };

export type EntityFlowNode = FlowNode<EntityData, "entity">;
export type LayerFlowNode = FlowNode<LayerData, "layerGroup">;
export type AppFlowNode = EntityFlowNode | LayerFlowNode;

export const NODE_WIDTH = 190;
export const NODE_HEIGHT = 54;
const GRID_COLS = 3;
const GAP = 14;
const HEADER = 42;
const GROUP_GAP = 48;

/** The layer a node belongs to: file nodes carry it directly; symbol nodes
 * inherit their file's layer through containment; maps without layers fall
 * back to a single implicit `root` layer. */
export function layerOfNode(
  node: MapNode,
  fileLayers: ReadonlyMap<string, string>,
): string {
  if (node.kind === "file") {
    return node.layer ?? "root";
  }
  return fileLayers.get(node.path) ?? "root";
}

export function toFlow(map: KnowledgeGraph): {
  nodes: AppFlowNode[];
  edges: FlowEdge[];
} {
  const fileLayers = new Map<string, string>();
  for (const node of map.nodes) {
    if (node.kind === "file") {
      fileLayers.set(node.path, node.layer ?? "root");
    }
  }

  // Declared layers first, in contract order; any layer only discovered via
  // node assignment (older maps, absent `layers`) is appended.
  const layerNames = new Map<string, string>();
  for (const layer of map.layers ?? []) {
    layerNames.set(layer.id, layer.name);
  }
  const membership = new Map<string, MapNode[]>();
  for (const layerId of layerNames.keys()) {
    membership.set(layerId, []);
  }
  for (const node of map.nodes) {
    const layerId = layerOfNode(node, fileLayers);
    if (!layerNames.has(layerId)) {
      layerNames.set(layerId, layerId);
      membership.set(layerId, []);
    }
    membership.get(layerId)?.push(node);
  }

  const nodes: AppFlowNode[] = [];
  let x = 0;
  for (const [layerId, members] of membership) {
    if (members.length === 0) {
      continue;
    }
    const cols = Math.min(GRID_COLS, members.length);
    const rows = Math.ceil(members.length / cols);
    const width = GAP + cols * (NODE_WIDTH + GAP);
    const height = HEADER + rows * (NODE_HEIGHT + GAP) + GAP;
    const groupId = `layer:${layerId}`;

    nodes.push({
      id: groupId,
      type: "layerGroup",
      position: { x, y: 0 },
      width,
      height,
      draggable: false,
      selectable: false,
      data: { layerId, label: layerNames.get(layerId) ?? layerId },
    });

    members.forEach((node, i) => {
      nodes.push({
        id: node.id,
        type: "entity",
        parentId: groupId,
        extent: "parent",
        position: {
          x: GAP + (i % cols) * (NODE_WIDTH + GAP),
          y: HEADER + Math.floor(i / cols) * (NODE_HEIGHT + GAP),
        },
        width: NODE_WIDTH,
        height: NODE_HEIGHT,
        // Static handle geometry lets React Flow draw edges without DOM
        // measurement (the same mechanism it documents for SSR).
        handles: [
          {
            type: "target",
            position: Position.Top,
            x: NODE_WIDTH / 2,
            y: 0,
            width: 6,
            height: 6,
          },
          {
            type: "source",
            position: Position.Bottom,
            x: NODE_WIDTH / 2,
            y: NODE_HEIGHT,
            width: 6,
            height: 6,
          },
        ],
        data: { node },
      });
    });

    x += width + GROUP_GAP;
  }

  const known = new Set(map.nodes.map((n) => n.id));
  const edges: FlowEdge[] = map.edges
    .filter((e) => known.has(e.source) && known.has(e.target))
    .map((e) => ({
      id: `${e.source}--${e.kind}--${e.target}`,
      source: e.source,
      target: e.target,
      label: e.kind,
    }));

  return { nodes, edges };
}

/** Case-insensitive substring search over node names and paths. */
export function searchNodes(map: KnowledgeGraph, query: string): MapNode[] {
  const q = query.trim().toLowerCase();
  if (q === "") {
    return [];
  }
  return map.nodes.filter(
    (n) => n.name.toLowerCase().includes(q) || n.path.toLowerCase().includes(q),
  );
}
