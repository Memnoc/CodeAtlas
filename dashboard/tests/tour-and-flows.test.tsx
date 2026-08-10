// Seam 1 (map contract): feed a graph file in, drive the tour and the
// domain-flow affordances as a newcomer would, assert what is rendered and
// what the canvas selects. No component internals are touched.
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";
import tourMap from "./fixtures/tour-map.json";
import oldMap from "../../crates/codeatlas/tests/fixtures/maps/known-good.json";

const map = tourMap as KnowledgeGraph;

/** The node the canvas currently has selected, as React Flow marks it —
 * and never more than one, whoever made the selection. */
function selectedOnCanvas(): string | null {
  const selected = document.querySelectorAll(".react-flow__node.selected");
  expect(selected.length).toBeLessThanOrEqual(1);
  return selected[0]?.getAttribute("data-id") ?? null;
}

describe("guided tour", () => {
  it("walks the tour step by step, moving the canvas selection", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    const tour = within(screen.getByLabelText("Guided tour"));
    // Nothing is selected until the newcomer starts walking.
    expect(selectedOnCanvas()).toBeNull();

    await user.click(tour.getByRole("button", { name: /start tour/i }));
    expect(tour.getByText("Step 1 of 3")).toBeInTheDocument();
    expect(tour.getByText("Start here: the CLI entry point.")).toBeVisible();
    expect(selectedOnCanvas()).toBe("file:cli/run.ts");

    await user.click(tour.getByRole("button", { name: "Next" }));
    expect(tour.getByText("Step 2 of 3")).toBeInTheDocument();
    expect(
      tour.getByText("Entry point: src/main.ts — fan-in 1, fan-out 1"),
    ).toBeVisible();
    expect(selectedOnCanvas()).toBe("file:src/main.ts");
    // The step's node is the one the detail panel describes.
    expect(
      within(screen.getByLabelText("Node detail")).getByRole("heading", {
        name: "main.ts",
      }),
    ).toBeInTheDocument();

    await user.click(tour.getByRole("button", { name: "Previous" }));
    expect(tour.getByText("Step 1 of 3")).toBeInTheDocument();
    expect(selectedOnCanvas()).toBe("file:cli/run.ts");
  });

  it("stops at both ends of the walk", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    const tour = within(screen.getByLabelText("Guided tour"));
    await user.click(tour.getByRole("button", { name: /start tour/i }));
    expect(tour.getByRole("button", { name: "Previous" })).toBeDisabled();

    await user.click(tour.getByRole("button", { name: "Next" }));
    await user.click(tour.getByRole("button", { name: "Next" }));
    expect(tour.getByText("Step 3 of 3")).toBeInTheDocument();
    expect(tour.getByRole("button", { name: "Next" })).toBeDisabled();
  });

  it("badges each label's provenance, mechanical and enriched alike", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    const tour = within(screen.getByLabelText("Guided tour"));
    await user.click(tour.getByRole("button", { name: /start tour/i }));
    // Step 1 is narrated by enrichment …
    expect(tour.getByTestId("provenance-badge")).toHaveTextContent("llm");

    await user.click(tour.getByRole("button", { name: "Next" }));
    // … step 2 still carries its mechanical label.
    expect(tour.getByTestId("provenance-badge")).toHaveTextContent(
      "structural",
    );
  });

  it("walks only steps it can point at on the canvas", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    // The fixture's fourth step names a node this map does not contain.
    const tour = within(screen.getByLabelText("Guided tour"));
    expect(map.tour?.length).toBe(4);
    await user.click(tour.getByRole("button", { name: /start tour/i }));
    expect(tour.getByText("Step 1 of 3")).toBeInTheDocument();
    expect(
      tour.queryByText("A step whose node is not in this map"),
    ).not.toBeInTheDocument();
  });
});

/** Expands a domain group and returns a scope over the flows panel. */
async function openDomain(user: ReturnType<typeof userEvent.setup>, domain: string) {
  const flows = within(screen.getByLabelText("Domain flows"));
  await user.click(
    within(flows.getByRole("heading", { name: new RegExp(`^${domain}`) })).getByRole(
      "button",
    ),
  );
  return flows;
}

describe("domain flows", () => {
  it("opens as an index of domains, each expanding to its flows", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    // Every domain in the map is listed, with how many flows it holds — a
    // real repo has a flow per entry point, far too many to list at once.
    const panel = within(screen.getByLabelText("Domain flows"));
    expect(panel.getByRole("heading", { name: /^cli/ })).toHaveTextContent(
      "cli 1",
    );
    expect(panel.getByRole("heading", { name: /^src/ })).toHaveTextContent(
      "src 1",
    );
    expect(panel.queryByText("Greeting delivery")).not.toBeInTheDocument();

    const flows = await openDomain(user, "src");
    expect(flows.getByText("Greeting delivery")).toBeVisible();
    // Only the expanded domain's flows are listed.
    expect(flows.queryByText("run → parseArgs")).not.toBeInTheDocument();
  });

  it("badges each flow name's provenance", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    const mechanical = (await openDomain(user, "cli")).getByRole("button", {
      name: /run → parseArgs/,
    });
    expect(
      within(mechanical).getByTestId("provenance-badge"),
    ).toHaveTextContent("structural");

    const enriched = (await openDomain(user, "src")).getByRole("button", {
      name: /Greeting delivery/,
    });
    expect(within(enriched).getByTestId("provenance-badge")).toHaveTextContent(
      "llm",
    );
  });

  it("opens a flow's ordered steps and selects each on the canvas", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    const flows = await openDomain(user, "src");
    await user.click(flows.getByRole("button", { name: /Greeting delivery/ }));

    // Opening a flow lands the newcomer on its entry point.
    expect(selectedOnCanvas()).toBe("function:src/main.ts:main");

    const steps = within(flows.getByLabelText("Steps of Greeting delivery"));
    const buttons = steps.getAllByRole("button");
    // In call order — and the step naming a node this map lacks is not
    // offered, because it cannot be pointed at.
    expect(buttons.map((b) => b.textContent)).toEqual([
      "main",
      "greet",
      "format",
    ]);

    const format = buttons[2];
    expect(format).toBeDefined();
    if (format) {
      await user.click(format);
    }
    expect(selectedOnCanvas()).toBe("function:src/util.ts:format");
    expect(
      within(screen.getByLabelText("Node detail")).getByRole("heading", {
        name: "format",
      }),
    ).toBeInTheDocument();
  });

  it("shows one flow's steps at a time", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    let flows = await openDomain(user, "src");
    await user.click(flows.getByRole("button", { name: /Greeting delivery/ }));
    expect(flows.getByLabelText("Steps of Greeting delivery")).toBeVisible();

    flows = await openDomain(user, "cli");
    await user.click(flows.getByRole("button", { name: /run → parseArgs/ }));

    expect(
      flows.queryByLabelText("Steps of Greeting delivery"),
    ).not.toBeInTheDocument();
    expect(
      flows.getByLabelText("Steps of run → parseArgs"),
    ).toBeInTheDocument();
  });
});

describe("maps without a tour or domain flows", () => {
  // Both fields are optional in the contract, and a repository whose files
  // neither import nor call one another legitimately has neither.
  const bare: KnowledgeGraph = (() => {
    const { tour: _tour, domain_flows: _flows, ...rest } = map;
    return rest;
  })();

  it("renders the explorer without either affordance and without errors", () => {
    render(<MapExplorer map={bare} />);

    expect(screen.queryByLabelText("Guided tour")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Domain flows")).not.toBeInTheDocument();
    expect(
      screen.getAllByText("main.ts", { selector: ".react-flow__node *" })
        .length,
    ).toBeGreaterThan(0);
  });

  it("renders empty collections the same way", () => {
    render(<MapExplorer map={{ ...map, tour: [], domain_flows: [] }} />);

    expect(screen.queryByLabelText("Guided tour")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Domain flows")).not.toBeInTheDocument();
  });

  it("renders an older map that predates both fields", () => {
    render(<MapExplorer map={oldMap as KnowledgeGraph} />);

    expect(screen.queryByLabelText("Guided tour")).not.toBeInTheDocument();
    expect(screen.queryByLabelText("Domain flows")).not.toBeInTheDocument();
    expect(
      screen.getAllByText("Greeter", { selector: ".react-flow__node *" })
        .length,
    ).toBeGreaterThan(0);
  });
});
