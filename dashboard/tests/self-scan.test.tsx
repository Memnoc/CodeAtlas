// AC: the dashboard renders CodeAtlas's own self-scan map. The map is
// produced fresh by the real binary, then fed through the same seam as any
// other map file.
import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import path from "node:path";
import { fireEvent, render, screen } from "@testing-library/react";
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
