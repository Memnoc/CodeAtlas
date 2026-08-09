// Share mode (ticket 14, spec stories 8 and 10): when the document carries
// an embedded share payload, the app renders it directly — no fetch, no
// server — and displays the redaction disclosure. This is the seam the
// share artifact exercises when a colleague double-clicks it from file://,
// where fetch is unusable.
import { render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { App } from "../src/app/App.js";
import { SHARE_DATA_ID } from "../src/app/share.js";
import smallMap from "./fixtures/small-map.json";

const payload = {
  map: {
    ...smallMap,
    nodes: smallMap.nodes.map((node) =>
      node.provenance === "llm" ? { ...node, summary: "[redacted]" } : node,
    ),
  },
  redaction: {
    marker: "[redacted]",
    policy: ["DomainFlow.name", "Layer.name", "Node.summary", "TourStep.label"],
    redacted: [
      { field: "Layer.name", count: 1 },
      { field: "Node.summary", count: 2 },
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
  it("renders the embedded map without fetch existing at all", () => {
    render(<App />);

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

  it("offers no diff overlay toggle in share mode", () => {
    render(<App />);

    expect(
      screen.queryByRole("checkbox", { name: "Diff overlay" }),
    ).not.toBeInTheDocument();
  });
});
