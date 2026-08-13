// Where an edge meets a card.
//
// Every card used to expose one attachment point per side, so every edge
// touching a well-connected file arrived at the same pixel: twelve lines
// became one thick smear that said nothing about which file connects to
// which. That convergence — not layer assignment — is the diagnosed defect
// ([ADR-0011](../../../docs/adr/0011-no-layout-library-a-share-ceiling-enforces-it.md)),
// which is why the hand-rolled layout was kept and no engine taken: React
// Flow draws the edges either way, and a layout library would never have
// reached this.
//
// Pure and synchronous, like the rest of the projection: same map, same
// state, byte-identical anchors. Nothing here reads iteration order over a
// hash map — the point an edge gets is a function of that edge and the card,
// through the two rules below.
import { type HandleType, type NodeHandle, Position } from "@xyflow/react";
import { byPath } from "./significance.js";

/** One attachment point on a card's edge: a place a single edge lands. */
export type Anchor = {
  /** Handle ID, unique within the card, which the edge names to claim it. */
  id: string;
  /** `source` for an edge leaving the card, `target` for one arriving. */
  type: HandleType;
  /** The side the point sits on: [`Position.Bottom`] for a source (imports
   * run downward, so what a file leans on sits below it), [`Position.Top`]
   * for a target. */
  position: Position;
  /** Distance from the card's left edge to the point, in pixels. */
  x: number;
};

/** A drawn card, reduced to what deciding an anchor needs. */
export type Card = {
  id: string;
  /** Left edge, in canvas coordinates — the card's own `position.x`. */
  x: number;
  width: number;
};

/** A drawn edge, reduced the same way. */
export type Span = { id: string; source: string; target: string };

/** The handle IDs one edge attaches to, at each of its ends. */
export type Ends = { source: string; target: string };

export type Fan = {
  /** The points each card exposes, by card ID. */
  anchors: Map<string, Anchor[]>;
  /** The point each edge takes at each end, by edge ID. */
  ends: Map<string, Ends>;
};

/** How far from a card's corner the outermost point sits. A line landing on
 * the very corner reads as passing the card rather than arriving at it. */
const INSET = 14;

/** The closest two points are allowed to sit. Below this the fan stops being
 * separate lines again, which is the whole defect. */
const MIN_PITCH = 12;

/** The handle box React Flow measures, in pixels. Square, and centred on the
 * point rather than started at it — see [`handlesOf`]. */
const HANDLE_SIZE = 6;

/** Gives every edge its own point on both cards it touches.
 *
 * **The order.** Within one side of one card the edges are ranked by where
 * their *other* end sits horizontally, ties broken on edge ID. That is what
 * makes the result read as a fan rather than as lines that merely start
 * apart: the edge going furthest left takes the leftmost point, so no two
 * curves leaving the same card cross each other on the way out. The tie-break
 * is the producer's string order (`byPath`, shared with the significance
 * ordering) rather than the order the map happens to list its edges in, so
 * reordering the map's edge list cannot move a single anchor.
 *
 * **The spread.** A card of width `w` offers points between `INSET` and
 * `w - INSET`, spread evenly, and a card with one edge on a side keeps that
 * side's centre. How many points fit is `MIN_PITCH` apart across that span —
 * the card's *capacity*.
 *
 * **Beyond capacity.** Points repeat rather than overflow: the edges keep
 * their left-to-right rank and the rank is scaled onto the points that fit,
 * so edge `k` of `n` takes point `floor(k · used / n)`. Points therefore
 * repeat in ascending runs — two neighbouring edges may share a point, but
 * the fan still opens left to right, nothing is drawn off the card, and
 * nothing collapses back to the centre.
 *
 * An edge whose either end is not a drawn card is skipped: it cannot be
 * drawn, so it has nowhere to land. */
export function fanOut(
  cards: readonly Card[],
  edges: readonly Span[],
): Fan {
  const card = new Map(cards.map((c) => [c.id, c]));
  const centre = (id: string) => {
    const c = card.get(id);
    return c === undefined ? 0 : c.x + c.width / 2;
  };

  // Both sides of every card are gathered before anything is placed: which
  // point an edge gets depends on how many others share its side.
  const leaving = new Map<string, Span[]>();
  const arriving = new Map<string, Span[]>();
  for (const edge of edges) {
    if (!card.has(edge.source) || !card.has(edge.target)) {
      continue;
    }
    group(leaving, edge.source).push(edge);
    group(arriving, edge.target).push(edge);
  }

  const anchors = new Map<string, Anchor[]>();
  const ends = new Map<string, Ends>();
  for (const self of cards) {
    place(self, "source", leaving.get(self.id) ?? [], anchors, ends, centre);
    place(self, "target", arriving.get(self.id) ?? [], anchors, ends, centre);
  }
  return { anchors, ends };
}

/** The static handle geometry the anchors mean, in React Flow's shape.
 *
 * Static rather than measured, as it was before per-edge anchors existed: it
 * is the mechanism React Flow's own SSR guidance uses, and it is what makes
 * the canvas draw under jsdom, where nothing has a bounding box. React Flow
 * reads a handle as a box and takes the middle of the side the box sits on,
 * so the box is centred on the point rather than started at it — otherwise
 * every line would land half a handle to the right of where it was placed. */
export function handlesOf(
  anchors: readonly Anchor[],
  height: number,
): NodeHandle[] {
  return anchors.map((a) => ({
    id: a.id,
    type: a.type,
    position: a.position,
    x: a.x - HANDLE_SIZE / 2,
    y: a.position === Position.Top ? 0 : height - HANDLE_SIZE,
    width: HANDLE_SIZE,
    height: HANDLE_SIZE,
  }));
}

/** Appends to the list under `key`, starting one if this is the first. */
function group(index: Map<string, Span[]>, key: string): Span[] {
  const list = index.get(key);
  if (list !== undefined) {
    return list;
  }
  const started: Span[] = [];
  index.set(key, started);
  return started;
}

/** Places one side of one card: ranks its edges, cuts the points, and hands
 * each edge the point its rank earns. */
function place(
  self: Card,
  type: HandleType,
  edges: readonly Span[],
  anchors: Map<string, Anchor[]>,
  ends: Map<string, Ends>,
  centre: (id: string) => number,
): void {
  if (edges.length === 0) {
    return;
  }
  const otherEnd = (e: Span) => (e.source === self.id ? e.target : e.source);
  const ranked = [...edges].sort(
    (a, b) => centre(otherEnd(a)) - centre(otherEnd(b)) || byPath(a.id, b.id),
  );

  const position = type === "source" ? Position.Bottom : Position.Top;
  const points = pointsOn(self.width, ranked.length);
  const side = anchors.get(self.id) ?? [];
  const placed = points.map((x, i) => {
    const anchor: Anchor = { id: `${type[0]}${i}`, type, position, x };
    side.push(anchor);
    return anchor;
  });
  anchors.set(self.id, side);

  ranked.forEach((edge, k) => {
    const anchor = placed[Math.floor((k * placed.length) / ranked.length)];
    if (anchor === undefined) {
      return;
    }
    const both = ends.get(edge.id) ?? { source: "", target: "" };
    both[type] = anchor.id;
    ends.set(edge.id, both);
  });
}

/** The distinct points a card of this width offers `n` edges: `n` of them if
 * they fit `MIN_PITCH` apart, otherwise the card's capacity. A single edge
 * keeps the centre, which is where every edge used to land. */
function pointsOn(width: number, n: number): number[] {
  const span = Math.max(0, width - 2 * INSET);
  const capacity = Math.max(1, Math.floor(span / MIN_PITCH) + 1);
  const used = Math.min(n, capacity);
  if (used <= 1) {
    return [width / 2];
  }
  return Array.from({ length: used }, (_, i) => INSET + (i * span) / (used - 1));
}
