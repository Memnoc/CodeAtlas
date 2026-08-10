// AC: the dashboard renders CodeAtlas's own self-scan map. The map is
// produced fresh by the real binary, then fed through the same seam as any
// other map file.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeAll, describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";

const repoRoot = path.resolve(import.meta.dirname, "..", "..");
let map: KnowledgeGraph;

describe("CodeAtlas's own self-scan map", () => {
  beforeAll(() => {
    execFileSync("cargo", ["run", "-q", "-p", "codeatlas", "--", "scan", "."], {
      cwd: repoRoot,
      stdio: "pipe",
      timeout: 300_000,
    });
    map = JSON.parse(
      readFileSync(
        path.join(repoRoot, ".codeatlas", "knowledge-graph.json"),
        "utf8",
      ),
    ) as KnowledgeGraph;
  }, 320_000);

  it("renders every node and layer without errors", () => {
    render(<MapExplorer map={map} />);

    // Every node in the emitted map is on the canvas.
    const rendered = document.querySelectorAll(
      ".react-flow__node .entity",
    ).length;
    expect(rendered).toBe(map.nodes.length);
    expect(map.nodes.length).toBeGreaterThan(0);

    // Every emitted layer became a visible group.
    const groups = document.querySelectorAll('[data-testid^="layer-group-"]');
    expect(groups.length).toBe(map.layers?.length ?? 0);
    expect(groups.length).toBeGreaterThan(0);

    // A node this repo is guaranteed to contain.
    expect(
      screen.getAllByText("main.rs", { selector: ".react-flow__node *" })
        .length,
    ).toBeGreaterThan(0);
  });

  it("walks a bounded, curated tour of the architecture, not an inventory", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    const byId = new Map(map.nodes.map((n) => [n.id, n]));
    const files = map.nodes.filter((n) => n.kind === "file");
    const tour = map.tour ?? [];
    expect(tour.length).toBeGreaterThan(0);
    // One step per file is an enumeration, not a guided tour: this repo has
    // ~150 files (including every lockfile and test fixture) and the walk
    // stays a newcomer's sitting. The exact bound is `TOUR_MAX_STEPS`,
    // pinned on the Rust side (crates/codeatlas/tests/scan.rs); 20 is the
    // ceiling past which no bound could still be called newcomer-sized.
    expect(files.length).toBeGreaterThan(50);
    expect(tour.length).toBeLessThanOrEqual(20);

    // Every stop is wired into the architecture: something imports it, it
    // imports something, or a call chain starts in it.
    const degree = new Map<string, number>();
    for (const edge of map.edges) {
      if (edge.kind === "imports") {
        degree.set(edge.source, (degree.get(edge.source) ?? 0) + 1);
        degree.set(edge.target, (degree.get(edge.target) ?? 0) + 1);
      }
    }
    const callers = new Set(
      map.edges.filter((e) => e.kind === "calls").map((e) => e.source),
    );
    const called = new Set(
      map.edges.filter((e) => e.kind === "calls").map((e) => e.target),
    );
    const entryPointPaths = new Set(
      map.nodes
        .filter(
          (n) =>
            n.kind === "function" && callers.has(n.id) && !called.has(n.id),
        )
        .map((n) => n.path),
    );
    for (const step of tour) {
      const node = byId.get(step.node);
      expect(node, `dangling tour step ${step.node}`).toBeDefined();
      if (!node) {
        continue;
      }
      const significance =
        (degree.get(node.id) ?? 0) + (entryPointPaths.has(node.path) ? 1 : 0);
      expect(significance, `isolated file on the tour: ${node.path}`).
        toBeGreaterThan(0);
    }

    // The regression ticket 16 exists for: an integration-test file with
    // fan-in 0 and fan-out 0 used to open the walk, with the crate root
    // nine steps behind it.
    const paths = tour.map((step) => byId.get(step.node)?.path);
    expect(paths).not.toContain("crates/codeatlas/tests/scan.rs");
    expect(paths).toContain("crates/codeatlas/src/lib.rs");

    // And the walk is reachable: the newcomer starts it from the sidebar.
    const panel = within(screen.getByLabelText("Guided tour"));
    await user.click(panel.getByRole("button", { name: /start tour/i }));
    expect(panel.getByText(`Step 1 of ${tour.length}`)).toBeInTheDocument();
    const first = paths[0];
    expect(first).toBeDefined();
    expect(screen.getByLabelText("Node detail")).toHaveTextContent(
      first ?? "",
    );
  });

  it("resolves the NodeNext import specifiers the dashboard is written with", () => {
    const byId = new Map(map.nodes.map((n) => [n.id, n]));
    const imports = map.edges
      .filter((e) => e.kind === "imports")
      .map(
        (e) =>
          [byId.get(e.source)?.path, byId.get(e.target)?.path] as const,
      );

    // TypeScript under NodeNext obliges this source to say "./graph.js"
    // for a file that is graph.ts.
    expect(imports).toContainEqual([
      "dashboard/src/app/MapExplorer.tsx",
      "dashboard/src/app/graph.ts",
    ]);

    // Throughout, not merely somewhere. While these specifiers went
    // unresolved the whole subtree was islands: eleven files in src/app
    // sharing one edge between them.
    const connected = new Set(imports.flat());
    const orphans = map.nodes
      .filter((n) => n.kind === "file" && n.path.startsWith("dashboard/src/"))
      .map((n) => n.path)
      .filter((path) => !connected.has(path));
    expect(orphans).toEqual([]);

    // And the consequence that made it visible: the tour ranks by import
    // degree, so an edgeless dashboard could never appear on the walk. It
    // held none of the twelve stops.
    const tourPaths = (map.tour ?? []).map((step) => byId.get(step.node)?.path);
    expect(
      tourPaths.filter((path) => path?.startsWith("dashboard/")),
    ).not.toHaveLength(0);
  });

  it("resolves the crate-name paths this repository's Rust is written with", () => {
    const byId = new Map(map.nodes.map((n) => [n.id, n]));
    const imports = map.edges
      .filter((e) => e.kind === "imports")
      .map(
        (e) => [byId.get(e.source)?.path, byId.get(e.target)?.path] as const,
      );

    // An integration test cannot say `crate::` — that path does not span the
    // tests/ boundary — so it names its own crate, and the map has to follow
    // it or every tests/ file in every Rust project is an orphan. Asserted by
    // name: a count would pass on any two edges.
    expect(imports).toContainEqual([
      "crates/codeatlas/tests/share.rs",
      "crates/codeatlas/src/share.rs",
    ]);
    expect(imports).toContainEqual([
      "crates/codeatlas/tests/share.rs",
      "crates/codeatlas/src/map.rs",
    ]);

    // And the other half of the guarantee: a crate that is not in the scanned
    // tree must still resolve to nothing. lib.rs imports std, anyhow, serde
    // and clap alongside its own modules, and a wrongly resolved external
    // path would land on some real file rather than on one named after the
    // crate — so the check is where the edges point, not what they are called.
    const fromLib = imports
      .filter(([source]) => source === "crates/codeatlas/src/lib.rs")
      .map(([, target]) => target ?? "");
    expect(fromLib.length).toBeGreaterThan(0);
    expect(
      fromLib.filter((t) => !t.startsWith("crates/codeatlas/src/")),
    ).toEqual([]);
  });

  it("groups the self-scan's domain flows by domain", () => {
    render(<MapExplorer map={map} />);

    const flows = map.domain_flows ?? [];
    expect(flows.length).toBeGreaterThan(0);
    const panel = within(screen.getByLabelText("Domain flows"));
    for (const domain of new Set(flows.map((f) => f.domain))) {
      expect(
        panel.getByRole("heading", { name: new RegExp(`^${domain}`) }),
      ).toBeInTheDocument();
    }
  });

  it("shows detail for a real node from the self-scan", () => {
    render(<MapExplorer map={map} />);

    const anyNode = map.nodes[0];
    expect(anyNode).toBeDefined();
    if (!anyNode) {
      return;
    }
    // Selecting via the canvas node click path.
    const el = document.querySelector(
      `[data-id="${CSS.escape(anyNode.id)}"]`,
    ) as HTMLElement | null;
    expect(el).not.toBeNull();
    if (el) {
      fireEvent.click(el);
    }

    expect(screen.getByLabelText("Node detail")).toHaveTextContent(
      anyNode.summary,
    );
  });
});
