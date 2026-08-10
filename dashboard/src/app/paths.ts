// Shortest path between two nodes, for the Path control.
//
// Edges are followed in both directions. The question the control answers is
// "how are these two related", and a reader tracing a relationship does not
// care which end declared it — an importer reaches its import, and the
// imported file is reached by its importer, by the same edge.
import type { KnowledgeGraph, Node as MapNode } from "../index.js";

/** Nodes from `fromId` to `toId` inclusive, or `null` when no path exists.
 * Breadth-first, so the result is a shortest path; among equally short ones
 * it is the first found in map edge order, which is stable for a given map. */
export function shortestPath(
  map: KnowledgeGraph,
  fromId: string,
  toId: string,
): MapNode[] | null {
  const byId = new Map(map.nodes.map((n) => [n.id, n]));
  if (!byId.has(fromId) || !byId.has(toId)) {
    return null;
  }
  if (fromId === toId) {
    const node = byId.get(fromId);
    return node === undefined ? null : [node];
  }

  const neighbours = new Map<string, string[]>();
  const link = (a: string, b: string) => {
    const bucket = neighbours.get(a);
    if (bucket === undefined) {
      neighbours.set(a, [b]);
    } else {
      bucket.push(b);
    }
  };
  for (const edge of map.edges) {
    link(edge.source, edge.target);
    link(edge.target, edge.source);
  }

  const cameFrom = new Map<string, string>([[fromId, fromId]]);
  let frontier = [fromId];
  while (frontier.length > 0) {
    const next: string[] = [];
    for (const id of frontier) {
      for (const neighbour of neighbours.get(id) ?? []) {
        if (cameFrom.has(neighbour)) {
          continue;
        }
        cameFrom.set(neighbour, id);
        if (neighbour === toId) {
          return walkBack(byId, cameFrom, fromId, toId);
        }
        next.push(neighbour);
      }
    }
    frontier = next;
  }
  return null;
}

function walkBack(
  byId: ReadonlyMap<string, MapNode>,
  cameFrom: ReadonlyMap<string, string>,
  fromId: string,
  toId: string,
): MapNode[] {
  const ids = [toId];
  let cursor = toId;
  while (cursor !== fromId) {
    const previous = cameFrom.get(cursor);
    if (previous === undefined) {
      break;
    }
    ids.push(previous);
    cursor = previous;
  }
  ids.reverse();
  return ids.flatMap((id) => {
    const node = byId.get(id);
    return node === undefined ? [] : [node];
  });
}
