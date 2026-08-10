// The derivations the new panels read. These are the numbers the reference
// UI shows but the contract does not publish — region file counts, the
// complexity word, the two rankings — so the rules live in the dashboard and
// are pinned here. A panel showing a number nobody can reproduce is worse
// than a panel showing none.
import { describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { mapFilename } from "../src/app/export.js";
import {
  entryPoints,
  languageOf,
  languageCounts,
  mostDependedOn,
  projectCounts,
} from "../src/app/insights.js";
import { fileFlow, regionFlow } from "../src/app/graph.js";
import { shortestPath } from "../src/app/paths.js";
import {
  complexityOf,
  domainRegions,
  regionLinks,
  structuralRegions,
  UNROUTED,
} from "../src/app/regions.js";
import smallMap from "./fixtures/small-map.json";
import tourMap from "./fixtures/tour-map.json";

const small = smallMap as KnowledgeGraph;
const tour = tourMap as KnowledgeGraph;

describe("structural regions", () => {
  it("counts files, not nodes, and describes each mechanically", () => {
    const regions = structuralRegions(small);
    const byId = new Map(regions.map((r) => [r.id, r]));

    // src holds two files and two symbols; the count is of files.
    expect(byId.get("src")?.files).toHaveLength(2);
    expect(byId.get("src")?.name).toBe("Source code");
    expect(byId.get("src")?.description).toBe("Files under src/");
    expect(byId.get("docs")?.files).toHaveLength(1);
  });

  it("keeps a declared layer that holds no files", () => {
    const empty: KnowledgeGraph = {
      ...small,
      layers: [
        ...(small.layers ?? []),
        { id: "vendor", name: "vendor", provenance: "structural" },
      ],
    };
    const vendor = structuralRegions(empty).find((r) => r.id === "vendor");

    // Dropping it would make the region count disagree with the map's own.
    expect(vendor?.files).toHaveLength(0);
    expect(vendor?.complexity).toBe("simple");
  });

  it("falls back to one root region for a map with no layers", () => {
    const { layers: _layers, ...bare } = small;
    const regions = structuralRegions(bare as KnowledgeGraph);

    // Every file still lands somewhere; none goes missing.
    expect(regions.flatMap((r) => r.files)).toHaveLength(
      small.nodes.filter((n) => n.kind === "file").length,
    );
  });

  it("names the repository root rather than calling it a directory", () => {
    const rooted: KnowledgeGraph = {
      ...small,
      layers: [{ id: "root", name: "root", provenance: "structural" }],
      nodes: small.nodes.map((n) =>
        n.kind === "file" ? { ...n, layer: "root" } : n,
      ),
    };
    expect(structuralRegions(rooted)[0]?.description).toBe(
      "Files at the repository root",
    );
  });
});

describe("the complexity word", () => {
  // The band boundaries, stated where the rule is stated. The UI puts the
  // same sentence in the card's tooltip.
  it("bands on relationships per file", () => {
    expect(complexityOf(10, 0)).toBe("simple");
    expect(complexityOf(10, 9)).toBe("simple");
    expect(complexityOf(10, 10)).toBe("moderate");
    expect(complexityOf(10, 29)).toBe("moderate");
    expect(complexityOf(10, 30)).toBe("complex");
  });

  it("calls an empty region simple rather than dividing by zero", () => {
    expect(complexityOf(0, 0)).toBe("simple");
    expect(complexityOf(0, 5)).toBe("simple");
  });
});

describe("region links", () => {
  it("counts crossings only, and names the kind when they agree", () => {
    const links = regionLinks(tour, structuralRegions(tour));
    const cliToSrc = links.find(
      (l) => l.source === "cli" && l.target === "src",
    );

    expect(cliToSrc).toBeDefined();
    expect(cliToSrc?.label).toBe(`${cliToSrc?.count} imports`);
  });

  it("draws no loop for a region talking to itself", () => {
    const links = regionLinks(small, structuralRegions(small));

    // src imports within itself; that is what its complexity word is for.
    expect(links.filter((l) => l.source === l.target)).toEqual([]);
  });
});

describe("domain regions", () => {
  it("buckets by the domains the map's flows already carry", () => {
    const domains = domainRegions(tour).map((r) => r.id).sort();
    const declared = [
      ...new Set((tour.domain_flows ?? []).map((f) => f.domain)),
    ];

    // The map's own domains, and one more for what they do not reach.
    expect(domains).toEqual([...declared, UNROUTED].sort());
  });

  it("says how many flows are rooted in each", () => {
    const src = domainRegions(tour).find((r) => r.id === "src");
    expect(src?.description).toBe("1 call flow rooted here");
  });

  it("yields nothing for a map with no flows, rather than throwing", () => {
    const { domain_flows: _flows, ...bare } = small;
    expect(domainRegions(bare as KnowledgeGraph)).toEqual([]);
  });

  it("accounts for every file, including the ones no flow touches", () => {
    // Domains cover only what their flows run through. Without a home for
    // the rest, the panel says "N files in M regions" over rows that sum to
    // far less, and the missing files have no card, chip, row or way in.
    const regions = domainRegions(tour);
    const held = regions.flatMap((r) => r.files.map((f) => f.path));

    expect(new Set(held).size).toBe(held.length);
    expect(held.sort()).toEqual(
      tour.nodes
        .filter((n) => n.kind === "file")
        .map((n) => n.path)
        .sort(),
    );
    expect(regions.some((r) => r.id === UNROUTED)).toBe(true);
  });

  it("gives each file to exactly one domain, so counts and links agree", () => {
    // Overlapping membership with exclusive weighting made a domain whose
    // files were all claimed earlier report a full file count, no links, and
    // `simple` forever.
    const regions = domainRegions(tour);
    const owners = new Map<string, string>();
    for (const region of regions) {
      for (const file of region.files) {
        expect(owners.has(file.path)).toBe(false);
        owners.set(file.path, region.id);
      }
    }

    // Every link's endpoints are regions that really hold files.
    const byId = new Map(regions.map((r) => [r.id, r]));
    for (const link of regionLinks(tour, regions)) {
      expect(byId.get(link.source)?.files.length).toBeGreaterThan(0);
      expect(byId.get(link.target)?.files.length).toBeGreaterThan(0);
    }
  });
});

describe("where to start", () => {
  it("reads the map's own flow roots rather than recomputing them", () => {
    const starts = entryPoints(tour).map((n) => n.id);
    const roots = (tour.domain_flows ?? []).map((f) => f.steps[0]);

    // Same answer as the tour's, which is the point: two panels ranking the
    // same nodes differently is worse than either panel alone.
    expect(starts).toEqual([...new Set(roots)]);
  });

  it("computes the same property directly when a map has no flows", () => {
    const { domain_flows: _flows, ...bare } = tour;
    const starts = entryPoints(bare as KnowledgeGraph).map((n) => n.id);

    expect(starts).toContain("function:cli/run.ts:run");
    // `format` is called by `greet`, so it is nobody's entry point.
    expect(starts).not.toContain("function:src/util.ts:format");
  });
});

describe("what everything leans on", () => {
  it("ranks files by how many others import them", () => {
    const ranked = mostDependedOn(tour);

    expect(ranked.length).toBeGreaterThan(0);
    // Descending, and each count is the real number of importers.
    const counts = ranked.map((r) => r.count);
    expect([...counts].sort((a, b) => b - a)).toEqual(counts);
    for (const { node, count } of ranked) {
      expect(count).toBe(
        tour.edges.filter((e) => e.kind === "imports" && e.target === node.id)
          .length,
      );
    }
  });
});

describe("languages", () => {
  it("reads the extension, including the ones that share a language", () => {
    expect(languageOf("src/app/MapExplorer.tsx")).toBe("TypeScript");
    expect(languageOf("src/app/graph.ts")).toBe("TypeScript");
    expect(languageOf("crates/x/src/main.rs")).toBe("Rust");
    expect(languageOf("README.md")).toBe("Markdown");
  });

  it("does not mistake a dotted directory for an extension", () => {
    expect(languageOf(".github/workflows/ci.yml")).toBe("YAML");
    expect(languageOf("some.dir/Makefile")).toBe("Other");
  });

  it("counts commonest first", () => {
    const counts = languageCounts(tour).map((l) => l.count);
    expect([...counts].sort((a, b) => b - a)).toEqual(counts);
  });
});

describe("the headline count", () => {
  it("counts files and the relationships between them, not containment", () => {
    const counts = projectCounts(small, structuralRegions(small).length);

    expect(counts.files).toBe(
      small.nodes.filter((n) => n.kind === "file").length,
    );
    expect(counts.regions).toBe(2);
    // `contains` and `exports` run from a file to its own symbols, so
    // neither is a relationship *between* things.
    expect(counts.relationships).toBe(
      small.edges.filter((e) => e.kind === "imports" || e.kind === "calls")
        .length,
    );
    expect(counts.relationships).toBeLessThan(small.edges.length);
  });
});

describe("shortest path", () => {
  it("follows edges in both directions", () => {
    // greeter.ts is imported *by* main.ts; a reader asking how they relate
    // does not care which end declared the edge.
    const forward = shortestPath(small, "file:src/main.ts", "file:src/greeter.ts");
    const back = shortestPath(small, "file:src/greeter.ts", "file:src/main.ts");

    expect(forward?.map((n) => n.id)).toEqual([
      "file:src/main.ts",
      "file:src/greeter.ts",
    ]);
    expect(back?.map((n) => n.id)).toEqual([
      "file:src/greeter.ts",
      "file:src/main.ts",
    ]);
  });

  it("returns the shorter of two routes", () => {
    const path = shortestPath(
      small,
      "function:src/main.ts:main",
      "class:src/greeter.ts:Greeter",
    );

    // main → main.ts → greeter.ts → Greeter, through containment and import.
    expect(path?.map((n) => n.id)).toEqual([
      "function:src/main.ts:main",
      "file:src/main.ts",
      "file:src/greeter.ts",
      "class:src/greeter.ts:Greeter",
    ]);
  });

  it("is a single node when both ends are the same", () => {
    expect(
      shortestPath(small, "file:src/main.ts", "file:src/main.ts"),
    ).toHaveLength(1);
  });

  it("says so when nothing joins them", () => {
    // docs/guide.md is in the map but linked to nothing.
    expect(
      shortestPath(small, "file:src/main.ts", "file:docs/guide.md"),
    ).toBeNull();
  });

  it("says so when an endpoint is not in the map", () => {
    expect(shortestPath(small, "file:src/main.ts", "file:nope.ts")).toBeNull();
  });
});

describe("the overview layout", () => {
  /** A map of `n` regions, all at the same dependency depth. */
  function wide(n: number): KnowledgeGraph {
    return {
      ...small,
      layers: Array.from({ length: n }, (_, i) => ({
        id: `l${i}`,
        name: `l${i}`,
        provenance: "structural" as const,
      })),
      nodes: Array.from({ length: n }, (_, i) => ({
        id: `file:l${i}/a.ts`,
        kind: "file" as const,
        name: "a.ts",
        path: `l${i}/a.ts`,
        summary: "",
        layer: `l${i}`,
        provenance: "structural" as const,
      })),
      edges: [],
    };
  }

  it("never draws two region cards on top of each other", () => {
    // Nine regions at one depth wrap to three rows. Placing bands by depth
    // number rather than by rows occupied drew the next band through them.
    const regions = structuralRegions(wide(9));
    const { nodes } = regionFlow(regions, []);

    expect(nodes).toHaveLength(9);
    for (const a of nodes) {
      for (const b of nodes) {
        if (a.id >= b.id) {
          continue;
        }
        const apart =
          Math.abs(a.position.x - b.position.x) >= (a.width ?? 0) ||
          Math.abs(a.position.y - b.position.y) >= (a.height ?? 0);
        expect(apart, `${a.id} and ${b.id} overlap`).toBe(true);
      }
    }
  });

  it("puts what everything leans on below what leans on it", () => {
    const regions = structuralRegions(tour);
    const links = regionLinks(tour, regions);
    const { nodes } = regionFlow(regions, links);
    const y = new Map(nodes.map((n) => [n.id, n.position.y]));

    // cli imports src, so src settles one band lower.
    expect(y.get("region:src") ?? 0).toBeGreaterThan(y.get("region:cli") ?? 0);
  });

  it("drops a link whose region was never drawn", () => {
    const { edges } = regionFlow(structuralRegions(tour), [
      { source: "cli", target: "ghost", count: 1, label: "1 import" },
    ]);

    // Never emitted dangling — the same rule the map itself follows.
    expect(edges).toEqual([]);
  });
});

describe("the region drill-in layout", () => {
  /** A one-region map of `files`, wired by `imports` as `[from, to]` pairs. */
  function repo(
    files: readonly string[],
    imports: readonly (readonly [string, string])[],
  ): KnowledgeGraph {
    return {
      version: "0.2.0",
      project: { name: "layout" },
      layers: [{ id: "app", name: "app", provenance: "structural" }],
      nodes: files.map((f) => ({
        id: `file:app/${f}`,
        kind: "file" as const,
        name: f,
        path: `app/${f}`,
        summary: "",
        layer: "app",
        provenance: "structural" as const,
      })),
      edges: imports.map(([from, to]) => ({
        source: `file:app/${from}`,
        target: `file:app/${to}`,
        kind: "imports" as const,
        weight: 1,
      })),
    };
  }

  /** Where each file was drawn, by filename. */
  function drawn(map: KnowledgeGraph) {
    const region = structuralRegions(map)[0];
    if (region === undefined) {
      throw new Error("fixture has no region");
    }
    const flow = fileFlow(map, region);
    return {
      ...flow,
      at: new Map(
        flow.nodes.map((n) => [
          n.id.replace("file:app/", ""),
          n.position,
        ]),
      ),
    };
  }

  it("draws what a file imports below it", () => {
    const { at } = drawn(repo(["a.ts", "b.ts"], [["a.ts", "b.ts"]]));

    // Same direction as the overview: leaned-on things settle downward.
    expect(at.get("b.ts")?.y ?? 0).toBeGreaterThan(at.get("a.ts")?.y ?? 0);
  });

  it("cuts the link that closes a cycle rather than banding around it", () => {
    // c imports a and x; a and b import each other. Without cutting the
    // closing link, the two of them ratchet each other downward on every
    // pass and drag a out of the row it shares with x — which is how this
    // repository's own crates region came to be laid out in 190 bands.
    const { at } = drawn(
      repo(
        ["a.ts", "b.ts", "c.ts", "x.ts"],
        [
          ["c.ts", "a.ts"],
          ["c.ts", "x.ts"],
          ["a.ts", "b.ts"],
          ["b.ts", "a.ts"],
        ],
      ),
    );

    expect(at.get("a.ts")?.y).toBe(at.get("x.ts")?.y);
    expect(new Set([...at.values()].map((p) => p.y)).size).toBe(3);
  });

  it("orders a layer so that its links stop crossing", () => {
    // a reaches the second of the pair below, b reaches the first. Drawn in
    // the order they arrive, the two links cross; the layer has to be
    // reordered for them not to.
    const { at } = drawn(
      repo(
        ["a.ts", "b.ts", "x.ts", "y.ts"],
        [
          ["a.ts", "y.ts"],
          ["b.ts", "x.ts"],
        ],
      ),
    );

    const leftToRight = (one: string, two: string) =>
      (at.get(one)?.x ?? 0) < (at.get(two)?.x ?? 0);
    expect(leftToRight("y.ts", "x.ts")).toBe(leftToRight("a.ts", "b.ts"));
  });

  it("parks a file with no relationship in the region below the rest", () => {
    const { at } = drawn(
      repo(["a.ts", "b.ts", "loose.ts"], [["a.ts", "b.ts"]]),
    );

    // Still drawn — the panel beside the canvas counts three files — but
    // out of the way of the layers it has nothing to do with.
    expect(at.size).toBe(3);
    expect(at.get("loose.ts")?.y ?? 0).toBeGreaterThan(at.get("b.ts")?.y ?? 0);
  });

  it("draws a region with no relationships as a block, not a column", () => {
    // Nothing here imports anything here, so there are no layers to take a
    // width from. Twenty cards one per row is a list wearing a canvas.
    const files = Array.from({ length: 20 }, (_, i) => `f${i}.ts`);
    const { at } = drawn(repo(files, []));

    const rows = new Set([...at.values()].map((p) => p.y));
    const columns = new Set([...at.values()].map((p) => p.x));
    expect(columns.size).toBeGreaterThan(1);
    expect(rows.size).toBeLessThan(files.length);
  });

  it("never draws two file cards on top of each other", () => {
    const { nodes } = drawn(
      repo(
        ["a.ts", "b.ts", "c.ts", "d.ts", "e.ts", "f.ts", "g.ts"],
        [
          ["a.ts", "b.ts"],
          ["a.ts", "c.ts"],
          ["b.ts", "d.ts"],
          ["c.ts", "d.ts"],
        ],
      ),
    );

    for (const one of nodes) {
      for (const two of nodes) {
        if (one.id >= two.id) {
          continue;
        }
        const apart =
          Math.abs(one.position.x - two.position.x) >= (one.width ?? 0) ||
          Math.abs(one.position.y - two.position.y) >= (one.height ?? 0);
        expect(apart, `${one.id} and ${two.id} overlap`).toBe(true);
      }
    }
  });

  it("labels no edge, because every edge on this canvas is an import", () => {
    const { edges } = drawn(
      repo(
        ["a.ts", "b.ts", "c.ts"],
        [
          ["a.ts", "b.ts"],
          ["b.ts", "c.ts"],
        ],
      ),
    );

    expect(edges).toHaveLength(2);
    expect(edges.every((e) => e.label === undefined)).toBe(true);
  });

  it("draws no self-import, whatever the map says", () => {
    const { edges } = drawn(repo(["a.ts"], [["a.ts", "a.ts"]]));

    // A loop from a card back to itself is a scribble, not information.
    expect(edges).toEqual([]);
  });
});

describe("export filename", () => {
  it("is derived from the project name, safely", () => {
    expect(mapFilename({ ...small, project: { name: "my repo" } })).toBe(
      "my-repo-map.json",
    );
    // Nothing that reads as a relative path or a hidden file.
    expect(mapFilename({ ...small, project: { name: "../etc/passwd" } })).toBe(
      "etc-passwd-map.json",
    );
    expect(mapFilename({ ...small, project: { name: "..." } })).toBe(
      "map-map.json",
    );
  });
});
