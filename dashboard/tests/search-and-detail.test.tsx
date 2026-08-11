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

describe("dismissing the search results", () => {
  it("closes on a click outside, without discarding the query", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await user.type(screen.getByLabelText("Search nodes"), "main");
    expect(screen.getByLabelText("Search results")).toBeInTheDocument();

    await user.click(screen.getByRole("heading", { level: 1 }));
    expect(screen.queryByLabelText("Search results")).not.toBeInTheDocument();
    // The reader dismissed the results, not their search.
    expect(screen.getByLabelText("Search nodes")).toHaveValue("main");
  });

  it("reopens when the reader goes back to the input", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await user.type(screen.getByLabelText("Search nodes"), "main");
    await user.click(screen.getByRole("heading", { level: 1 }));
    await user.click(screen.getByLabelText("Search nodes"));

    expect(screen.getByLabelText("Search results")).toBeInTheDocument();
  });

  it("closes on Escape and leaves focus on the input", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await user.type(screen.getByLabelText("Search nodes"), "main");
    await user.keyboard("{Escape}");

    expect(screen.queryByLabelText("Search results")).not.toBeInTheDocument();
    expect(screen.getByLabelText("Search nodes")).toHaveFocus();
  });

  it("does not swallow the click that dismissed it", async () => {
    // The usual bug: a document-level mousedown closes the overlay before
    // React's click reaches the element under the pointer, so the reader has
    // to click twice and the first click appears to do nothing.
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await user.type(screen.getByLabelText("Search nodes"), "greeter");
    const chips = screen
      .getAllByRole("button")
      .filter((b) => b.classList.contains("region-chip"));
    await user.click(chips[0]!);

    expect(screen.queryByLabelText("Search results")).not.toBeInTheDocument();
    expect(chips[0]).toHaveClass("region-chip-on");
  });

  it("closes when a result is chosen", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await user.type(screen.getByLabelText("Search nodes"), "main");
    await user.click(
      within(screen.getByLabelText("Search results")).getByText("main"),
    );

    expect(screen.queryByLabelText("Search results")).not.toBeInTheDocument();
  });
});
