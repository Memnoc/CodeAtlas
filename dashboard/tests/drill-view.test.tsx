// Seam 5 (the jsdom component seam): gesture → state. The drill view opens
// showing the files that carry a region; one affordance reveals the rest.
//
// What the *selection* is — top 40 by published significance, ties on path —
// belongs to the pure projection and is asserted there
// (`regions-and-insights.test.ts`). What this file asserts is the half jsdom
// can see: how many cards the canvas holds, what the control says, and that
// nothing the reader is told about the region's size changes when cards are
// hidden.
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";
import { openRegion } from "./drive.js";

/** A map of two regions: `wide` holds `wideCount` files, `narrow` holds
 * three. Significance rises with the index, so which forty the projection
 * chooses is decided and not accidental. */
function twoRegions(wideCount: number): KnowledgeGraph {
  const file = (layer: string, i: number) => ({
    id: `file:${layer}/f${String(i).padStart(3, "0")}.ts`,
    kind: "file" as const,
    name: `f${String(i).padStart(3, "0")}.ts`,
    path: `${layer}/f${String(i).padStart(3, "0")}.ts`,
    summary: `file ${i} of ${layer}`,
    layer,
    provenance: "structural" as const,
    significance: i,
  });
  return {
    version: "0.4.0",
    project: { name: "disclosure" },
    layers: [
      { id: "wide", name: "wide", provenance: "structural" },
      { id: "narrow", name: "narrow", provenance: "structural" },
    ],
    nodes: [
      ...Array.from({ length: wideCount }, (_, i) => file("wide", i)),
      ...Array.from({ length: 3 }, (_, i) => file("narrow", i)),
    ],
    edges: [],
  };
}

/** How many file cards the canvas is holding. */
function cardCount(): number {
  return document.querySelectorAll(".react-flow__node .entity").length;
}

describe("the drill view opens readable", () => {
  it("draws forty cards for a wider region and reveals the rest on the gesture", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={twoRegions(60)} />);

    await openRegion(user, "wide");
    expect(cardCount()).toBe(40);

    // The control names the whole region and how much of it is put away —
    // both true numbers, so the reader knows what they are asking for.
    const reveal = screen.getByRole("button", {
      name: "Show all 60 files (20 hidden)",
    });
    await user.click(reveal);

    expect(cardCount()).toBe(60);
  });

  it("offers no reveal control for a region of forty files or fewer", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={twoRegions(40)} />);

    await openRegion(user, "wide");

    expect(cardCount()).toBe(40);
    expect(screen.queryByRole("button", { name: /^Show all/ })).toBeNull();
  });

  it("keeps reporting the region's true file count while cards are hidden", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={twoRegions(60)} />);

    // The overview's card, before anything is hidden or drilled into.
    expect(
      within(screen.getByTestId("region-wide")).getByText("60 files"),
    ).toBeInTheDocument();

    await openRegion(user, "wide");

    // The chip, the info panel's row, and the canvas caption all still say
    // sixty. The default view hides cards; it never hides facts.
    const chip = screen
      .getAllByRole("button")
      .find((b) => b.classList.contains("region-chip") &&
        b.textContent?.startsWith("wide"));
    expect(chip?.textContent).toContain("60");
    expect(
      within(screen.getByLabelText("Regions")).getByText("60 files"),
    ).toBeInTheDocument();
    expect(document.querySelector(".crumb-note")?.textContent).toContain(
      "60 files",
    );
  });

  it("puts the region back to its readable forty", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={twoRegions(60)} />);

    await openRegion(user, "wide");
    await user.click(
      screen.getByRole("button", { name: "Show all 60 files (20 hidden)" }),
    );
    expect(cardCount()).toBe(60);

    await user.click(screen.getByRole("button", { name: "Show the top 40" }));

    expect(cardCount()).toBe(40);
  });

  it("reveals one region only — it is a gesture, not a preference", async () => {
    const user = userEvent.setup();
    // Both regions are wider than the default view, so a global "show
    // everything" would be indistinguishable from a region-scoped one.
    const map = twoRegions(60);
    const wider = {
      ...map,
      nodes: [
        ...map.nodes,
        ...Array.from({ length: 50 }, (_, i) => ({
          id: `file:narrow/g${String(i).padStart(3, "0")}.ts`,
          kind: "file" as const,
          name: `g${String(i).padStart(3, "0")}.ts`,
          path: `narrow/g${String(i).padStart(3, "0")}.ts`,
          summary: "",
          layer: "narrow",
          provenance: "structural" as const,
          significance: i,
        })),
      ],
    };
    render(<MapExplorer map={wider} />);

    await openRegion(user, "wide");
    await user.click(
      screen.getByRole("button", { name: "Show all 60 files (20 hidden)" }),
    );
    expect(cardCount()).toBe(60);

    await openRegion(user, "narrow");

    // 53 files here, and the reveal of `wide` says nothing about them.
    expect(cardCount()).toBe(40);
    expect(
      screen.getByRole("button", { name: "Show all 53 files (13 hidden)" }),
    ).toBeInTheDocument();

    // Coming back finds `wide` as the reader left it: revealing is scoped to
    // a region, and visiting another one is not a reason to undo it.
    await openRegion(user, "wide");
    expect(cardCount()).toBe(60);
  });

  it("does not carry a reveal across a change of grouping", async () => {
    // The two groupings draw region IDs from the same well — a layer and a
    // domain can both be called `crates` — so a reveal that survived the
    // switch would open a region nobody asked to see in full.
    const user = userEvent.setup();
    render(<MapExplorer map={twoRegions(60)} />);

    await openRegion(user, "wide");
    await user.click(
      screen.getByRole("button", { name: "Show all 60 files (20 hidden)" }),
    );
    expect(cardCount()).toBe(60);

    const grouping = within(screen.getByRole("radiogroup", { name: "Grouping" }));
    await user.click(grouping.getByRole("radio", { name: "Domain" }));
    await user.click(grouping.getByRole("radio", { name: "Structural" }));
    await openRegion(user, "wide");

    expect(cardCount()).toBe(40);
  });
});
