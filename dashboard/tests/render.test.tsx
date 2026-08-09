// Seam 1 (map contract): feed a graph file in, assert what is rendered.
// No pipeline or component internals are touched.
import { render, screen, waitFor } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";
import smallMap from "./fixtures/small-map.json";
import oldMap from "../../crates/codeatlas/tests/fixtures/maps/known-good.json";

describe("rendering a map file", () => {
  it("renders every node of the fixture map on the canvas", () => {
    render(<MapExplorer map={smallMap as KnowledgeGraph} />);

    for (const name of ["main.ts", "main", "greeter.ts", "Greeter", "guide.md"]) {
      expect(
        screen.getAllByText(name, { selector: ".react-flow__node *" }).length,
      ).toBeGreaterThan(0);
    }
  });

  it("renders edges between the fixture nodes", async () => {
    const { container } = render(<MapExplorer map={smallMap as KnowledgeGraph} />);

    await waitFor(() => {
      expect(container.querySelectorAll(".react-flow__edge").length).toBe(3);
    });
  });

  it("groups nodes by layer, showing each layer's display name", () => {
    render(<MapExplorer map={smallMap as KnowledgeGraph} />);

    // Layer display names (one of them enriched) are visible as group labels.
    expect(
      screen.getByText("Source code", { selector: ".layer-label" }),
    ).toBeInTheDocument();
    expect(
      screen.getByText("docs", { selector: ".layer-label" }),
    ).toBeInTheDocument();

    // Every layer in the map becomes a group container on the canvas.
    const groups = document.querySelectorAll('[data-testid^="layer-group-"]');
    expect(groups.length).toBe(2);
  });

  it("renders an older minimal map (no layers) without errors", () => {
    render(<MapExplorer map={oldMap as KnowledgeGraph} />);

    expect(
      screen.getAllByText("Greeter", { selector: ".react-flow__node *" }).length,
    ).toBeGreaterThan(0);
  });
});
