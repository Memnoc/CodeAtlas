// Regions: the unit the overview canvas draws. A region is a group of files
// with a name, a size and a description — either a structural layer (the
// directory-derived grouping every file node carries) or a domain (the
// grouping the map's call flows are already bucketed into).
//
// Everything here is derived from the published contract as it stands; no
// field was added to the map for it. Where the reference UI shows a number
// the contract does not publish — a region's file count, its complexity
// word — the rule is written down here and used by every consumer, so two
// panels can never disagree about the same region.
import type { KnowledgeGraph, Node as MapNode, Provenance } from "../index.js";

/** How busy a region is, as a word. See [`complexityOf`] for the rule. */
export type Complexity = "simple" | "moderate" | "complex";

export type Region = {
  id: string;
  /** Display name: mechanical (the directory) or enriched; provenance says. */
  name: string;
  /** Mechanically derived one-liner — the contract publishes no prose. */
  description: string;
  provenance: Provenance;
  /** The file nodes in this region, in map order. */
  files: MapNode[];
  complexity: Complexity;
};

/** The two groupings the Domain | Structural control switches between. */
export type RegionKind = "structural" | "domain";

/** A counted connection between two regions, as drawn on the overview. */
export type RegionLink = {
  source: string;
  target: string;
  count: number;
  /** `imports` or `calls` when every counted edge agrees; else `links`. */
  label: string;
};

/** The edge kinds that connect one file to another. `contains` and `exports`
 * both point from a file to its own symbols, so they say nothing about how
 * two regions relate and are left out of every count on the overview. */
const RELATING: ReadonlySet<string> = new Set(["imports", "calls"]);

/** Region of each file path — the lookup symbol nodes need, since they carry
 * no layer of their own and inherit their file's through containment. */
function fileRegions(map: KnowledgeGraph): Map<string, string> {
  const byPath = new Map<string, string>();
  for (const node of map.nodes) {
    if (node.kind === "file") {
      byPath.set(node.path, node.layer ?? "root");
    }
  }
  return byPath;
}

/** The region a node belongs to under the structural grouping. */
export function regionOfNode(
  node: MapNode,
  byPath: ReadonlyMap<string, string>,
): string {
  return node.kind === "file"
    ? (node.layer ?? "root")
    : (byPath.get(node.path) ?? "root");
}

/** Structural regions: the map's declared layers in contract order, then any
 * layer only discovered through node assignment (older maps omit `layers`).
 * A declared layer with no files is kept — that it is empty is worth seeing,
 * and dropping it would make the region count disagree with the map's own. */
export function structuralRegions(map: KnowledgeGraph): Region[] {
  const members = new Map<string, MapNode[]>();
  for (const layer of map.layers ?? []) {
    members.set(layer.id, []);
  }
  for (const node of map.nodes) {
    if (node.kind !== "file") {
      continue;
    }
    const id = node.layer ?? "root";
    const bucket = members.get(id);
    if (bucket === undefined) {
      members.set(id, [node]);
    } else {
      bucket.push(node);
    }
  }

  const declared = new Map((map.layers ?? []).map((l) => [l.id, l]));
  const byPath = fileRegions(map);
  const counts = weightPerRegion(map, (node) => regionOfNode(node, byPath));
  return [...members].map(([id, files]) => ({
    id,
    name: declared.get(id)?.name ?? id,
    description: describeDirectory(id),
    provenance: declared.get(id)?.provenance ?? "structural",
    files,
    complexity: complexityOf(files.length, counts.get(id) ?? 0),
  }));
}

/** The region holding every file no call flow runs through. Domains cover
 * only the files their flows touch — on this repository's own map that is 85
 * of 219 — and without somewhere for the rest to go, two thirds of the files
 * would have no card, no chip, no row, and no way in at all. */
export const UNROUTED = "no-call-flow";

/** Domain regions: the buckets the map's flows already carry, plus a final
 * region for everything they miss.
 *
 * Membership is **exclusive** — a file worked by several domains belongs to
 * the first that claimed it, and `domain_flows` is emitted in a stable order
 * so "first" is deterministic. That matters beyond tidiness: a region's file
 * count, its complexity word and its drawn links are three readings of one
 * grouping, and letting membership overlap while the other two stayed
 * exclusive made a domain whose files were all claimed earlier report a full
 * file count, no links, and `simple` forever. One rule, applied once. */
export function domainRegions(map: KnowledgeGraph): Region[] {
  const byId = new Map(map.nodes.map((n) => [n.id, n]));
  const files = map.nodes.filter((n) => n.kind === "file");
  const filesByPath = new Map(files.map((n) => [n.path, n]));
  const flowCounts = new Map<string, number>();
  const owner = new Map<string, string>();

  for (const flow of map.domain_flows ?? []) {
    flowCounts.set(flow.domain, (flowCounts.get(flow.domain) ?? 0) + 1);
    for (const step of flow.steps) {
      const path = byId.get(step)?.path;
      if (path !== undefined && filesByPath.has(path) && !owner.has(path)) {
        owner.set(path, flow.domain);
      }
    }
  }
  // No flows, no domains — and no catch-all either, since there is nothing
  // for it to be the remainder of.
  if (flowCounts.size === 0) {
    return [];
  }

  const members = new Map<string, MapNode[]>();
  for (const domain of flowCounts.keys()) {
    members.set(domain, []);
  }
  members.set(UNROUTED, []);
  for (const file of files) {
    members.get(owner.get(file.path) ?? UNROUTED)?.push(file);
  }

  const counts = weightPerRegion(map, (node) =>
    filesByPath.has(node.path)
      ? (owner.get(node.path) ?? UNROUTED)
      : undefined,
  );
  return [...members].map(([id, held]) => {
    const flows = flowCounts.get(id) ?? 0;
    return {
      id,
      name: id === UNROUTED ? "No call flow" : id,
      description:
        id === UNROUTED
          ? "Files no call flow runs through"
          : `${flows} call ${plural("flow", flows)} rooted here`,
      provenance: "structural" as const,
      files: held,
      complexity: complexityOf(held.length, counts.get(id) ?? 0),
    };
  });
}

/** Relating edges touching each region, counted once per edge end, so an
 * edge inside one region counts twice for it and a crossing edge counts once
 * for each side. That is what "how busy is this region" means. */
function weightPerRegion(
  map: KnowledgeGraph,
  regionOf: (node: MapNode) => string | undefined,
): Map<string, number> {
  const byId = new Map(map.nodes.map((n) => [n.id, n]));
  const counts = new Map<string, number>();
  for (const edge of map.edges) {
    if (!RELATING.has(edge.kind)) {
      continue;
    }
    for (const end of [edge.source, edge.target]) {
      const node = byId.get(end);
      const region = node === undefined ? undefined : regionOf(node);
      if (region !== undefined) {
        counts.set(region, (counts.get(region) ?? 0) + 1);
      }
    }
  }
  return counts;
}

/** Relationships per file, banded. The reference UI shows the word without
 * saying what produces it; this is CodeAtlas's rule, and the UI states it
 * where the word appears rather than leaving a reader to guess. An empty
 * region is simple: nothing to understand. */
export function complexityOf(fileCount: number, relating: number): Complexity {
  if (fileCount === 0) {
    return "simple";
  }
  const perFile = relating / fileCount;
  if (perFile < 1) {
    return "simple";
  }
  return perFile < 3 ? "moderate" : "complex";
}

/** Which region owns each file path. Regions do not overlap by
 * construction — both groupings assign a file exactly once — but the lookup
 * still guards against a duplicate rather than silently double-counting the
 * edges touching it. */
export function fileOwners(regions: readonly Region[]): Map<string, string> {
  const owners = new Map<string, string>();
  for (const region of regions) {
    for (const file of region.files) {
      if (!owners.has(file.path)) {
        owners.set(file.path, region.id);
      }
    }
  }
  return owners;
}

/** Counted connections between distinct regions. Self-links are dropped: a
 * region's internal traffic is what its complexity word is for, and drawing
 * it would put a loop on every card. */
export function regionLinks(
  map: KnowledgeGraph,
  regions: readonly Region[],
): RegionLink[] {
  const regionOfFile = fileOwners(regions);
  const byId = new Map(map.nodes.map((n) => [n.id, n]));

  type Tally = {
    source: string;
    target: string;
    count: number;
    kinds: Set<string>;
  };
  const tally = new Map<string, Tally>();
  for (const edge of map.edges) {
    if (!RELATING.has(edge.kind)) {
      continue;
    }
    const source = regionOfFile.get(byId.get(edge.source)?.path ?? "");
    const target = regionOfFile.get(byId.get(edge.target)?.path ?? "");
    if (source === undefined || target === undefined || source === target) {
      continue;
    }
    const key = `${source} ${target}`;
    const entry = tally.get(key) ?? {
      source,
      target,
      count: 0,
      kinds: new Set<string>(),
    };
    entry.count += 1;
    entry.kinds.add(edge.kind);
    tally.set(key, entry);
  }

  return [...tally.values()].map(({ source, target, count, kinds }) => {
    // One kind gets named; a mix is just "links", because "12 imports and
    // calls" is a number nobody can act on. Contract kinds are verb forms —
    // "A imports B" — so the counted noun has to drop their s first:
    // 1 import, 2 imports. Pluralising the verb printed "2 importss" on
    // every busy region edge.
    const kind = kinds.size === 1 ? ([...kinds][0] ?? "link") : "link";
    const noun = kind.replace(/s$/, "");
    return { source, target, count, label: `${count} ${plural(noun, count)}` };
  });
}

/** The regions of a map under the chosen grouping. */
export function regionsOf(map: KnowledgeGraph, kind: RegionKind): Region[] {
  return kind === "structural" ? structuralRegions(map) : domainRegions(map);
}

/** Mechanical description of a directory-derived region, matching how the
 * CLI describes the same thing. */
function describeDirectory(id: string): string {
  return id === "root" ? "Files at the repository root" : `Files under ${id}/`;
}

function plural(word: string, n: number): string {
  return n === 1 ? word : `${word}s`;
}
