// Seam 5 (the jsdom component seam): gesture → state. The drill view opens
// showing the files that carry a region; one affordance reveals the rest
// (story 2), and anything that points at a specific file reveals it first
// (story 3).
//
// What the *selection* is — top 40 by published significance, ties on path —
// and which regions a pointer has to reveal belong to the pure projection and
// are asserted there (`regions-and-insights.test.ts`). What this file asserts
// is the half jsdom can see: how many cards the canvas holds, what the control
// says, that nothing the reader is told about the region's size changes when
// cards are hidden, and that each of the four features that name a file lands
// on a card that is actually drawn.
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";
import type { DiffOverlay } from "../src/app/overlay.js";
import {
  openDomainGrouping,
  openLearn,
  openRegion,
  selectedOnCanvas,
} from "./drive.js";

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

/** The same map with one call flow in it, so the domain grouping has
 * something to draw: one small domain around the flow's file, and the
 * catch-all region holding the other sixty-two. Regrouping therefore moves
 * every file into a region whose own default view hides most of it — which
 * is the situation a reveal made under the other grouping has to survive or
 * be let go of. */
function withOneFlow(map: KnowledgeGraph): KnowledgeGraph {
  return {
    ...map,
    nodes: [
      ...map.nodes,
      {
        id: "function:narrow/f002.ts:go",
        kind: "function",
        name: "go",
        path: "narrow/f002.ts",
        summary: "the flow's root",
        provenance: "structural",
      },
    ],
    domain_flows: [
      {
        id: "flow:function:narrow/f002.ts:go",
        domain: "greeting",
        name: "go",
        provenance: "structural",
        steps: ["function:narrow/f002.ts:go"],
      },
    ],
  };
}

/** How many file cards the canvas is holding. */
function cardCount(): number {
  return document.querySelectorAll(".react-flow__node .entity").length;
}

/** Whether the canvas is drawing a card for this file at all. */
function onCanvas(id: string): boolean {
  return document.querySelector(`[data-id="${CSS.escape(id)}"]`) !== null;
}

/** The diff mark on a file's card — or `not drawn`, which is the failure the
 * whole story exists to prevent and so is worth naming rather than throwing
 * on. */
function highlightOf(id: string): string {
  const el = document.querySelector(`[data-id="${CSS.escape(id)}"] .entity`);
  if (el === null) {
    return "not drawn";
  }
  if (el.classList.contains("entity-changed")) {
    return "changed";
  }
  return el.classList.contains("entity-affected") ? "affected" : "none";
}

/** The file the detail panel is describing while the canvas holds no card
 * for it — the contradiction story 3 exists to prevent, named rather than
 * thrown on. `null` when the panel and the canvas agree, which includes the
 * panel not being there at all. Asked this way round on purpose: it is the
 * one thing that must not be true whichever way a grouping change resolves
 * the selection, so the assertion does not presume the resolution. */
function describedButUndrawn(): string | null {
  const path = document.querySelector(".detail .detail-path")?.textContent;
  if (path === undefined || path === null || path === "") {
    return null;
  }
  return onCanvas(`file:${path}`) ? null : path;
}

/** `wide`'s least significant file, and so one of the twenty its default
 * drill view holds back. */
const HIDDEN = "file:wide/f000.ts";

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

describe("nothing points at a hidden file", () => {
  it("reveals the region a search hit lands in", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={twoRegions(60)} />);

    // Nothing drilled into, and the hit is on one of the twenty files the
    // default view of `wide` holds back.
    await user.type(screen.getByLabelText("Search nodes"), "wide/f000.ts");
    await user.click(
      within(screen.getByLabelText("Search results")).getByText("wide/f000.ts"),
    );

    expect(onCanvas(HIDDEN)).toBe(true);
    expect(selectedOnCanvas()).toBe(HIDDEN);
  });

  it("reveals the region a focused file lands in", async () => {
    // The FILES tab: the panel whose whole job is "where is the thing I
    // already know the name of". Every panel that points the canvas at a file
    // — a citation, a path step, an edge in the detail panel, a flow step —
    // goes through the same call this row does.
    const user = userEvent.setup();
    render(<MapExplorer map={twoRegions(60)} />);

    await user.click(screen.getByRole("tab", { name: "Files" }));
    const wide = within(screen.getByLabelText("Files in wide"));
    await user.click(wide.getByRole("button", { expanded: false }));
    await user.click(wide.getByRole("button", { name: "wide/f000.ts" }));

    expect(onCanvas(HIDDEN)).toBe(true);
    expect(selectedOnCanvas()).toBe(HIDDEN);
  });

  it("reveals the region a tour stop lands in", async () => {
    const user = userEvent.setup();
    render(
      <MapExplorer
        map={{
          ...twoRegions(60),
          tour: [
            {
              node: HIDDEN,
              label: "A stop on a file the default view holds back.",
              provenance: "structural",
            },
          ],
        }}
      />,
    );

    await openLearn(user);
    await user.click(
      within(screen.getByLabelText("Codebase tour")).getByRole("button", {
        name: /start tour/i,
      }),
    );

    expect(onCanvas(HIDDEN)).toBe(true);
    expect(selectedOnCanvas()).toBe(HIDDEN);
  });

  it("reveals every region the diff overlay marks", async () => {
    // Marks in both regions, and the overlay is switched on before either is
    // drilled into: the overlay names files across the whole map, so it
    // reveals across the whole map rather than only where the reader is
    // standing.
    const user = userEvent.setup();
    const overlay: DiffOverlay = {
      version: 1,
      changed: [HIDDEN],
      affected: ["file:wide/f001.ts", "file:narrow/f000.ts"],
      unmapped_paths: [],
    };
    render(<MapExplorer map={twoRegions(60)} overlay={overlay} />);

    await user.click(screen.getByLabelText("Diff overlay"));
    await openRegion(user, "wide");

    expect(highlightOf(HIDDEN)).toBe("changed");
    expect(highlightOf("file:wide/f001.ts")).toBe("affected");
    // The other region's mark is on a file its own default view draws, so
    // nothing was revealed there and nothing needed to be.
    await openRegion(user, "narrow");
    expect(highlightOf("file:narrow/f000.ts")).toBe("affected");
  });

  it("leaves a region the reader collapsed collapsed, overlay and all", async () => {
    // The overlay reveals from an effect rather than from a handler, because
    // its marks have to outlive a grouping change — which makes it the one
    // reveal in the product that can fire more than once. A reader who has
    // put the region back to its readable forty has said what they want, and
    // the next render of anything at all must not talk them out of it.
    const user = userEvent.setup();
    const overlay: DiffOverlay = {
      version: 1,
      changed: [HIDDEN],
      affected: [],
      unmapped_paths: [],
    };
    render(<MapExplorer map={twoRegions(60)} overlay={overlay} />);

    await user.click(screen.getByLabelText("Diff overlay"));
    await openRegion(user, "wide");
    expect(cardCount()).toBe(60);
    expect(highlightOf(HIDDEN)).toBe("changed");

    await user.click(screen.getByRole("button", { name: "Show the top 40" }));
    expect(cardCount()).toBe(40);

    // Something else entirely: a look at the FILES tab and back. The overlay
    // is still on and still marks a file behind the cut — neither of which
    // is news, and neither of which is a reason to re-open what the reader
    // just closed.
    await user.click(screen.getByRole("tab", { name: "Files" }));
    await user.click(screen.getByRole("tab", { name: "Info" }));

    expect(cardCount()).toBe(40);
    expect(highlightOf(HIDDEN)).toBe("not drawn");
  });

  it("puts an auto-reveal on the same control the reader already has", async () => {
    // One mechanism: what a pointer revealed, the affordance can put back,
    // because both wrote the same projection input. A second reveal path with
    // its own state would leave this control reading "Show all 60 files"
    // over a canvas already drawing sixty.
    const user = userEvent.setup();
    render(<MapExplorer map={twoRegions(60)} />);

    await user.type(screen.getByLabelText("Search nodes"), "wide/f000.ts");
    await user.click(
      within(screen.getByLabelText("Search results")).getByText("wide/f000.ts"),
    );
    expect(cardCount()).toBe(60);

    await user.click(screen.getByRole("button", { name: "Show the top 40" }));
    expect(cardCount()).toBe(40);
  });

  it("never resets a reveal the reader asked for, and never reverts its own", async () => {
    const user = userEvent.setup();
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

    // A pointer into the other region reveals that one and says nothing about
    // this one.
    await user.type(screen.getByLabelText("Search nodes"), "narrow/g000.ts");
    await user.click(
      within(screen.getByLabelText("Search results")).getByText(
        "narrow/g000.ts",
      ),
    );
    expect(cardCount()).toBe(53);

    // The reader's own reveal is where they left it …
    await openRegion(user, "wide");
    expect(cardCount()).toBe(60);

    // … and so is the one they never asked for: an auto-reveal that lapsed
    // the moment the reader looked elsewhere would put the file back behind
    // the cut while they were still reading about it.
    await openRegion(user, "narrow");
    expect(cardCount()).toBe(53);
  });

  it("does not leave the reader pointed at a file the new grouping hides", async () => {
    // Changing the grouping clears the revealed set, deliberately: the two
    // groupings draw their region IDs from the same well. The reveal that a
    // pointer made goes with it — it was a one-shot call, not a standing
    // instruction — so a selection that outlived it would be a detail panel
    // describing a card the canvas is no longer drawing.
    const user = userEvent.setup();
    render(<MapExplorer map={withOneFlow(twoRegions(60))} />);

    await user.type(screen.getByLabelText("Search nodes"), "wide/f000.ts");
    await user.click(
      within(screen.getByLabelText("Search results")).getByText("wide/f000.ts"),
    );
    expect(selectedOnCanvas()).toBe(HIDDEN);

    // Regroup, then walk back down to where that file now lives: the
    // catch-all domain, whose own default view holds it back just as `wide`
    // did.
    await openDomainGrouping(user);
    await openRegion(user, "No call flow");

    expect(describedButUndrawn()).toBeNull();
    expect(selectedOnCanvas()).toBe(onCanvas(HIDDEN) ? HIDDEN : null);
  });
});
