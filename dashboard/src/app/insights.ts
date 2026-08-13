// The rankings and tallies the info panel reads: where to start, which files
// carry the codebase, what the project is written in, and how big it is.
//
// All of it is read from the contract as published, never re-derived. Two of
// these — the entry points and the significance ranking — are the "which
// nodes matter" question the tour and the drill view also ask, and the map
// answers it once for all of them: `domain_flows` roots are exactly "a
// function nothing else calls", and `significance` is the published
// per-file number (ADR-0010). Reading the producer's own answers is what
// keeps three consumers from naming three different files.
import type { KnowledgeGraph, Node as MapNode } from "../index.js";
import { bySignificance } from "./significance.js";

/** How many rows a ranking panel shows. Both rankings have a long tail that
 * says nothing; the panel is a starting point, not an inventory. */
const TOP_N = 6;

/** One row of the significance ranking: the file, and the number the map
 * published about it — not a count of anything this module tallied. */
export type Ranked = { node: MapNode; significance: number };

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

/** Whether this map publishes significance at all. An empty ranking has two
 * unrelated causes — a map that scored every file zero, and a map that
 * scored nothing (the field is optional; every map written before ADR-0010
 * omits it) — and a reader told "nothing scored above zero" about the second
 * has been told a measurement that was never taken. */
export function publishesSignificance(map: KnowledgeGraph): boolean {
  return map.nodes.some(
    (n) => n.kind === "file" && (n.significance ?? null) !== null,
  );
}

/** The files the map says carry the architecture, most significant first,
 * ties broken on path — the producer's own comparator, so this ranking's top
 * six are the six a tour or a drill view cutting the same list would take.
 *
 * The number is the map's own published significance (ADR-0010) — import
 * fan-in + fan-out + 1 if the file hosts an entry point, computed at scan —
 * and nothing here re-derives it. That is the whole decision: the tour
 * selects on this number, the drill view discloses on it, and this panel
 * ranks on it, so the three cannot disagree about the same repository. The
 * ranking this replaced counted importers and skipped self-imports, which
 * the published formula counts, so the panel and the tour could and did
 * name different files.
 *
 * A file scoring zero is left out rather than listed: nothing leans on it,
 * it leans on nothing, and no walk starts there. On a map written before
 * significance existed that is every file and this returns nothing — which
 * is why the panel asks [`publishesSignificance`] before it says why. */
export function mostSignificant(map: KnowledgeGraph): Ranked[] {
  return map.nodes
    .filter((n) => n.kind === "file" && (n.significance ?? 0) > 0)
    .sort(bySignificance)
    .slice(0, TOP_N)
    .map((node) => ({ node, significance: node.significance ?? 0 }));
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
