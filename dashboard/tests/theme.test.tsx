// AC: the dashboard ships Rosé Pine Dawn and Moon, switchable from the header
// and remembered. The palettes are CSS, which jsdom does not apply, so what is
// asserted here is the contract between the toggle and the stylesheet: the
// `data-theme` attribute the palettes key off, and the storage the choice
// survives in.
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";

const map = {
  version: "0.3.1",
  project: { name: "probe", root: "." },
  nodes: [
    {
      id: "file:a.ts",
      kind: "file",
      name: "a.ts",
      path: "a.ts",
      summary: "A",
      provenance: "structural",
    },
  ],
  edges: [],
} as unknown as KnowledgeGraph;

/** The media query the toggle consults, which jsdom does not implement. */
function stubPrefersDark(prefersDark: boolean) {
  vi.stubGlobal(
    "matchMedia",
    (query: string) =>
      ({
        matches: query.includes("dark") && prefersDark,
        media: query,
      }) as MediaQueryList,
  );
}

beforeEach(() => {
  localStorage.clear();
  document.documentElement.removeAttribute("data-theme");
});

afterEach(() => {
  vi.unstubAllGlobals();
});

describe("theme toggle", () => {
  it("opens in the theme the operating system asks for", () => {
    stubPrefersDark(true);
    render(<MapExplorer map={map} />);
    expect(document.documentElement).toHaveAttribute("data-theme", "moon");
  });

  it("opens in Dawn when the operating system prefers light", () => {
    stubPrefersDark(false);
    render(<MapExplorer map={map} />);
    expect(document.documentElement).toHaveAttribute("data-theme", "dawn");
  });

  it("switches themes from the header and remembers the choice", async () => {
    stubPrefersDark(false);
    const user = userEvent.setup();
    const { unmount } = render(<MapExplorer map={map} />);

    const toggle = screen.getByRole("button", { name: /^Theme: Rosé Pine/ });
    expect(toggle).toHaveAccessibleName(/Dawn\. Switch to Moon\./);

    await user.click(toggle);
    expect(document.documentElement).toHaveAttribute("data-theme", "moon");
    expect(toggle).toHaveAccessibleName(/Moon\. Switch to Dawn\./);

    // The choice outranks the system preference on the next visit.
    unmount();
    document.documentElement.removeAttribute("data-theme");
    render(<MapExplorer map={map} />);
    expect(document.documentElement).toHaveAttribute("data-theme", "moon");
  });

  it("still themes the page when storage refuses, as on file://", async () => {
    stubPrefersDark(false);
    // Some browsers give a double-clicked artifact an opaque origin, where
    // touching localStorage throws rather than returning null.
    vi.stubGlobal("localStorage", {
      getItem() {
        throw new DOMException("denied", "SecurityError");
      },
      setItem() {
        throw new DOMException("denied", "SecurityError");
      },
    });
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    expect(document.documentElement).toHaveAttribute("data-theme", "dawn");
    await user.click(screen.getByRole("button", { name: /^Theme: Rosé Pine/ }));
    expect(document.documentElement).toHaveAttribute("data-theme", "moon");
  });

  it("falls back to the stylesheet's own default when matchMedia is absent", () => {
    vi.stubGlobal("matchMedia", undefined);
    render(<MapExplorer map={map} />);
    expect(document.documentElement).toHaveAttribute("data-theme", "dawn");
  });
});
