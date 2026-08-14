// The derivations the new panels read. These are the numbers the reference
// UI shows but the contract does not publish — region file counts, the
// complexity word, the two rankings — so the rules live in the dashboard and
// are pinned here. A panel showing a number nobody can reproduce is worse
// than a panel showing none.
import type { Edge as FlowEdge } from "@xyflow/react";
import { describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { mapFilename } from "../src/app/export.js";
import {
  entryPoints,
  languageOf,
  languageCounts,
  mostSignificant,
  projectCounts,
} from "../src/app/insights.js";
import {
  type AppFlowNode,
  DRILL_DEFAULT_CARDS,
  fileFlow,
  magnifyFlow,
  neighbourhoodOf,
  NODE_HEIGHT,
  NODE_WIDTH,
  regionFlow,
  regionNodeId,
  regionsHiding,
} from "../src/app/graph.js";
import {
  captionOf,
  edgeLabelOf,
  enrichmentHint,
  narrativeOf,
  regionCaptionOf,
} from "../src/app/labels.js";
import { shortestPath } from "../src/app/paths.js";
import { byPath } from "../src/app/significance.js";
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

  it("prefers the description the contract publishes, each part keeping its own author", () => {
    const described: KnowledgeGraph = {
      ...small,
      layers: [
        {
          id: "docs",
          name: "docs",
          provenance: "structural",
          description: {
            text: "Guides and reference material",
            provenance: "llm",
          },
        },
        {
          id: "src",
          name: "Source code",
          provenance: "llm",
          description: { text: "Files under src/", provenance: "structural" },
        },
      ],
    };
    const byId = new Map(structuralRegions(described).map((r) => [r.id, r]));

    // Mechanical name, purchased description…
    expect(byId.get("docs")?.description).toBe("Guides and reference material");
    expect(byId.get("docs")?.descriptionProvenance).toBe("llm");
    expect(byId.get("docs")?.provenance).toBe("structural");
    // …and the reverse: neither part inherits the other's provenance.
    expect(byId.get("src")?.description).toBe("Files under src/");
    expect(byId.get("src")?.descriptionProvenance).toBe("structural");
    expect(byId.get("src")?.provenance).toBe("llm");
  });

  it("synthesises when the map publishes no description — never an empty card", () => {
    // small-map's layers predate the field entirely: the fallback is the
    // mechanical sentence, owned as structural.
    const byId = new Map(structuralRegions(small).map((r) => [r.id, r]));
    expect(byId.get("docs")?.description).toBe("Files under docs/");
    expect(byId.get("docs")?.descriptionProvenance).toBe("structural");

    // A published-but-blank text is treated as absent, not rendered: the
    // synthesised replacement is the dashboard's own, so it may never wear
    // the publisher's enrichment provenance either.
    const blank: KnowledgeGraph = {
      ...small,
      layers: (small.layers ?? []).map((l) => ({
        ...l,
        description: { text: "", provenance: "llm" as const },
      })),
    };
    const blankDocs = structuralRegions(blank).find((r) => r.id === "docs");
    expect(blankDocs?.description).toBe("Files under docs/");
    expect(blankDocs?.descriptionProvenance).toBe("structural");
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
    // Kind named, singular or plural as the count demands — the exact
    // grammar is pinned below, where the counts are chosen on purpose.
    expect(cliToSrc?.label).toMatch(/^\d+ imports?$/);
  });

  it("draws no loop for a region talking to itself", () => {
    const links = regionLinks(small, structuralRegions(small));

    // src imports within itself; that is what its complexity word is for.
    expect(links.filter((l) => l.source === l.target)).toEqual([]);
  });

  it("counts nouns: 1 import, 2 imports — never the kind's own s twice", () => {
    // The shipped bug this pins: contract kinds are verb forms ("A imports
    // B"), and pluralising the verb put "2 importss" on every busy region
    // edge — immortalised in the first README screenshots before anyone
    // read it back.
    const crossing = (
      edges: readonly (readonly ["imports" | "calls", string, string])[],
    ): KnowledgeGraph => ({
      version: "0.2.0",
      project: { name: "grammar" },
      layers: [
        { id: "one", name: "one", provenance: "structural" as const },
        { id: "two", name: "two", provenance: "structural" as const },
      ],
      nodes: ["one/a.ts", "one/b.ts", "two/z.ts"].map((path) => ({
        id: `file:${path}`,
        kind: "file" as const,
        name: path.split("/")[1] ?? path,
        path,
        summary: "",
        layer: path.split("/")[0] ?? "one",
        provenance: "structural" as const,
      })),
      edges: edges.map(([kind, from, to]) => ({
        source: `file:${from}`,
        target: `file:${to}`,
        kind,
        weight: 1,
      })),
    });

    const label = (map: KnowledgeGraph) =>
      regionLinks(map, structuralRegions(map))[0]?.label;

    expect(label(crossing([["imports", "one/a.ts", "two/z.ts"]]))).toBe(
      "1 import",
    );
    expect(
      label(
        crossing([
          ["imports", "one/a.ts", "two/z.ts"],
          ["imports", "one/b.ts", "two/z.ts"],
        ]),
      ),
    ).toBe("2 imports");
    // A mix stays "links": the noun path, exercised with a real plural.
    expect(
      label(
        crossing([
          ["imports", "one/a.ts", "two/z.ts"],
          ["calls", "one/b.ts", "two/z.ts"],
        ]),
      ),
    ).toBe("2 links");
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
    // Domain text is always the dashboard's own synthesis — domains are not
    // contract entities, so nothing purchasable ever lands here.
    expect(src?.descriptionProvenance).toBe("structural");
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

describe("what matters most", () => {
  /** A map whose published significance and a private fan-in count cannot
   * agree — which is the disagreement ADR-0010 exists to end.
   *
   * Five import edges: hub → a, hub → b, hub → c, hub → hub, a → hub. Under
   * the published formula (fan-in + fan-out, self-imports counted, no entry
   * point here) that is hub 6, a 2, b 1, c 1. Under a fan-in ranking that
   * skips self-imports every one of them scores 1, and the order inverts to
   * path order with the hub last. */
  const disagreeing: KnowledgeGraph = {
    version: "0.4.0",
    project: { name: "rankings" },
    layers: [{ id: "src", name: "src", provenance: "structural" }],
    nodes: (
      [
        ["hub.ts", 6],
        ["a.ts", 2],
        ["b.ts", 1],
        ["c.ts", 1],
      ] as const
    ).map(([name, significance]) => ({
      id: `file:src/${name}`,
      kind: "file" as const,
      name,
      path: `src/${name}`,
      summary: "",
      layer: "src",
      provenance: "structural" as const,
      significance,
    })),
    edges: (
      [
        ["hub.ts", "a.ts"],
        ["hub.ts", "b.ts"],
        ["hub.ts", "c.ts"],
        ["hub.ts", "hub.ts"],
        ["a.ts", "hub.ts"],
      ] as const
    ).map(([from, to]) => ({
      source: `file:src/${from}`,
      target: `file:src/${to}`,
      kind: "imports" as const,
      weight: 1,
    })),
  };

  it("ranks on the significance the map publishes, deriving nothing", () => {
    const ranked = mostSignificant(disagreeing);

    // The published order and the published numbers, verbatim. A ranking
    // that counted importers here would answer a.ts, b.ts, c.ts, hub.ts,
    // every one of them 1 — which is exactly what the panel and the tour
    // used to disagree about.
    expect(ranked.map((r) => r.node.path)).toEqual([
      "src/hub.ts",
      "src/a.ts",
      "src/b.ts",
      "src/c.ts",
    ]);
    expect(ranked.map((r) => r.significance)).toEqual([6, 2, 1, 1]);
    for (const { node, significance } of ranked) {
      expect(significance).toBe(node.significance);
    }
  });

  it("orders a tie the way the map producer does, not the way a locale would", () => {
    // The producer breaks a significance tie on the path's bytes
    // (`a.path.cmp(b.path)`, crates/codeatlas/src/semantics.rs), and this
    // pair is one where byte order and locale collation genuinely disagree:
    // `R` is 0x52 and `a` is 0x61, so bytes put `README.md` first, while
    // collation reads past the case and puts `adr/` first. Checked here
    // rather than asserted in prose, so a fixture that stopped discriminating
    // would say so.
    expect("docs/README.md".localeCompare("docs/adr/index.md")).toBeGreaterThan(
      0,
    );

    const tied: KnowledgeGraph = {
      version: "0.4.0",
      project: { name: "collation" },
      layers: [{ id: "docs", name: "docs", provenance: "structural" }],
      nodes: ["docs/README.md", "docs/adr/index.md"].map((path) => ({
        id: `file:${path}`,
        kind: "file" as const,
        name: path.slice(path.lastIndexOf("/") + 1),
        path,
        summary: "",
        layer: "docs",
        provenance: "structural" as const,
        significance: 3,
      })),
      edges: [],
    };

    // The producer's order, so a top-N cut here takes the file a top-N cut
    // there would have taken. Ranking by collation returns these reversed.
    expect(mostSignificant(tied).map((r) => r.node.path)).toEqual([
      "docs/README.md",
      "docs/adr/index.md",
    ]);
  });

  it("ranks nothing when the map publishes no significance", () => {
    // The field is optional (ADR-0010), and a panel that cannot read the
    // number would rather say so than invent an order of its own.
    expect(mostSignificant(tour)).toEqual([]);
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

  it("never overlaps at a taller card height either", () => {
    // A label preset makes cards taller to fit a caption. If the layout kept
    // banding on the default height, every layer would be drawn through the
    // one below it — the caption feature breaking the layout silently.
    // Enough standalone files to wrap onto a second parked row, which is the
    // tightest pitch in the layout, and a height taller than the gap between
    // layers so the banding has to account for it too.
    const loose = Array.from({ length: 11 }, (_, i) => `loose${i}.ts`);
    const map = repo(
      ["a.ts", "b.ts", "c.ts", ...loose],
      [
        ["a.ts", "b.ts"],
        ["b.ts", "c.ts"],
      ],
    );
    const region = structuralRegions(map)[0];
    if (region === undefined) {
      throw new Error("fixture has no region");
    }

    for (const height of [58, 92, 200]) {
      const { nodes } = fileFlow(map, region, height);
      for (const one of nodes) {
        for (const two of nodes) {
          if (one.id >= two.id) {
            continue;
          }
          const apart =
            Math.abs(one.position.x - two.position.x) >= (one.width ?? 0) ||
            Math.abs(one.position.y - two.position.y) >= height;
          expect(apart, `${one.id} and ${two.id} overlap at ${height}px`).toBe(
            true,
          );
        }
      }
    }
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

describe("the default drill view", () => {
  /** A one-region map of the given files, each `[name, significance]` — a
   * significance of `undefined` omits the field, which is what every map
   * written before ADR-0010 looks like. No edges: what the layout does with
   * them is the block above's business, and what the *selection* does is
   * this one's. */
  function repo(
    files: readonly (readonly [string, number | undefined])[],
  ): KnowledgeGraph {
    return {
      version: "0.4.0",
      project: { name: "drill" },
      layers: [{ id: "app", name: "app", provenance: "structural" }],
      nodes: files.map(([name, significance]) => ({
        id: `file:app/${name}`,
        kind: "file" as const,
        name,
        path: `app/${name}`,
        summary: "",
        layer: "app",
        provenance: "structural" as const,
        ...(significance === undefined ? {} : { significance }),
      })),
      edges: [],
    };
  }

  /** `f007.ts` — zero-padded so path order and index order agree. */
  const named = (i: number) => `f${String(i).padStart(3, "0")}.ts`;

  function onlyRegion(map: KnowledgeGraph) {
    const region = structuralRegions(map)[0];
    if (region === undefined) {
      throw new Error("fixture has no region");
    }
    return region;
  }

  /** The files the drill view draws, by name. */
  function drawn(
    map: KnowledgeGraph,
    revealed: ReadonlySet<string> = new Set(),
  ): string[] {
    return fileFlow(map, onlyRegion(map), NODE_HEIGHT, undefined, revealed)
      .nodes.map((n) => n.id.replace("file:app/", ""));
  }

  it("draws the forty most significant files of a bigger region", () => {
    // Sixty files, significance rising with the index: the forty that carry
    // the region are f020…f059, and the twenty that do not are not drawn.
    const map = repo(Array.from({ length: 60 }, (_, i) => [named(i), i] as const));

    const shown = new Set(drawn(map));

    expect(shown.size).toBe(40);
    for (let i = 20; i < 60; i += 1) {
      expect(shown.has(named(i)), `${named(i)} carries the region`).toBe(true);
    }
    for (let i = 0; i < 20; i += 1) {
      expect(shown.has(named(i)), `${named(i)} is one of the hidden`).toBe(
        false,
      );
    }
    // The region itself is untouched: the default view hides cards, never
    // facts, and every panel counts off this list.
    expect(onlyRegion(map).files).toHaveLength(60);
  });

  it("breaks a tie on path, not on the order the map happens to list", () => {
    // Forty-five files of equal significance, declared in reverse path order.
    // Map order alone would draw f044…f005; the tie-break draws f000…f039.
    const map = repo(
      Array.from({ length: 45 }, (_, i) => [named(44 - i), 5] as const),
    );

    const shown = new Set(drawn(map));

    expect(shown.size).toBe(40);
    expect(shown.has(named(0))).toBe(true);
    expect(shown.has(named(39))).toBe(true);
    expect(shown.has(named(40))).toBe(false);
    expect(shown.has(named(44))).toBe(false);
  });

  it("breaks a tie the way the map producer does, not the way a locale would", () => {
    // Same ordering rule as the tour's, or the two disagree at the cut: the
    // producer compares the path's bytes (`a.path.cmp(b.path)`,
    // crates/codeatlas/src/semantics.rs). Byte order and collation disagree
    // about this pair — `R` is 0x52, `a` is 0x61, so bytes put `README.md`
    // first and collation puts `adr/` first — which is what makes the
    // fixture able to tell them apart at all.
    expect("app/README.md".localeCompare("app/adr/index.md")).toBeGreaterThan(
      0,
    );

    // Thirty-nine files clear of the cut and two tied for the fortieth card:
    // exactly one of the pair is drawn, and which one is the whole question.
    const map = repo([
      ...Array.from({ length: 39 }, (_, i) => [named(i), 10] as const),
      ["README.md", 1] as const,
      ["adr/index.md", 1] as const,
    ]);

    const shown = new Set(drawn(map));

    expect(shown.size).toBe(DRILL_DEFAULT_CARDS);
    expect(shown.has("README.md")).toBe(true);
    expect(shown.has("adr/index.md")).toBe(false);
  });

  it("draws the whole region once the reader has revealed it", () => {
    const map = repo(Array.from({ length: 60 }, (_, i) => [named(i), i] as const));
    const region = onlyRegion(map);

    expect(drawn(map, new Set([region.id]))).toHaveLength(60);
    // Region-scoped: revealing some other region reveals nothing here.
    expect(drawn(map, new Set(["somewhere-else"]))).toHaveLength(40);
  });

  it("takes the revealed set as an argument and keeps none of it", () => {
    // Purity, asserted the way ADR-0011 relies on it: the projection may not
    // remember the last set it was handed, so drawing the revealed picture
    // between two default ones must leave the default picture byte-identical.
    const map = repo(Array.from({ length: 60 }, (_, i) => [named(i), i] as const));
    const region = onlyRegion(map);
    const positions = (revealed: ReadonlySet<string>) =>
      JSON.stringify(
        fileFlow(map, region, NODE_HEIGHT, undefined, revealed).nodes.map(
          (n) => [n.id, n.position],
        ),
      );

    const before = positions(new Set());
    const all = positions(new Set([region.id]));
    const after = positions(new Set());

    expect(after).toBe(before);
    expect(all).not.toBe(before);
    // And the region it was handed comes back as it went in: a projection
    // that sorted `region.files` in place would reorder every panel reading
    // the same list.
    expect(region.files.map((f) => f.name)).toEqual(
      Array.from({ length: 60 }, (_, i) => named(i)),
    );
  });

  it("still draws a map that publishes no significance, path order deciding", () => {
    // `significance` is optional (ADR-0010), so every map written before it
    // existed arrives here with none. Every file ties; path order is all
    // there is left to break the tie with.
    const map = repo(
      Array.from({ length: 50 }, (_, i) => [named(49 - i), undefined] as const),
    );

    expect(drawn(map).sort()).toEqual(
      Array.from({ length: 40 }, (_, i) => named(i)),
    );
  });

  it("leaves no region of a map without significance empty", () => {
    // The fixtures in this suite predate ADR-0010 and carry none, which is
    // the point: not one of their regions may come out as a blank canvas.
    for (const fixture of [small, tour]) {
      for (const region of [
        ...structuralRegions(fixture),
        ...domainRegions(fixture),
      ]) {
        if (region.files.length === 0) {
          continue;
        }
        const { nodes } = fileFlow(fixture, region);
        expect(nodes.length, `${region.id} drew nothing`).toBeGreaterThan(0);
      }
    }
  });

  describe("what a feature pointing at a file has to reveal", () => {
    /** Two regions of `count` files each, significance rising with the index
     * within each, so both hold the same twenty back. The diff overlay marks
     * files all over a map, which is the case one region cannot pose. */
    function twoRegions(count: number): KnowledgeGraph {
      const file = (layer: string, i: number) => ({
        id: `file:${layer}/${named(i)}`,
        kind: "file" as const,
        name: named(i),
        path: `${layer}/${named(i)}`,
        summary: "",
        layer,
        provenance: "structural" as const,
        significance: i,
      });
      return {
        version: "0.4.0",
        project: { name: "drill" },
        layers: [
          { id: "one", name: "one", provenance: "structural" },
          { id: "two", name: "two", provenance: "structural" },
        ],
        nodes: [
          ...Array.from({ length: count }, (_, i) => file("one", i)),
          ...Array.from({ length: count }, (_, i) => file("two", i)),
        ],
        edges: [],
      };
    }

    const sixty = () =>
      repo(Array.from({ length: 60 }, (_, i) => [named(i), i] as const));

    it("names the region holding a file the default view holds back", () => {
      const map = sixty();

      expect([
        ...regionsHiding([onlyRegion(map)], new Set(["file:app/f000.ts"])),
      ]).toEqual(["app"]);
    });

    it("names nothing for a file the default view already draws", () => {
      // The most significant file in the region. Revealing here would blow a
      // region open for a pointer that already had somewhere to land.
      const map = sixty();

      expect([
        ...regionsHiding([onlyRegion(map)], new Set(["file:app/f059.ts"])),
      ]).toEqual([]);
    });

    it("names nothing for a region the default view draws whole", () => {
      const map = repo(
        Array.from({ length: DRILL_DEFAULT_CARDS }, (_, i) => [named(i), i] as const),
      );

      expect([
        ...regionsHiding([onlyRegion(map)], new Set(["file:app/f000.ts"])),
      ]).toEqual([]);
    });

    it("names every region holding one of the files, and no more", () => {
      // The diff overlay's shape: marks scattered across the map, some on
      // files the default view draws and some on files it does not.
      const regions = structuralRegions(twoRegions(60));

      expect(
        [
          ...regionsHiding(
            regions,
            new Set([
              "file:one/f000.ts", // held back
              "file:two/f001.ts", // held back
              "file:two/f059.ts", // already drawn
            ]),
          ),
        ].sort(),
      ).toEqual(["one", "two"]);
      expect([
        ...regionsHiding(regions, new Set(["file:one/f059.ts"])),
      ]).toEqual([]);
      expect([...regionsHiding(regions, new Set())]).toEqual([]);
    });

    it("answers for a file no region holds without inventing one", () => {
      const regions = structuralRegions(twoRegions(60));

      expect([
        ...regionsHiding(regions, new Set(["file:nowhere/f000.ts"])),
      ]).toEqual([]);
    });

    it("asks exactly the question the canvas answers, file by file", () => {
      // The one-mechanism claim, at the level it can actually be proved: for
      // every file in the region, needing a reveal and not being drawn are the
      // same fact. A second opinion computed here — a different cut, a
      // different tie-break — would part company with the canvas somewhere in
      // these sixty, and a feature would either point at nothing or reveal a
      // region it had no reason to.
      const map = sixty();
      const region = onlyRegion(map);
      const drawn = new Set(fileFlow(map, region).nodes.map((n) => n.id));

      for (const file of region.files) {
        expect(
          [...regionsHiding([region], new Set([file.id]))],
          `${file.path} is ${drawn.has(file.id) ? "drawn" : "held back"}`,
        ).toEqual(drawn.has(file.id) ? [] : ["app"]);
      }
    });
  });
});

// Stories 24 and 25: magnify draws only the focused file and its direct
// neighbours — the files it imports and the files that import it — laid out
// by the drill view's own layering. Seam 3 — the pure projection: which files
// the neighbourhood holds, how they layer, and that the magnified set is an
// argument, never state.
describe("the magnified neighbourhood", () => {
  /** Files across two layers, wired so every rule has a witness: an importer
   * above the hub, an import below it, a second import in another region, a
   * link between two neighbours, a file two hops out, an unrelated pair, a
   * caller related by `calls` only, a self-import, and a file with no edges
   * at all. */
  function atlas(): KnowledgeGraph {
    const file = (path: string) => ({
      id: `file:${path}`,
      kind: "file" as const,
      name: path.split("/").pop() ?? path,
      path,
      summary: "",
      layer: path.split("/")[0] ?? "root",
      provenance: "structural" as const,
    });
    const imports = (from: string, to: string) => ({
      source: `file:${from}`,
      target: `file:${to}`,
      kind: "imports" as const,
      weight: 1,
    });
    return {
      version: "0.4.0",
      project: { name: "magnify" },
      layers: [
        { id: "app", name: "app", provenance: "structural" },
        { id: "lib", name: "lib", provenance: "structural" },
      ],
      nodes: [
        file("app/hub.ts"),
        file("app/above.ts"),
        file("app/below.ts"),
        file("app/far.ts"),
        file("app/apart.ts"),
        file("app/other.ts"),
        file("app/caller.ts"),
        file("app/alone.ts"),
        file("lib/shared.ts"),
      ],
      edges: [
        imports("app/above.ts", "app/hub.ts"),
        imports("app/hub.ts", "app/below.ts"),
        imports("app/hub.ts", "lib/shared.ts"),
        imports("app/above.ts", "app/below.ts"),
        imports("app/below.ts", "app/far.ts"),
        imports("app/apart.ts", "app/other.ts"),
        imports("app/hub.ts", "app/hub.ts"),
        {
          source: "file:app/caller.ts",
          target: "file:app/hub.ts",
          kind: "calls" as const,
          weight: 1,
        },
      ],
    };
  }

  /** The lens drawn for one file, positions keyed by path. */
  function lensOn(map: KnowledgeGraph, focus: string) {
    const flow = magnifyFlow(map, neighbourhoodOf(map, `file:${focus}`));
    return {
      ...flow,
      at: new Map(
        flow.nodes.map((n) => [n.id.replace("file:", ""), n.position]),
      ),
    };
  }

  it("draws the focused file and its direct neighbours, wherever they live, and nothing else", () => {
    // `lib/shared.ts` is in another region: the neighbourhood is the map's,
    // not the open region's. The unrelated pair, the two-hop file, the
    // calls-only caller and the loose file are all left out.
    const { nodes } = lensOn(atlas(), "app/hub.ts");

    expect(nodes.map((n) => n.id).sort()).toEqual([
      "file:app/above.ts",
      "file:app/below.ts",
      "file:app/hub.ts",
      "file:lib/shared.ts",
    ]);
  });

  it("stops at one hop — the next hop is one more magnify", () => {
    // `far.ts` is the neighbour's neighbour, so hub's lens leaves it out;
    // magnifying the neighbour itself is what reaches it.
    const map = atlas();

    expect(neighbourhoodOf(map, "file:app/hub.ts").has("file:app/far.ts")).toBe(
      false,
    );
    expect(
      neighbourhoodOf(map, "file:app/below.ts").has("file:app/far.ts"),
    ).toBe(true);
  });

  it("relates files by imports, not calls", () => {
    // The decision the ticket records: `calls` is a relating kind, but every
    // canvas this dashboard draws relates files by imports alone, and the
    // dim-in-place focus lights exactly those edges — magnify agreeing with
    // them means the two focus modes never name different neighbours.
    const map = atlas();

    expect(
      neighbourhoodOf(map, "file:app/hub.ts").has("file:app/caller.ts"),
    ).toBe(false);
    // And from the caller's side the lens is honest the same way: its one
    // relating edge is a call, so it magnifies alone.
    expect([...neighbourhoodOf(map, "file:app/caller.ts")]).toEqual([
      "file:app/caller.ts",
    ]);
  });

  it("lays the neighbourhood out the way the drill view does: imports run downward", () => {
    const { at } = lensOn(atlas(), "app/hub.ts");

    // What leans on the hub sits above it; what it leans on sits below —
    // both of its imports on the same row.
    expect(at.get("app/above.ts")?.y ?? 0).toBeLessThan(
      at.get("app/hub.ts")?.y ?? 0,
    );
    expect(at.get("app/hub.ts")?.y ?? 0).toBeLessThan(
      at.get("app/below.ts")?.y ?? 0,
    );
    expect(at.get("lib/shared.ts")?.y).toBe(at.get("app/below.ts")?.y);
  });

  it("draws the induced subgraph: a link between two neighbours is drawn too", () => {
    const { edges } = lensOn(atlas(), "app/hub.ts");

    // Four imports run inside the neighbourhood — the fixture's fifth link
    // among these files is the hub's self-import, which is a scribble, not
    // information, on this canvas exactly as on the drill view's.
    expect(
      edges.map((e) => `${e.source} -> ${e.target}`).sort(),
    ).toEqual([
      "file:app/above.ts -> file:app/below.ts",
      "file:app/above.ts -> file:app/hub.ts",
      "file:app/hub.ts -> file:app/below.ts",
      "file:app/hub.ts -> file:lib/shared.ts",
    ]);
  });

  it("magnifies a file with no relating edges to itself alone", () => {
    const { nodes, edges } = lensOn(atlas(), "app/alone.ts");

    expect(nodes.map((n) => n.id)).toEqual(["file:app/alone.ts"]);
    expect(edges).toEqual([]);
  });

  it("draws a neighbour the default drill view holds back", () => {
    // Sixty files, significance rising with the index, and the top file
    // imports the least significant one: the drill view's default cut hides
    // the neighbour, and the lens — whose relevance test is connectivity,
    // not significance — draws it without touching the revealed set, which
    // it does not even take.
    const named = (i: number) => `f${String(i).padStart(3, "0")}.ts`;
    const wide: KnowledgeGraph = {
      version: "0.4.0",
      project: { name: "magnify" },
      layers: [{ id: "app", name: "app", provenance: "structural" }],
      nodes: Array.from({ length: 60 }, (_, i) => ({
        id: `file:app/${named(i)}`,
        kind: "file" as const,
        name: named(i),
        path: `app/${named(i)}`,
        summary: "",
        layer: "app",
        provenance: "structural" as const,
        significance: i,
      })),
      edges: [
        {
          source: "file:app/f059.ts",
          target: "file:app/f000.ts",
          kind: "imports" as const,
          weight: 1,
        },
      ],
    };
    const region = structuralRegions(wide)[0];
    if (region === undefined) {
      throw new Error("fixture has no region");
    }

    const dflt = new Set(fileFlow(wide, region).nodes.map((n) => n.id));
    expect(dflt.has("file:app/f000.ts"), "the fixture must hide it").toBe(
      false,
    );

    const lens = magnifyFlow(wide, neighbourhoodOf(wide, "file:app/f059.ts"));
    expect(lens.nodes.map((n) => n.id).sort()).toEqual([
      "file:app/f000.ts",
      "file:app/f059.ts",
    ]);
  });

  it("takes the magnified set as an argument and keeps none of it", () => {
    // Purity, asserted the way ADR-0011 relies on it: same map, same focused
    // file, byte-identical positions — with another lens drawn in between.
    const map = atlas();
    const hub = neighbourhoodOf(map, "file:app/hub.ts");
    const positions = (set: ReadonlySet<string>) =>
      JSON.stringify(
        magnifyFlow(map, set).nodes.map((n) => [n.id, n.position]),
      );

    const before = positions(hub);
    const other = positions(neighbourhoodOf(map, "file:app/alone.ts"));
    const after = positions(hub);

    expect(after).toBe(before);
    expect(other).not.toBe(before);
  });

  it("draws the same lens however the map orders its edges", () => {
    const map = atlas();
    const reversed: KnowledgeGraph = {
      ...map,
      edges: [...map.edges].reverse(),
    };
    const drawing = (m: KnowledgeGraph) => {
      const flow = magnifyFlow(m, neighbourhoodOf(m, "file:app/hub.ts"));
      return JSON.stringify({
        cards: [...flow.nodes]
          .sort((one, two) => byPath(one.id, two.id))
          .map((n) => [n.id, n.position, n.data.anchors]),
        edges: [...flow.edges]
          .sort((one, two) => byPath(one.id, two.id))
          .map((e) => [e.id, e.sourceHandle, e.targetHandle]),
      });
    };

    expect(drawing(map)).toBe(drawing(map));
    expect(drawing(reversed)).toBe(drawing(map));
  });
});

// Story 5 (ADR-0011): every edge touching a card lands at its own point along
// that card's edge, so a well-connected file reads as a fan rather than as one
// smear that says nothing about which file connects to which. Seam 3 — the
// pure projection, asserted as geometry.
describe("a fan rather than a knot", () => {
  /** A one-region map: `hub.ts` importing `spokes` files, each named so path
   * order and index order agree. Every one of those edges leaves the same
   * card, which is exactly the case that used to converge on one pixel. */
  function hubAndSpokes(spokes: number): KnowledgeGraph {
    const names = ["hub.ts", ...Array.from({ length: spokes }, (_, i) => spoke(i))];
    return {
      version: "0.4.0",
      project: { name: "fan" },
      layers: [{ id: "app", name: "app", provenance: "structural" }],
      nodes: names.map((name) => ({
        id: `file:app/${name}`,
        kind: "file" as const,
        name,
        path: `app/${name}`,
        summary: "",
        layer: "app",
        provenance: "structural" as const,
      })),
      edges: Array.from({ length: spokes }, (_, i) => ({
        source: "file:app/hub.ts",
        target: `file:app/${spoke(i)}`,
        kind: "imports" as const,
        weight: 1,
      })),
    };
  }

  /** The same shape, but with the three spokes listed — and so drawn — in an
   * order that is neither the order the map lists its edges in nor the order
   * their IDs sort in. */
  function threeDrawnOutOfOrder(): KnowledgeGraph {
    const map = hubAndSpokes(0);
    const file = (name: string) => ({
      id: `file:app/${name}`,
      kind: "file" as const,
      name,
      path: `app/${name}`,
      summary: "",
      layer: "app",
      provenance: "structural" as const,
    });
    return {
      ...map,
      nodes: [...map.nodes, ...["b.ts", "c.ts", "a.ts"].map(file)],
      edges: ["a.ts", "b.ts", "c.ts"].map((name) => ({
        source: "file:app/hub.ts",
        target: `file:app/${name}`,
        kind: "imports" as const,
        weight: 1,
      })),
    };
  }

  const spoke = (i: number) => `s${String(i).padStart(3, "0")}.ts`;
  const HUB = "file:app/hub.ts";

  function onlyRegion(map: KnowledgeGraph) {
    const region = structuralRegions(map)[0];
    if (region === undefined) {
      throw new Error("fixture has no region");
    }
    return region;
  }

  /** The card the projection drew for `id`. */
  function cardOf(nodes: readonly AppFlowNode[], id: string) {
    const card = nodes.find((n) => n.id === id);
    if (card === undefined) {
      throw new Error(`${id} was not drawn`);
    }
    return card;
  }

  /** Where an edge meets a card, resolved the way React Flow resolves it:
   * the handle the edge names, looked up among the points that card exposes.
   * Returned in canvas coordinates so two edges landing on the same pixel is
   * a fact about the drawing, not about handle bookkeeping. */
  function landing(
    nodes: readonly AppFlowNode[],
    cardId: string,
    handleId: string | null | undefined,
  ): { x: number; y: number } {
    const card = cardOf(nodes, cardId);
    const anchor = (card.data.anchors ?? []).find((a) => a.id === handleId);
    if (anchor === undefined) {
      throw new Error(
        `${cardId} exposes no point named ${String(handleId)}; it has ` +
          `${(card.data.anchors ?? []).map((a) => a.id).join(", ") || "none"}`,
      );
    }
    return {
      x: card.position.x + anchor.x,
      y: card.position.y + (anchor.type === "source" ? (card.height ?? 0) : 0),
    };
  }

  /** Every point an edge leaves the hub at, in the order the edges are drawn. */
  function leavingHub(flow: { nodes: AppFlowNode[]; edges: FlowEdge[] }) {
    return flow.edges
      .filter((e) => e.source === HUB)
      .map((e) => landing(flow.nodes, HUB, e.sourceHandle));
  }

  /** How far along the hub's own edge each line leaves, read left to right by
   * where the line arrives — the fan as the reader sees it. */
  function fanAcrossHub(flow: { nodes: AppFlowNode[]; edges: FlowEdge[] }) {
    const hub = cardOf(flow.nodes, HUB);
    return flow.edges
      .filter((e) => e.source === HUB)
      .map((e) => ({
        leaves: landing(flow.nodes, HUB, e.sourceHandle).x - hub.position.x,
        arrives: landing(flow.nodes, e.target, e.targetHandle).x,
      }))
      .sort((one, two) => one.arrives - two.arrives)
      .map((run) => run.leaves);
  }

  it("gives every edge leaving one card its own point", () => {
    const map = hubAndSpokes(6);
    const flow = fileFlow(map, onlyRegion(map));

    const points = leavingHub(flow);

    expect(points).toHaveLength(6);
    expect(new Set(points.map((p) => p.x)).size).toBe(6);
  });

  it("ranks the points by where the other card is drawn, so the fan does not cross", () => {
    // The fixture is built to tell the rules apart: the hub's three targets
    // are drawn b, c, a left to right, while both the order the map lists its
    // edges in and the order their IDs sort in say a, b, c. A rule reading
    // either list would send the leftmost line to the rightmost card, which
    // is lines that start apart and then cross — not a fan.
    const map = threeDrawnOutOfOrder();
    const flow = fileFlow(map, onlyRegion(map));

    const spokes = flow.nodes
      .filter((n) => n.id !== HUB)
      .sort((one, two) => one.position.x - two.position.x)
      .map((n) => n.id.replace("file:app/", ""));
    expect(spokes, "the fixture must draw them out of listed order").toEqual([
      "b.ts",
      "c.ts",
      "a.ts",
    ]);

    const runs = flow.edges
      .map((e) => ({
        leaves: landing(flow.nodes, HUB, e.sourceHandle).x,
        arrives: landing(flow.nodes, e.target, e.targetHandle).x,
      }))
      .sort((one, two) => one.arrives - two.arrives);

    // The line reaching furthest left leaves furthest left, all the way
    // across: that, and not merely starting apart, is what reads as a fan.
    for (let i = 1; i < runs.length; i += 1) {
      expect(
        runs[i]?.leaves ?? 0,
        `the line to the card at ${runs[i]?.arrives} leaves left of the one at ${runs[i - 1]?.arrives}`,
      ).toBeGreaterThan(runs[i - 1]?.leaves ?? 0);
    }
  });

  it("repeats points in ascending runs once the card is full, never off it", () => {
    // Twenty-four edges on a two-hundred-pixel card. Fourteen pixels stay
    // clear at each corner and no two points sit closer than twelve, so the
    // card holds fifteen; the twenty-four ranks are scaled onto those
    // fifteen. Every extra edge therefore shares a point with the neighbour
    // beside it in the fan — the degradation is repetition in order, not an
    // anchor drawn past the card's corner and not a collapse to the centre.
    const map = hubAndSpokes(24);
    const offsets = fanAcrossHub(fileFlow(map, onlyRegion(map)));

    expect(offsets).toHaveLength(24);
    expect(new Set(offsets).size).toBe(15);

    for (const x of offsets) {
      expect(x, "an anchor was drawn off the card").toBeGreaterThanOrEqual(14);
      expect(x).toBeLessThanOrEqual(NODE_WIDTH - 14);
    }
    for (let i = 1; i < offsets.length; i += 1) {
      expect(
        offsets[i] ?? 0,
        "the fan went backwards, so two of its lines cross",
      ).toBeGreaterThanOrEqual(offsets[i - 1] ?? 0);
    }
    const distinct = [...new Set(offsets)].sort((one, two) => one - two);
    for (let i = 1; i < distinct.length; i += 1) {
      expect(
        (distinct[i] ?? 0) - (distinct[i - 1] ?? 0),
        "two points sit closer than a reader can tell apart",
      ).toBeGreaterThanOrEqual(12);
    }
  });

  it("draws the same anchors however the map orders its edges", () => {
    // Determinism, at the place it usually dies in a change like this: an
    // anchor decided by the order a list happened to arrive in, or by
    // iteration over a hash map, moves when the producer emits the same
    // relationships in another order. Reversing the map's edge list is the
    // probe — same map, same state, byte-identical positions and anchors.
    const map = hubAndSpokes(9);
    const reversed: KnowledgeGraph = { ...map, edges: [...map.edges].reverse() };
    const drawing = (m: KnowledgeGraph) => {
      const flow = fileFlow(m, onlyRegion(m));
      return JSON.stringify({
        cards: [...flow.nodes]
          .sort((one, two) => byPath(one.id, two.id))
          .map((n) => [n.id, n.position, n.data.anchors]),
        edges: [...flow.edges]
          .sort((one, two) => byPath(one.id, two.id))
          .map((e) => [e.id, e.sourceHandle, e.targetHandle]),
      });
    };

    expect(drawing(map)).toBe(drawing(map));
    expect(drawing(reversed)).toBe(drawing(map));
  });

  it("spreads the overview's region cards, not only the drill view's", () => {
    // Both views draw cards, and the overview's are the ones a reader meets
    // first: a region every other region leans on knots exactly the same way.
    const map: KnowledgeGraph = {
      version: "0.4.0",
      project: { name: "overview" },
      layers: ["r0", "r1", "r2", "r3"].map((id) => ({
        id,
        name: id,
        provenance: "structural" as const,
      })),
      nodes: ["r0", "r1", "r2", "r3"].map((layer) => ({
        id: `file:${layer}/a.ts`,
        kind: "file" as const,
        name: "a.ts",
        path: `${layer}/a.ts`,
        summary: "",
        layer,
        provenance: "structural" as const,
      })),
      edges: [],
    };
    const flow = regionFlow(
      structuralRegions(map),
      ["r1", "r2", "r3"].map((target) => ({
        source: "r0",
        target,
        count: 1,
        label: "1 import",
      })),
    );

    const points = flow.edges.map(
      (e) => landing(flow.nodes, regionNodeId("r0"), e.sourceHandle).x,
    );

    expect(points).toHaveLength(3);
    expect(new Set(points).size).toBe(3);
  });
});

describe("card captions and named edges", () => {
  const file = (name: string) =>
    tour.nodes.find((n) => n.kind === "file" && n.name === name)!;

  it("gives a file card the map's own summary of it", () => {
    // Not a second opinion computed here: whatever the map says, including
    // the prose enrichment puts there, so the caption improves with the map.
    const node = file("main.ts");
    expect(captionOf(node)).toBe(node.summary);
    expect(captionOf({ ...node, summary: "" })).toBeNull();
  });

  it("reads a region by how the other regions lean on it", () => {
    const regions = structuralRegions(tour);
    const links = regionLinks(tour, regions);
    const byId = new Map(regions.map((r) => [r.id, r]));
    // cli imports src and nothing imports cli.
    expect(regionCaptionOf(byId.get("src")!, links)).toMatch(/foundation/i);
    expect(regionCaptionOf(byId.get("cli")!, links)).toMatch(/way in/i);
    // A region nothing links to either way says so, rather than saying zero.
    expect(regionCaptionOf(byId.get("src")!, [])).toBe("Keeps to itself");
  });

  it("agrees a region caption's verb with its count", () => {
    // "1 region lean on it" reads as a bug in the map rather than the prose.
    const region = structuralRegions(tour)[0]!;
    const link = (source: string) => ({
      source,
      target: region.id,
      count: 1,
      label: "1 import",
    });
    expect(regionCaptionOf(region, [link("a")])).toContain("1 region leans on");
    expect(regionCaptionOf(region, [link("a"), link("b")])).toContain(
      "2 regions lean on",
    );
  });

  it("names a focused edge by direction, so the arrow needs no decoding", () => {
    expect(edgeLabelOf(true)).toBe("uses");
    expect(edgeLabelOf(false)).toBe("used by");
  });
});

describe("the plain-words account of a node", () => {
  const byId = new Map(tour.nodes.map((n) => [n.id, n]));
  const say = (id: string) =>
    narrativeOf(tour, byId.get(id)!, byId).join(" ");

  it("names what a file holds, rather than counting it", () => {
    // A name is something a reader can go and look at; a number is something
    // they have to go and find out.
    const said = say("file:src/util.ts");
    expect(said).toMatch(/holds \d+ definitions?:/i);
    expect(said).toContain("greet");
  });

  it("names who reaches it and what it reaches, in both directions", () => {
    const said = say("file:src/main.ts");
    expect(said).toMatch(/reached by|way in/i);
    expect(said).toMatch(/it reaches/i);
  });

  it("says a way in is a way in, instead of leaving the reader to infer it", () => {
    // This is the sentence that replaces "Entry point — fan-in 0, fan-out 8".
    expect(say("file:cli/run.ts")).toMatch(/nothing in this map reaches it/i);
  });

  it("caps the names it lists and counts the rest", () => {
    const many: KnowledgeGraph = {
      ...tour,
      edges: [
        ...tour.edges,
        ...["a", "b", "c", "d", "e"].map((n) => ({
          source: `file:${n}.ts`,
          target: "file:src/util.ts",
          kind: "imports" as const,
          weight: 1,
        })),
      ],
      nodes: [
        ...tour.nodes,
        ...["a", "b", "c", "d", "e"].map((n) => ({
          id: `file:${n}.ts`,
          kind: "file" as const,
          name: `${n}.ts`,
          path: `${n}.ts`,
          summary: "",
          provenance: "structural" as const,
        })),
      ],
    };
    const wide = new Map(many.nodes.map((n) => [n.id, n]));
    const said = narrativeOf(many, wide.get("file:src/util.ts")!, wide).join(" ");
    expect(said).toMatch(/and \d+ more/);
    // One conjunction per list: "a, b and c, and 4 more" trips the reader.
    expect(said).not.toMatch(/\w and \w[^,]*, and \d+ more/);
  });

  it("offers enrichment only where the prose is still mechanical", () => {
    const node = byId.get("file:src/main.ts")!;
    expect(enrichmentHint({ ...node, provenance: "structural" })).toContain(
      "--enrich",
    );
    expect(enrichmentHint({ ...node, provenance: "llm" })).toBeNull();
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
