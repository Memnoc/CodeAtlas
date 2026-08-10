// Share mode (ticket 14, spec stories 8 and 10): when the document carries
// an embedded share payload, the app renders it directly — no fetch, no
// server — and displays the redaction disclosure. This is the seam the
// share artifact exercises when a colleague double-clicks it from file://,
// where fetch is unusable.
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "../src/app/App.js";
import { SHARE_DATA_ID } from "../src/app/share.js";
import { openLearn, openRegion } from "./drive.js";
import smallMap from "./fixtures/small-map.json";

// What `codeatlas share` emits for an enriched map: every LLM-provenance
// prose slot — node summaries, layer names, flow names, tour narrations —
// replaced with the marker, provenance itself left intact.
const payload = {
  map: {
    ...smallMap,
    nodes: smallMap.nodes.map((node) =>
      node.provenance === "llm" ? { ...node, summary: "[redacted]" } : node,
    ),
    domain_flows: smallMap.domain_flows.map((flow) => ({
      ...flow,
      name: "[redacted]",
      provenance: "llm",
    })),
    tour: smallMap.tour.map((step) => ({
      ...step,
      label: "[redacted]",
      provenance: "llm",
    })),
  },
  redaction: {
    marker: "[redacted]",
    policy: ["DomainFlow.name", "Layer.name", "Node.summary", "TourStep.label"],
    redacted: [
      { field: "DomainFlow.name", count: 1 },
      { field: "Layer.name", count: 1 },
      { field: "Node.summary", count: 2 },
      { field: "TourStep.label", count: 1 },
    ],
  },
};

const realFetch = globalThis.fetch;

beforeEach(() => {
  const script = document.createElement("script");
  script.id = SHARE_DATA_ID;
  script.type = "application/json";
  script.textContent = JSON.stringify(payload);
  document.head.appendChild(script);
  // A double-clicked file:// artifact cannot fetch anything; share mode
  // must never try. Removing fetch makes any attempt crash the test.
  // @ts-expect-error simulating a runtime without usable fetch
  delete globalThis.fetch;
});

afterEach(() => {
  document.getElementById(SHARE_DATA_ID)?.remove();
  globalThis.fetch = realFetch;
});

describe("share mode", () => {
  it("renders the embedded map without fetch existing at all", async () => {
    const user = userEvent.setup();
    render(<App />);

    // The same overview the served dashboard draws, then the same drill-in.
    expect(screen.getByTestId("region-src")).toBeInTheDocument();
    await openRegion(user, "Source code");
    for (const name of ["main.ts", "greeter.ts"]) {
      expect(
        screen.getAllByText(name, { selector: ".react-flow__node *" }).length,
      ).toBeGreaterThan(0);
    }
  });

  it("discloses what was redacted, per field with counts", () => {
    render(<App />);

    const banner = screen.getByRole("note", {
      name: /redaction disclosure/i,
    });
    expect(banner).toHaveTextContent("Layer.name (1)");
    expect(banner).toHaveTextContent("Node.summary (2)");
    expect(banner).toHaveTextContent(/diff overlay/i);
  });

  it("walks the guided tour and the domain flows, redacted labels intact", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Same renderer as the served dashboard (ticket 14), so the newcomer's
    // affordances are in the artifact too — showing the marker, not prose,
    // and badging the slot as enriched so the reader knows why.
    await openLearn(user);
    const tour = within(screen.getByLabelText("Guided tour"));
    await user.click(tour.getByRole("button", { name: /start tour/i }));
    expect(tour.getByText("Step 1 of 1")).toBeInTheDocument();
    expect(tour.getByText(/\[redacted\]/)).toBeVisible();
    expect(tour.getByTestId("provenance-badge")).toHaveTextContent("llm");

    const flows = within(screen.getByLabelText("Domain flows"));
    await user.click(
      within(flows.getByRole("heading", { name: /^src/ })).getByRole("button"),
    );
    await user.click(flows.getByRole("button", { name: /\[redacted\]/ }));
    expect(
      within(flows.getByLabelText("Steps of [redacted]")).getByText("main"),
    ).toBeInTheDocument();
  });

  it("offers no diff overlay toggle in share mode", () => {
    render(<App />);

    expect(
      screen.queryByRole("checkbox", { name: "Diff overlay" }),
    ).not.toBeInTheDocument();
  });
});
