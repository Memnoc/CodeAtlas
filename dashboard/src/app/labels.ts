// What a card says about itself, what a focused edge calls itself, and the
// plain-words account of a node the panels show when you look at one.
//
// Settled 2026-08-11 after comparing three presets on a real map: **region
// cards read how they sit** in the graph, **file cards carry the map's own
// summary** of the node. The two card types are answering different
// questions, so they get different captions — a region is a place, and what
// you want to know is what leans on it; a file is a thing, and what you want
// to know is what it is.
import type { KnowledgeGraph, Node as MapNode } from "../index.js";
import type { Region, RegionLink } from "./regions.js";

/** Height of a file card: name, kind, and two lines of summary. */
export const CARD_HEIGHT = 92;

/** Fan-in and fan-out per node, over the edges that relate two files. */
export type Degrees = Map<string, { in: number; out: number }>;

export function degreesOf(map: KnowledgeGraph): Degrees {
  const degrees: Degrees = new Map();
  const bump = (id: string, key: "in" | "out") => {
    const entry = degrees.get(id) ?? { in: 0, out: 0 };
    entry[key] += 1;
    degrees.set(id, entry);
  };
  for (const edge of map.edges) {
    if (edge.kind !== "imports" && edge.kind !== "calls") {
      continue;
    }
    bump(edge.source, "out");
    bump(edge.target, "in");
  }
  return degrees;
}

/** The caption under a file card's name: the map's own summary of the node.
 * Mechanical ("TypeScript file, 214 lines: 3 functions") until enrichment
 * replaces it with prose, which is the slot enrichment exists to fill — so
 * this caption gets better the moment a map is enriched, without the
 * dashboard learning anything new. */
export function captionOf(node: MapNode): string | null {
  return node.summary === "" ? null : node.summary;
}

/** The caption under a region card's description: how the region sits among
 * the others. A region already says what it holds; what it cannot say for
 * itself is whether the rest of the repository leans on it. */
export function regionCaptionOf(
  region: Region,
  links: readonly RegionLink[],
): string {
  const leansOn = links.filter((l) => l.source === region.id).length;
  const leanedOn = links.filter((l) => l.target === region.id).length;
  if (leansOn === 0 && leanedOn === 0) {
    return "Keeps to itself";
  }
  if (leansOn === 0) {
    return `Foundation — ${subject(leanedOn, "region", "leans", "lean")} on it`;
  }
  if (leanedOn === 0) {
    return `A way in — leans on ${count(leansOn, "region")}`;
  }
  return `Leans on ${leansOn} · ${subject(leanedOn, "region", "leans", "lean")} on it`;
}

/** What a focused edge calls itself, from the selected file's point of view.
 * Direction is the whole point: a word that changes with the direction saves
 * the reader decoding an arrowhead. Plain words rather than `imports` —
 * every edge the canvas draws is an import, so the precise term adds nothing
 * the view does not already state. */
export function edgeLabelOf(outgoing: boolean): string {
  return outgoing ? "uses" : "used by";
}

/** A plain-words account of one node, as sentences.
 *
 * Everything here is read off the graph, so it is honest about what it can
 * know: **what the file holds**, **who reaches it**, and **what it reaches**.
 * It deliberately does not claim to say what the code *means* — that is
 * `node.summary` once a map has been enriched, and [`enrichmentHint`] says so
 * rather than letting a structural reading pass for a semantic one.
 *
 * Written as sentences rather than a stat line because "fan-in 0, fan-out 8"
 * is the same fact in a form that has to be translated before it is read. */
export function narrativeOf(
  map: KnowledgeGraph,
  node: MapNode,
  byId: ReadonlyMap<string, MapNode>,
): string[] {
  const sentences: string[] = [];

  const held = map.edges
    .filter((e) => e.kind === "contains" && e.source === node.id)
    .flatMap((e) => {
      const child = byId.get(e.target);
      return child === undefined ? [] : [child];
    });
  if (held.length > 0) {
    sentences.push(
      `Holds ${count(held.length, "definition")}: ${listOf(held.map((h) => h.name))}.`,
    );
  }

  // Who reaches it, and what it reaches — named, not counted. A name is
  // something a reader can go and look at; a number is something they have
  // to go and find out.
  const relating = (kind: "imports" | "calls") =>
    map.edges.filter((e) => e.kind === kind);
  const inbound = unique(
    [...relating("imports"), ...relating("calls")]
      .filter((e) => e.target === node.id)
      .flatMap((e) => nameOf(byId, e.source)),
  );
  const outbound = unique(
    [...relating("imports"), ...relating("calls")]
      .filter((e) => e.source === node.id)
      .flatMap((e) => nameOf(byId, e.target)),
  );

  if (inbound.length === 0 && outbound.length === 0) {
    sentences.push("Nothing in this map reaches it, and it reaches nothing.");
    return sentences;
  }
  sentences.push(
    inbound.length === 0
      ? "Nothing in this map reaches it, so it is a way in."
      : `Reached by ${listOf(inbound)}.`,
  );
  if (outbound.length > 0) {
    sentences.push(`It reaches ${listOf(outbound)}.`);
  } else {
    sentences.push("It reaches nothing further — the walk ends here.");
  }
  return sentences;
}

/** Said once, where a reader is looking at a structural summary and might
 * reasonably expect prose: the map can describe shape without an LLM, and
 * only an enriched map describes intent. `null` once this node carries
 * enrichment, because then the prose is already there. */
export function enrichmentHint(node: MapNode): string | null {
  return node.provenance === "llm"
    ? null
    : "This is read off the structure. Run `codeatlas scan --enrich` for a written explanation of what it does.";
}

function nameOf(byId: ReadonlyMap<string, MapNode>, id: string): string[] {
  const node = byId.get(id);
  return node === undefined ? [] : [node.name];
}

function unique(names: readonly string[]): string[] {
  return [...new Set(names)];
}

/** At most three names, then a count of the rest — a sentence that lists
 * fifteen files is a list wearing a sentence.
 *
 * The last item takes an "and" only when the list really ends there.
 * Otherwise the remainder carries it, because "wide, repo and drawn, and 4
 * more" trips a reader over two conjunctions in six words. */
function listOf(names: readonly string[]): string {
  const shown = names.slice(0, 3);
  const rest = names.length - shown.length;
  if (rest > 0) {
    return `${shown.join(", ")}, and ${rest} more`;
  }
  if (shown.length <= 1) {
    return shown[0] ?? "";
  }
  return `${shown.slice(0, -1).join(", ")} and ${shown[shown.length - 1]}`;
}

function count(n: number, noun: string): string {
  return `${n} ${noun}${n === 1 ? "" : "s"}`;
}

/** A counted subject with its verb agreeing: "1 region leans", "3 regions
 * lean". Worth the helper — "1 region lean on it" reads as a bug in the map
 * rather than in the prose. */
function subject(
  n: number,
  noun: string,
  singular: string,
  plural: string,
): string {
  return `${count(n, noun)} ${n === 1 ? singular : plural}`;
}
