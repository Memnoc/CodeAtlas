// Seam 1 (map contract): feed a graph file in, assert what is rendered.
// No pipeline or component internals are touched.
//
// The overview draws regions, not files — a repository has hundreds of files
// and a handful of regions, and only the second is a picture. The files are
// one drill-in away, which is what these tests walk.
import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";
import { openRegion } from "./drive.js";
import smallMap from "./fixtures/small-map.json";
import oldMap from "../../crates/codeatlas/tests/fixtures/maps/known-good.json";

const map = smallMap as KnowledgeGraph;

describe("rendering a map file", () => {
  it("draws one card per region, named and counted", () => {
    render(<MapExplorer map={map} />);

    // The fixture's two layers, one of them enriched ("Source code" for src).
    const source = screen.getByTestId("region-src");
    expect(within(source).getByText("Source code")).toBeInTheDocument();
    expect(within(source).getByText("2 files")).toBeInTheDocument();

    const docs = screen.getByTestId("region-docs");
    expect(within(docs).getByText("docs")).toBeInTheDocument();
    expect(within(docs).getByText("1 file")).toBeInTheDocument();

    // The description is mechanical, derived the way the CLI derives it.
    expect(within(docs).getByText("Files under docs/")).toBeInTheDocument();
  });

  it("draws no file on the overview, however many the map has", () => {
    render(<MapExplorer map={map} />);

    for (const name of ["main.ts", "greeter.ts", "guide.md"]) {
      expect(
        screen.queryAllByText(name, { selector: ".react-flow__node *" }),
      ).toHaveLength(0);
    }
    expect(document.querySelectorAll(".react-flow__node")).toHaveLength(2);
  });

  it("draws the region's files once one is opened", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await openRegion(user, "Source code");

    for (const name of ["main.ts", "greeter.ts"]) {
      expect(
        screen.getAllByText(name, { selector: ".react-flow__node *" }).length,
      ).toBeGreaterThan(0);
    }
    // Only that region's files: the other layer's stay behind.
    expect(
      screen.queryAllByText("guide.md", { selector: ".react-flow__node *" }),
    ).toHaveLength(0);
  });

  it("returns to the overview through the breadcrumb", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await openRegion(user, "Source code");
    await user.click(screen.getByRole("button", { name: "Project overview" }));

    expect(screen.getByTestId("region-src")).toBeInTheDocument();
    expect(
      screen.queryAllByText("main.ts", { selector: ".react-flow__node *" }),
    ).toHaveLength(0);
  });

  it("labels the counted link between two regions", async () => {
    const { container } = render(<MapExplorer map={map} />);

    // docs/guide.md is unlinked; src imports within itself, so the fixture's
    // only cross-region traffic is none — the canvas says so by drawing no
    // edge rather than by drawing an unlabelled one.
    await waitFor(() => {
      expect(container.querySelectorAll(".react-flow__edge")).toHaveLength(0);
    });
  });

  it("renders an older minimal map (no layers) without errors", () => {
    render(<MapExplorer map={oldMap as KnowledgeGraph} />);

    // No `layers` field at all: every file falls into the implicit root
    // region rather than vanishing.
    expect(screen.getByTestId("region-root")).toBeInTheDocument();
  });
});

describe("a panel with nothing to rank", () => {
  /** A one-file map whose only file either scored zero or was never scored:
   * `significance` is optional in the contract (ADR-0010), so both maps are
   * legal and they are not the same fact. */
  function quiet(significance: number | undefined): KnowledgeGraph {
    return {
      version: "0.4.0",
      project: { name: "quiet" },
      layers: [{ id: "src", name: "src", provenance: "structural" }],
      nodes: [
        {
          id: "file:src/lonely.ts",
          kind: "file",
          name: "lonely.ts",
          path: "src/lonely.ts",
          summary: "",
          layer: "src",
          provenance: "structural",
          ...(significance === undefined ? {} : { significance }),
        },
      ],
      edges: [],
    };
  }

  it("reports the measurement when every file was measured at zero", () => {
    render(<MapExplorer map={quiet(0)} />);

    expect(
      within(screen.getByLabelText("Files that matter")).getByText(
        "No file carries a significance above zero.",
      ),
    ).toBeInTheDocument();
  });

  it("says nothing was measured when the map publishes no significance", () => {
    render(<MapExplorer map={quiet(undefined)} />);

    // The panel may not report a score of zero for a file the map never
    // scored: an empty ranking here means the number is absent, not low.
    const section = within(screen.getByLabelText("Files that matter"));
    expect(
      section.getByText("This map publishes no significance to rank on."),
    ).toBeInTheDocument();
    expect(section.queryByText(/above zero/)).toBeNull();
  });
});
