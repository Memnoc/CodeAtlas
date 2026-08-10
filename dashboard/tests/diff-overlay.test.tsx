// Seam 1 (artifact side): feed a fixture map plus a fixture diff overlay in,
// assert the toggle renders and the right nodes get the right highlight
// classes. The overlay is the internal artifact `codeatlas diff` writes —
// not the map contract — so its type lives beside the app, not in the
// generated contract types.
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";
import type { DiffOverlay } from "../src/app/overlay.js";
import { openRegion } from "./drive.js";
import smallMap from "./fixtures/small-map.json";
import smallOverlay from "./fixtures/small-overlay.json";

const map = smallMap as KnowledgeGraph;
const overlay = smallOverlay as DiffOverlay;

function highlightOf(nodeId: string): string {
  const el = document.querySelector(`[data-id="${CSS.escape(nodeId)}"] .entity`);
  expect(el).not.toBeNull();
  if (el?.classList.contains("entity-changed")) {
    return "changed";
  }
  if (el?.classList.contains("entity-affected")) {
    return "affected";
  }
  return "none";
}

describe("diff impact overlay", () => {
  it("offers no toggle when there is no overlay", () => {
    render(<MapExplorer map={map} />);

    expect(screen.queryByLabelText("Diff overlay")).not.toBeInTheDocument();
  });

  it("renders a toggle when an overlay is present, off by default", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} overlay={overlay} />);

    const toggle = screen.getByLabelText("Diff overlay");
    expect(toggle).not.toBeChecked();
    // Untoggled, the map renders exactly as without an overlay.
    await openRegion(user, "Source code");
    expect(document.querySelector(".entity-changed")).toBeNull();
    expect(document.querySelector(".entity-affected")).toBeNull();
  });

  it("highlights changed and affected files distinctly when toggled on", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} overlay={overlay} />);

    await user.click(screen.getByLabelText("Diff overlay"));
    await openRegion(user, "Source code");

    expect(highlightOf("file:src/main.ts")).toBe("changed");
    // The one-hop neighbour, styled distinctly from changed.
    expect(highlightOf("file:src/greeter.ts")).toBe("affected");

    // A region the overlay never touches stays clean.
    await openRegion(user, "docs");
    expect(highlightOf("file:docs/guide.md")).toBe("none");
  });

  it("rolls a changed symbol up to the file the canvas draws", async () => {
    const user = userEvent.setup();
    // The overlay marks symbols as well as files. The canvas draws files, so
    // a symbol-only mark has to surface on its file — otherwise the reader
    // is told nothing changed in a file that did.
    const symbolOnly: DiffOverlay = {
      ...overlay,
      changed: ["function:src/main.ts:main"],
      affected: [],
    };
    render(<MapExplorer map={map} overlay={symbolOnly} />);

    await user.click(screen.getByLabelText("Diff overlay"));
    await openRegion(user, "Source code");

    expect(highlightOf("file:src/main.ts")).toBe("changed");
    expect(highlightOf("file:src/greeter.ts")).toBe("none");
  });

  it("removes every highlight when toggled back off", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} overlay={overlay} />);

    await user.click(screen.getByLabelText("Diff overlay"));
    await user.click(screen.getByLabelText("Diff overlay"));
    await openRegion(user, "Source code");

    expect(document.querySelector(".entity-changed")).toBeNull();
    expect(document.querySelector(".entity-affected")).toBeNull();
  });
});
