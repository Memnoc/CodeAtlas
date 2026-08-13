// Pure projection from the map contract to React Flow's node/edge shapes.
//
// Two views, and the canvas draws exactly one at a time. The overview draws
// one card per region, which is what makes the canvas readable: a repository
// has hundreds of files and a handful of regions, and a picture of hundreds
// of things is not a picture. Drilling into a region draws that region's
// files and nothing else.
import {
  type Edge as FlowEdge,
  type Node as FlowNode,
} from "@xyflow/react";
import type { KnowledgeGraph, Node as MapNode } from "../index.js";
import { type Anchor, fanOut, handlesOf } from "./anchors.js";
import type { Region, RegionLink } from "./regions.js";
import { bySignificance } from "./significance.js";

export type EntityData = {
  node: MapNode;
  /** The points edges land on, which the card renders as its handles. Absent
   * only on a card no edge touches. */
  anchors?: readonly Anchor[];
  /** Caption under the name, when the label preset asks for one. */
  caption?: string;
  /** Diff-overlay highlight, set only while the overlay toggle is on. */
  highlight?: "changed" | "affected";
  /** Set while this node is a step of the path the Path control found. */
  onPath?: boolean;
  /** Set on the files the selected one actually touches. */
  neighbour?: boolean;
  /** Set on everything a selection pushed into the background. */
  dim?: boolean;
};
export type RegionData = {
  region: Region;
  /** Which of the six accents this region is drawn in. */
  colorIndex: number;
  /** The points edges land on, which the card renders as its handles. Absent
   * only on a card no edge touches. */
  anchors?: readonly Anchor[];
  /** Caption under the description, when the label preset asks for one. */
  caption?: string;
};

export type EntityFlowNode = FlowNode<EntityData, "entity">;
export type RegionFlowNode = FlowNode<RegionData, "region">;
export type AppFlowNode = EntityFlowNode | RegionFlowNode;

export const NODE_WIDTH = 200;
export const NODE_HEIGHT = 58;
export const REGION_WIDTH = 226;
export const REGION_HEIGHT = 112;

const REGION_GAP_X = 46;
const REGION_GAP_Y = 92;
const REGION_COLS = 4;
const GAP = 18;
/** Room between two layers of files. Generous: the vertical gap is what makes
 * a layered drawing readable, because it is the only place an edge has to
 * travel in. */
const LAYER_GAP = GAP * 4;
/** Files with no relationship inside the region are parked in a plain block,
 * this many to a row. They have no edges to route, so all they owe the layout
 * is to stay out of its way. */
const STANDALONE_COLS = 8;

/** How many file cards a drill view draws before the reader asks for the
 * rest.
 *
 * Forty is the count the V1 reference material demonstrated readable —
 * `docs/intake/2026-08-12-codeatlas-v1-next.md`, written 2026-08-12, records
 * the reference dashboard drawing "~40 nodes, not 598". It is a named
 * constant, not a promise and not a setting: whether it should become
 * adjustable is an open question the V2 spec's Further Notes answers "not
 * now". */
export const DRILL_DEFAULT_CARDS = 40;

/** The default revealed set: nothing. Shared rather than built per call, and
 * read-only, so the default can never become a place state accumulates. */
const NOTHING_REVEALED: ReadonlySet<string> = new Set<string>();

/** How many of a region's files the default drill view holds back — what the
 * reveal affordance names, and zero for a region it already draws whole.
 * Stated here, beside the selection rule it inverts, so the control and the
 * canvas cannot come to disagree about the same region. */
export function hiddenByDefault(region: Region): number {
  return Math.max(0, region.files.length - DRILL_DEFAULT_CARDS);
}

/** A dependency, reduced to its endpoints — all the layout ever needs. */
type Link = { source: string; target: string };

/** A caption field that is absent when there is none: under
 * `exactOptionalPropertyTypes` an explicit `undefined` is not the same as an
 * omitted optional field. */
function caption(text: string | null | undefined): { caption?: string } {
  return text === null || text === undefined ? {} : { caption: text };
}

/** Appends to the list under `key`, starting one if this is the first. */
function push(index: Map<string, string[]>, key: string, value: string): void {
  const list = index.get(key);
  if (list === undefined) {
    index.set(key, [value]);
  } else {
    list.push(value);
  }
}

/** The last pass of either view: every edge is given its own landing point on
 * both cards it touches, and every card the handles those points need.
 *
 * Last, because the ranking that keeps a fan from crossing itself reads where
 * the other end of each edge was drawn — so the cards have to be placed first.
 * The rule itself, and what happens past a card's capacity, is
 * [`fanOut`](./anchors.ts). */
function fanned(
  nodes: readonly AppFlowNode[],
  edges: readonly FlowEdge[],
): { nodes: AppFlowNode[]; edges: FlowEdge[] } {
  const fan = fanOut(
    nodes.map((n) => ({ id: n.id, x: n.position.x, width: n.width ?? 0 })),
    edges,
  );
  return {
    nodes: nodes.map((node): AppFlowNode => {
      const anchors = fan.anchors.get(node.id) ?? [];
      const handles = handlesOf(anchors, node.height ?? 0);
      // The two arms are the same code twice on purpose: `AppFlowNode` is a
      // union discriminated on `type`, and rebuilding a member of it without
      // narrowing first widens `data` to neither half's shape. The anchors go
      // into `data` as well as into `handles` because React Flow hands a node
      // component its `data` and nothing else — the card has to be able to
      // render the very points the projection placed.
      return node.type === "region"
        ? { ...node, handles, data: { ...node.data, anchors } }
        : { ...node, handles, data: { ...node.data, anchors } };
    }),
    edges: edges.map((edge) => {
      const ends = fan.ends.get(edge.id);
      return ends === undefined
        ? edge
        : { ...edge, sourceHandle: ends.source, targetHandle: ends.target };
    }),
  };
}

/** The overview: one card per region, banded by how far down the dependency
 * order each sits, so the things everything leans on settle at the bottom
 * and the entry points rise to the top. A grid alone would be stable but
 * would say nothing; this says something and is still deterministic. */
export function regionFlow(
  regions: readonly Region[],
  links: readonly RegionLink[],
  captionOf?: (region: Region) => string | null,
): { nodes: AppFlowNode[]; edges: FlowEdge[] } {
  const depths = dependencyDepths(
    regions.map((r) => r.id),
    links,
  );
  const bands = new Map<number, Region[]>();
  for (const region of regions) {
    const depth = depths.get(region.id) ?? 0;
    const band = bands.get(depth);
    if (band === undefined) {
      bands.set(depth, [region]);
    } else {
      band.push(region);
    }
  }

  const colorIndex = new Map(regions.map((r, i) => [r.id, i]));
  const nodes: AppFlowNode[] = [];
  // Bands stack by the rows they actually occupy, not by depth number: a
  // band of nine regions wraps to three rows, and using depth as the y
  // multiplier would draw the next band straight through it.
  let row0 = 0;
  for (const depth of [...bands.keys()].sort((a, b) => a - b)) {
    const band = bands.get(depth) ?? [];
    band.forEach((region, i) => {
      const row = row0 + Math.floor(i / REGION_COLS);
      nodes.push({
        id: regionNodeId(region.id),
        type: "region",
        position: {
          x: (i % REGION_COLS) * (REGION_WIDTH + REGION_GAP_X),
          y: row * (REGION_HEIGHT + REGION_GAP_Y),
        },
        width: REGION_WIDTH,
        height: REGION_HEIGHT,
        data: {
          region,
          colorIndex: colorIndex.get(region.id) ?? 0,
          ...caption(captionOf?.(region)),
        },
      });
    });
    row0 += Math.max(1, Math.ceil(band.length / REGION_COLS));
  }

  const drawn = new Set(nodes.map((n) => n.id));
  const edges: FlowEdge[] = links
    .map((link) => ({
      id: `${link.source}--${link.target}`,
      source: regionNodeId(link.source),
      target: regionNodeId(link.target),
      label: link.label,
      type: "default",
    }))
    .filter((e) => drawn.has(e.source) && drawn.has(e.target));

  return fanned(nodes, edges);
}

/** Canvas node ID for a region. Region IDs come from directory names, which
 * can collide with nothing else here, but keeping the namespaces apart means
 * a region and a file can never answer to the same canvas ID. */
export function regionNodeId(id: string): string {
  return `region:${id}`;
}

/** The links that close a cycle, found by depth-first search: a link back to
 * something still on the stack. Import graphs have cycles — two modules in a
 * package referring to each other is ordinary — and "one deeper than whatever
 * points at me" never settles inside one, so the cycle has to be cut
 * somewhere. Cutting the link that closes it puts the break at the point a
 * reader would already call the loop-closing edge. */
function backLinks(
  ids: readonly string[],
  links: readonly Link[],
): Set<Link> {
  const out = new Map<string, Link[]>();
  for (const link of links) {
    const from = out.get(link.source);
    if (from === undefined) {
      out.set(link.source, [link]);
    } else {
      from.push(link);
    }
  }

  // Explicit stack rather than recursion: a deep import chain in a large
  // repository is exactly the input that would blow a call stack.
  const state = new Map<string, "open" | "closed">();
  const back = new Set<Link>();
  for (const root of ids) {
    if (state.has(root)) {
      continue;
    }
    state.set(root, "open");
    const stack = [{ id: root, next: 0 }];
    while (stack.length > 0) {
      const top = stack[stack.length - 1];
      if (top === undefined) {
        break;
      }
      const leaving = out.get(top.id) ?? [];
      const link = leaving[top.next];
      if (link === undefined) {
        state.set(top.id, "closed");
        stack.pop();
        continue;
      }
      top.next += 1;
      const seen = state.get(link.target);
      if (seen === "open") {
        back.add(link);
      } else if (seen === undefined) {
        state.set(link.target, "open");
        stack.push({ id: link.target, next: 0 });
      }
    }
  }
  return back;
}

/** How far each id sits down the dependency order: zero for one nothing
 * points at, one more than the deepest thing pointing at it otherwise. Links
 * that close a cycle sit the layering out, so the relaxation terminates on
 * the shape of the graph rather than on its iteration bound. */
function dependencyDepths(
  ids: readonly string[],
  links: readonly Link[],
): Map<string, number> {
  const back = backLinks(ids, links);
  const forward = links.filter(
    (link) => !back.has(link) && link.source !== link.target,
  );
  const depths = new Map(ids.map((id) => [id, 0]));
  for (let pass = 0; pass < ids.length; pass += 1) {
    let moved = false;
    for (const link of forward) {
      const from = depths.get(link.source);
      const to = depths.get(link.target);
      if (from === undefined || to === undefined) {
        continue;
      }
      if (to < from + 1) {
        depths.set(link.target, from + 1);
        moved = true;
      }
    }
    if (!moved) {
      break;
    }
  }
  return depths;
}

/** The ids grouped into layers by dependency depth, shallowest first. Empty
 * depths are closed up, so the result is one band per drawn row. */
function layersOf(ids: readonly string[], links: readonly Link[]): string[][] {
  const depths = dependencyDepths(ids, links);
  const banded = new Map<number, string[]>();
  for (const id of ids) {
    const depth = depths.get(id) ?? 0;
    const band = banded.get(depth);
    if (band === undefined) {
      banded.set(depth, [id]);
    } else {
      band.push(id);
    }
  }
  return [...banded.keys()]
    .sort((a, b) => a - b)
    .flatMap((depth) => {
      const band = banded.get(depth);
      return band === undefined ? [] : [band];
    });
}

/** Reorders each layer, in place, to pull crossings out of the drawing.
 *
 * Two passes of the standard treatment. Barycentre first — put each file
 * beside the average position of the files it connects to, sweeping down and
 * then back up so both ends of every link get a say. Then transpose: swap
 * neighbouring pairs for as long as swapping removes crossings, which is what
 * takes off the last third that averaging cannot see.
 *
 * Deterministic throughout: a fixed number of sweeps, ties broken by the
 * order already there. The same map always draws the same picture. */
function orderLayers(layers: string[][], links: readonly Link[]): void {
  const layerOf = new Map(
    layers.flatMap((layer, depth) => layer.map((id) => [id, depth] as const)),
  );
  const before = new Map<string, string[]>();
  const after = new Map<string, string[]>();
  const spanning: Link[] = [];
  for (const link of links) {
    const from = layerOf.get(link.source);
    const to = layerOf.get(link.target);
    // A link inside one layer is drawn, but it cannot inform an ordering
    // that only knows about the layer above and the layer below.
    if (from === undefined || to === undefined || from === to) {
      continue;
    }
    spanning.push(link);
    push(before, link.target, link.source);
    push(after, link.source, link.target);
  }

  const index = new Map<string, number>();
  const reindex = () => {
    for (const layer of layers) {
      layer.forEach((id, i) => index.set(id, i));
    }
  };
  reindex();

  const barycentre = (id: string, neighbours: Map<string, string[]>) => {
    const list = neighbours.get(id);
    if (list === undefined || list.length === 0) {
      // Nothing to be beside: stay put rather than drift to the left edge.
      return index.get(id) ?? 0;
    }
    return list.reduce((sum, n) => sum + (index.get(n) ?? 0), 0) / list.length;
  };
  const sortBy = (layer: string[], neighbours: Map<string, string[]>) => {
    const key = new Map(layer.map((id) => [id, barycentre(id, neighbours)]));
    layer.sort((a, b) => (key.get(a) ?? 0) - (key.get(b) ?? 0));
    reindex();
  };

  for (let sweep = 0; sweep < BARYCENTRE_SWEEPS; sweep += 1) {
    for (let depth = 1; depth < layers.length; depth += 1) {
      sortBy(layers[depth] ?? [], before);
    }
    for (let depth = layers.length - 2; depth >= 0; depth -= 1) {
      sortBy(layers[depth] ?? [], after);
    }
  }

  // Crossings between two neighbouring layers: a pair of links crosses when
  // their endpoints are in opposite orders on the two rows.
  const between = (upper: string[], lower: string[]) => {
    const up = new Map(upper.map((id, i) => [id, i]));
    const down = new Map(lower.map((id, i) => [id, i]));
    const pairs: [number, number][] = [];
    for (const link of spanning) {
      const from = up.get(link.source);
      const to = down.get(link.target);
      if (from !== undefined && to !== undefined) {
        pairs.push([from, to]);
      }
    }
    let count = 0;
    for (let i = 0; i < pairs.length; i += 1) {
      for (let j = i + 1; j < pairs.length; j += 1) {
        const a = pairs[i];
        const b = pairs[j];
        if (a !== undefined && b !== undefined && (a[0] - b[0]) * (a[1] - b[1]) < 0) {
          count += 1;
        }
      }
    }
    return count;
  };
  const around = (depth: number) =>
    (depth > 0 ? between(layers[depth - 1] ?? [], layers[depth] ?? []) : 0) +
    (depth + 1 < layers.length
      ? between(layers[depth] ?? [], layers[depth + 1] ?? [])
      : 0);

  for (let round = 0; round < TRANSPOSE_ROUNDS; round += 1) {
    let improved = false;
    for (let depth = 0; depth < layers.length; depth += 1) {
      const layer = layers[depth];
      if (layer === undefined) {
        continue;
      }
      for (let i = 0; i + 1 < layer.length; i += 1) {
        const left = layer[i];
        const right = layer[i + 1];
        if (left === undefined || right === undefined) {
          continue;
        }
        const cost = around(depth);
        layer[i] = right;
        layer[i + 1] = left;
        if (around(depth) < cost) {
          improved = true;
          reindex();
        } else {
          layer[i] = left;
          layer[i + 1] = right;
        }
      }
    }
    if (!improved) {
      break;
    }
  }
}

const BARYCENTRE_SWEEPS = 6;
const TRANSPOSE_ROUNDS = 4;

/** The files a drill view draws: all of them once the region is revealed,
 * otherwise the [`DRILL_DEFAULT_CARDS`] that carry it.
 *
 * "Carry it" is the map's own word, not a second opinion computed here — the
 * published significance (ADR-0010), which the tour selects on and the
 * rankings rank on, ordered by the comparator all three share so the cut
 * falls in the same place for all of them. A map that publishes none leaves
 * every file tied, and then path order decides, which is why a region can
 * never come out empty.
 *
 * The chosen files keep the order the region lists them in rather than
 * arriving sorted by significance: the drawing below is a layered layout
 * whose within-layer ordering starts from the order it is handed, and
 * re-sorting here would move cards for reasons that have nothing to do with
 * how they connect. */
function drawnFiles(
  region: Region,
  revealed: ReadonlySet<string>,
): MapNode[] {
  if (revealed.has(region.id) || region.files.length <= DRILL_DEFAULT_CARDS) {
    return region.files;
  }
  const carrying = new Set(
    [...region.files]
      .sort(bySignificance)
      .slice(0, DRILL_DEFAULT_CARDS)
      .map((f) => f.id),
  );
  return region.files.filter((f) => carrying.has(f.id));
}

/** The regions whose default drill view holds one of these files back — what
 * a feature pointing at those files has to reveal for its pointer to land on
 * something rather than on nothing.
 *
 * The answer is regions, not cards, for two reasons. A lone extra card among
 * forty is a card with no neighbours and no context, and the reader followed
 * the pointer *into* a region; and the revealed set the "show all" affordance
 * writes is already region-keyed, so answering in regions keeps every way of
 * revealing on the one projection input instead of growing a second.
 *
 * Read off [`drawnFiles`] — the same rule the canvas draws by — so this can
 * neither reveal a region that was already showing the target nor miss one
 * that was not. A file no region holds names no region: an overlay may mark a
 * path this grouping has no card for, and inventing one would be worse than
 * saying nothing. */
export function regionsHiding(
  regions: readonly Region[],
  fileIds: ReadonlySet<string>,
): Set<string> {
  const hiding = new Set<string>();
  if (fileIds.size === 0) {
    return hiding;
  }
  for (const region of regions) {
    // A region the default view draws whole hides nothing, and asking which
    // forty of its thirty files carry it is work with no answer to give.
    if (hiddenByDefault(region) === 0) {
      continue;
    }
    const drawn = new Set(
      drawnFiles(region, NOTHING_REVEALED).map((f) => f.id),
    );
    if (region.files.some((f) => fileIds.has(f.id) && !drawn.has(f.id))) {
      hiding.add(region.id);
    }
  }
  return hiding;
}

/** Drilled into one region: its files, and the relationships among them.
 * Links leaving the region are the overview's business, not this view's.
 *
 * Laid out the way the overview is — imports run downward, so what a file
 * leans on sits below it — and then ordered within each layer to keep the
 * lines apart. Order alone is worth about two thirds of the crossings on this
 * repository's own densest region; a grid in alphabetical order draws the
 * same graph as a ball of wool. Files that touch nothing else in the region
 * take no part in that and are parked in a block underneath, where they
 * neither widen the layers nor push connected files away from each other. */
export function fileFlow(
  map: KnowledgeGraph,
  region: Region,
  /** Card height, which the label preset decides: a caption needs room, and
   * a card that clips it is worse than a card without one. */
  cardHeight: number = NODE_HEIGHT,
  captionOf?: (node: MapNode) => string | null,
  /** The regions the reader has opened in full, by region ID. An argument
   * rather than state inside here: the same map in the same state must always
   * draw the same picture, and a projection that remembered what it drew last
   * could not promise that. Every way of revealing — the affordance, and the
   * features that point at a specific file — feeds this one input. */
  revealed: ReadonlySet<string> = NOTHING_REVEALED,
): { nodes: AppFlowNode[]; edges: FlowEdge[] } {
  const files = drawnFiles(region, revealed);
  const inside = new Set(files.map((f) => f.id));
  const links = map.edges.filter(
    (e) =>
      e.kind === "imports" &&
      e.source !== e.target &&
      inside.has(e.source) &&
      inside.has(e.target),
  );

  const touched = new Set(links.flatMap((l) => [l.source, l.target]));
  const connected = files.filter((f) => touched.has(f.id));
  const standalone = files.filter((f) => !touched.has(f.id));

  const layers = layersOf(
    connected.map((f) => f.id),
    links,
  );
  orderLayers(layers, links);

  const pitch = NODE_WIDTH + GAP;
  const widest = Math.max(1, ...layers.map((layer) => layer.length));
  // The parked block is a grid in its own right, never a column: a region
  // whose files import nothing from each other has no layers to take a width
  // from, and twenty cards stacked one per row is not a drawing of anything.
  const cols = Math.max(1, Math.min(STANDALONE_COLS, standalone.length));
  const span = Math.max(widest, cols);

  const at = new Map<string, { x: number; y: number }>();
  layers.forEach((layer, depth) => {
    layer.forEach((id, i) => {
      // Layers are centred on each other rather than left-aligned, which is
      // what keeps a two-file layer under the middle of the ten-file layer
      // feeding it instead of off at one end with its links stretched across.
      at.set(id, {
        x: (i - (layer.length - 1) / 2 + (span - 1) / 2) * pitch,
        y: depth * (cardHeight + LAYER_GAP),
      });
    });
  });

  const below =
    layers.length === 0 ? 0 : layers.length * (cardHeight + LAYER_GAP) + LAYER_GAP;
  standalone.forEach((file, i) => {
    at.set(file.id, {
      x: ((i % cols) + (span - cols) / 2) * pitch,
      y: below + Math.floor(i / cols) * (cardHeight + GAP),
    });
  });

  const nodes: AppFlowNode[] = files.map((node) => ({
    id: node.id,
    type: "entity",
    position: at.get(node.id) ?? { x: 0, y: 0 },
    width: NODE_WIDTH,
    height: cardHeight,
    data: { node, ...caption(captionOf?.(node)) },
  }));

  // No label. Every edge on this canvas is an import, and eighty-five
  // identical `imports` chips scattered over the drawing say nothing that
  // the view's own heading does not, while hiding the lines they sit on.
  const edges: FlowEdge[] = links.map((e) => ({
    id: `${e.source}--${e.kind}--${e.target}`,
    source: e.source,
    target: e.target,
  }));

  return fanned(nodes, edges);
}

/** The map's nodes by ID — the lookup every panel needs to turn a node
 * reference (an edge's other end, a tour stop, a flow step) into something
 * it can name or point at. */
export function nodesById(map: KnowledgeGraph): Map<string, MapNode> {
  return new Map(map.nodes.map((node) => [node.id, node]));
}

/** Case-insensitive substring search over node names, paths and summaries.
 * Summaries are searched because that is where enrichment puts the prose a
 * reader remembers a file by. */
export function searchNodes(map: KnowledgeGraph, query: string): MapNode[] {
  const q = query.trim().toLowerCase();
  if (q === "") {
    return [];
  }
  return map.nodes.filter(
    (n) =>
      n.name.toLowerCase().includes(q) ||
      n.path.toLowerCase().includes(q) ||
      n.summary.toLowerCase().includes(q),
  );
}
