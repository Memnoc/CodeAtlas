// The rankings and tallies the info panel reads: where to start, what
// everything leans on, what the project is written in, and how big it is.
//
// All of it is derived from the contract as published. Two of these — the
// entry points and the fan-in ranking — are the "which nodes matter"
// question that the tour also answers, and the map already answers it once:
// `domain_flows` roots are exactly "a function nothing else calls". Reading
// the producer's own answer keeps the panel and the tour from disagreeing.
import type { KnowledgeGraph, Node as MapNode } from "../index.js";

/** How many rows a ranking panel shows. Both rankings have a long tail that
 * says nothing; the panel is a starting point, not an inventory. */
const TOP_N = 6;

export type Ranked = { node: MapNode; count: number };

/** Functions nothing else calls — the roots the map's own flows are built
 * on. Falls back to computing the same property directly for maps that
 * predate `domain_flows`. */
export function entryPoints(map: KnowledgeGraph): MapNode[] {
  const byId = new Map(map.nodes.map((n) => [n.id, n]));
  const flows = map.domain_flows ?? [];
  if (flows.length > 0) {
    const seen = new Set<string>();
    const roots: MapNode[] = [];
    for (const flow of flows) {
      const root = byId.get(flow.steps[0] ?? "");
      if (root !== undefined && !seen.has(root.id)) {
        seen.add(root.id);
        roots.push(root);
      }
    }
    return roots.slice(0, TOP_N);
  }

  const called = new Set(
    map.edges.filter((e) => e.kind === "calls").map((e) => e.target),
  );
  const calling = new Set(
    map.edges.filter((e) => e.kind === "calls").map((e) => e.source),
  );
  return map.nodes
    .filter((n) => calling.has(n.id) && !called.has(n.id))
    .slice(0, TOP_N);
}

/** Files the most other files import, most-imported first. Ties break on
 * path so the order is stable between runs of the same map. */
export function mostDependedOn(map: KnowledgeGraph): Ranked[] {
  const byId = new Map(map.nodes.map((n) => [n.id, n]));
  const fanIn = new Map<string, number>();
  for (const edge of map.edges) {
    if (edge.kind === "imports" && edge.source !== edge.target) {
      fanIn.set(edge.target, (fanIn.get(edge.target) ?? 0) + 1);
    }
  }
  return [...fanIn]
    .flatMap(([id, count]) => {
      const node = byId.get(id);
      return node === undefined ? [] : [{ node, count }];
    })
    .sort((a, b) => b.count - a.count || a.node.path.localeCompare(b.node.path))
    .slice(0, TOP_N);
}

/** File extension to the language name CodeAtlas's parsers use. This mirrors
 * the CLI's parser registry rather than reading it: the contract publishes no
 * per-node language, and a summary is prose once enrichment has run, so it
 * cannot be parsed for one. Anything unlisted is counted as Other. */
const LANGUAGES: ReadonlyMap<string, string> = new Map([
  ["ts", "TypeScript"],
  ["tsx", "TypeScript"],
  ["mts", "TypeScript"],
  ["cts", "TypeScript"],
  ["js", "JavaScript"],
  ["jsx", "JavaScript"],
  ["mjs", "JavaScript"],
  ["cjs", "JavaScript"],
  ["rs", "Rust"],
  ["py", "Python"],
  ["go", "Go"],
  ["c", "C"],
  ["h", "C"],
  ["cpp", "C++"],
  ["cc", "C++"],
  ["cxx", "C++"],
  ["hpp", "C++"],
  ["hh", "C++"],
  ["md", "Markdown"],
  ["json", "JSON"],
  ["css", "CSS"],
  ["html", "HTML"],
  ["toml", "TOML"],
  ["yml", "YAML"],
  ["yaml", "YAML"],
]);

export type LanguageCount = { language: string; count: number };

/** Languages present, commonest first. Ties break on name for stability. */
export function languageCounts(map: KnowledgeGraph): LanguageCount[] {
  const counts = new Map<string, number>();
  for (const node of map.nodes) {
    if (node.kind !== "file") {
      continue;
    }
    const language = languageOf(node.path);
    counts.set(language, (counts.get(language) ?? 0) + 1);
  }
  return [...counts]
    .map(([language, count]) => ({ language, count }))
    .sort((a, b) => b.count - a.count || a.language.localeCompare(b.language));
}

export function languageOf(path: string): string {
  const name = path.slice(path.lastIndexOf("/") + 1);
  const dot = name.lastIndexOf(".");
  const extension = dot === -1 ? "" : name.slice(dot + 1).toLowerCase();
  return LANGUAGES.get(extension) ?? "Other";
}

export type ProjectCounts = {
  files: number;
  regions: number;
  relationships: number;
};

/** The headline line: files, regions, and relationships between them.
 * `contains` and `exports` both run from a file to its own symbols, so
 * neither is a relationship *between* things and neither is counted. */
export function projectCounts(
  map: KnowledgeGraph,
  regions: number,
): ProjectCounts {
  return {
    files: map.nodes.filter((n) => n.kind === "file").length,
    regions,
    relationships: map.edges.filter(
      (e) => e.kind === "imports" || e.kind === "calls",
    ).length,
  };
}
