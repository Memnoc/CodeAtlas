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

  it("renders a toggle when an overlay is present, off by default", () => {
    render(<MapExplorer map={map} overlay={overlay} />);

    const toggle = screen.getByLabelText("Diff overlay");
    expect(toggle).not.toBeChecked();
    // Untoggled, the map renders exactly as without an overlay.
    expect(document.querySelector(".entity-changed")).toBeNull();
    expect(document.querySelector(".entity-affected")).toBeNull();
  });

  it("highlights changed and affected nodes distinctly when toggled on", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} overlay={overlay} />);

    await user.click(screen.getByLabelText("Diff overlay"));

    // Changed: the edited file node and its contained function.
    expect(highlightOf("file:src/main.ts")).toBe("changed");
    expect(highlightOf("function:src/main.ts:main")).toBe("changed");
    // Affected: the one-hop neighbor, styled distinctly from changed.
    expect(highlightOf("file:src/greeter.ts")).toBe("affected");
    // Everything else: untouched.
    expect(highlightOf("class:src/greeter.ts:Greeter")).toBe("none");
    expect(highlightOf("file:docs/guide.md")).toBe("none");
  });

  it("removes every highlight when toggled back off", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} overlay={overlay} />);

    await user.click(screen.getByLabelText("Diff overlay"));
    await user.click(screen.getByLabelText("Diff overlay"));

    expect(document.querySelector(".entity-changed")).toBeNull();
    expect(document.querySelector(".entity-affected")).toBeNull();
  });
});
