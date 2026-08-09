// Seam 1 (map contract): feed a graph file in, drive the UI as a user
// would, assert what is rendered.
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";
import smallMap from "./fixtures/small-map.json";

const map = smallMap as KnowledgeGraph;

describe("search", () => {
  it("finds nodes by name and narrows as the query grows", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await user.type(screen.getByLabelText("Search nodes"), "main");
    let results = within(screen.getByLabelText("Search results"));
    // "main" matches main.ts, the main function — and guide.md does not.
    expect(results.getAllByRole("button").length).toBe(2);
    expect(results.queryByText("guide.md")).not.toBeInTheDocument();

    await user.clear(screen.getByLabelText("Search nodes"));
    await user.type(screen.getByLabelText("Search nodes"), "greeter");
    results = within(screen.getByLabelText("Search results"));
    // Both greeter.ts and the Greeter class match — by path for the class.
    expect(results.getAllByRole("button").length).toBe(2);
  });

  it("finds nodes by path", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await user.type(screen.getByLabelText("Search nodes"), "docs/");
    const results = within(screen.getByLabelText("Search results"));
    expect(results.getByText("guide.md")).toBeInTheDocument();
    expect(results.getAllByRole("button").length).toBe(1);
  });

  it("says so when nothing matches", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await user.type(screen.getByLabelText("Search nodes"), "zzz-nothing");
    expect(screen.getByText("No matches")).toBeInTheDocument();
  });
});

describe("node detail", () => {
  it("shows summary, edges, line range, and provenance badge for a selected node", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await user.type(screen.getByLabelText("Search nodes"), "main");
    await user.click(
      within(screen.getByLabelText("Search results")).getByText("main"),
    );

    const detail = within(screen.getByLabelText("Node detail"));
    expect(detail.getByRole("heading", { name: "main" })).toBeInTheDocument();
    expect(
      detail.getByText("Entry point that greets the user."),
    ).toBeInTheDocument();
    expect(detail.getByText("lines 3–9")).toBeInTheDocument();
    expect(detail.getByTestId("provenance-badge")).toHaveTextContent("llm");
    // Its one edge: contained by main.ts.
    const edges = within(detail.getByLabelText("Edges"));
    expect(edges.getByText("← contains main.ts")).toBeInTheDocument();
  });

  it("badges structural provenance and lists outgoing edges", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await user.type(screen.getByLabelText("Search nodes"), "main.ts");
    await user.click(
      within(screen.getByLabelText("Search results")).getByText("main.ts"),
    );

    const detail = within(screen.getByLabelText("Node detail"));
    expect(detail.getByTestId("provenance-badge")).toHaveTextContent(
      "structural",
    );
    const edges = within(detail.getByLabelText("Edges"));
    expect(edges.getByText("contains → main")).toBeInTheDocument();
    expect(edges.getByText("imports → greeter.ts")).toBeInTheDocument();
  });

  it("navigates to a neighboring node through its edge entry", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await user.type(screen.getByLabelText("Search nodes"), "main.ts");
    await user.click(
      within(screen.getByLabelText("Search results")).getByText("main.ts"),
    );
    await user.click(
      within(screen.getByLabelText("Node detail")).getByText(
        "imports → greeter.ts",
      ),
    );

    const detail = within(screen.getByLabelText("Node detail"));
    expect(
      detail.getByRole("heading", { name: "greeter.ts" }),
    ).toBeInTheDocument();
  });
});
