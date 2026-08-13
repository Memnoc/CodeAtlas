// Seam 5 (the jsdom component seam): gesture → state, for stories 24 and 25.
// Which files a lens holds and how they layer belongs to the pure projection
// and is asserted there (`regions-and-insights.test.ts`). What this file
// asserts is the half jsdom can see: entering magnify draws only the
// neighbourhood, leaving restores the view the reader came from — selection,
// disclosure and all — Escape leaves through the one cascade, and a pointer
// made inside the lens goes through the same pointer path as every other.
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";
import { openRegion, selectedOnCanvas } from "./drive.js";

/** Two regions wired for the lens. `wide` holds sixty files, significance
 * rising with the index, so its default drill view holds f000…f019 back; the
 * top file `f059.ts` imports the hidden `f000.ts`, is imported by `f058.ts`
 * and by `narrow/f001.ts` from the other region, and `narrow/f002.ts`
 * touches nothing at all — one fixture, every case the lens has to draw. */
function neighbourly(): KnowledgeGraph {
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
  const imports = (from: string, to: string) => ({
    source: `file:${from}`,
    target: `file:${to}`,
    kind: "imports" as const,
    weight: 1,
  });
  return {
    version: "0.4.0",
    project: { name: "magnify" },
    layers: [
      { id: "wide", name: "wide", provenance: "structural" },
      { id: "narrow", name: "narrow", provenance: "structural" },
    ],
    nodes: [
      ...Array.from({ length: 60 }, (_, i) => file("wide", i)),
      ...Array.from({ length: 3 }, (_, i) => file("narrow", i)),
    ],
    edges: [
      imports("wide/f059.ts", "wide/f000.ts"),
      imports("wide/f058.ts", "wide/f059.ts"),
      imports("narrow/f001.ts", "wide/f059.ts"),
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

/** Selects a file through the search overlay — the same place as pressing
 * the card, without pressing the card: React Flow's d3-drag reads
 * `event.view.document` on mousedown, which jsdom leaves null, and the
 * asynchronous throw reads as standing noise (`back-navigation.test.tsx`
 * records the same detour). */
async function selectFile(
  user: ReturnType<typeof userEvent.setup>,
  path: string,
): Promise<void> {
  await user.type(screen.getByLabelText("Search nodes"), path);
  await user.click(
    within(screen.getByLabelText("Search results")).getByText(path),
  );
  await user.clear(screen.getByLabelText("Search nodes"));
}

const back = () => screen.getByTestId("back");

describe("magnify draws the neighbourhood", () => {
  it("magnifies the selected file to its neighbourhood and nothing else", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={neighbourly()} />);

    // No selection, nothing to magnify: the control is not there to press.
    await openRegion(user, "wide");
    expect(screen.queryByRole("button", { name: /^Magnify/ })).toBeNull();

    await selectFile(user, "wide/f059.ts");
    await user.click(screen.getByRole("button", { name: "Magnify f059.ts" }));

    // The file, what it imports, and the two files that import it — one of
    // them from the other region, because the neighbourhood is the map's,
    // not the open region's — and not the other fifty-nine cards.
    expect(cardCount()).toBe(4);
    expect(onCanvas("file:wide/f059.ts")).toBe(true);
    expect(onCanvas("file:wide/f000.ts")).toBe(true);
    expect(onCanvas("file:wide/f058.ts")).toBe(true);
    expect(onCanvas("file:narrow/f001.ts")).toBe(true);
    expect(selectedOnCanvas()).toBe("file:wide/f059.ts");
  });

  it("draws the hidden neighbour without writing a reveal", async () => {
    // f000 is one of the twenty the default drill view holds back. The lens
    // draws it because the lens draws from the map — so after leaving, the
    // disclosure is exactly as the reader left it: forty cards, and the
    // control still offering all sixty.
    const user = userEvent.setup();
    render(<MapExplorer map={neighbourly()} />);

    await selectFile(user, "wide/f059.ts");
    expect(onCanvas("file:wide/f000.ts")).toBe(false);

    await user.click(screen.getByRole("button", { name: "Magnify f059.ts" }));
    expect(onCanvas("file:wide/f000.ts")).toBe(true);

    await user.click(back());
    expect(cardCount()).toBe(40);
    expect(
      screen.getByRole("button", { name: "Show all 60 files (20 hidden)" }),
    ).toBeInTheDocument();
  });

  it("leaves through the back control to exactly where the reader was", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={neighbourly()} />);

    await selectFile(user, "wide/f059.ts");
    expect(cardCount()).toBe(40);

    await user.click(screen.getByRole("button", { name: "Magnify f059.ts" }));
    expect(cardCount()).toBe(4);

    // Named for where it goes: the region the reader was reading.
    expect(back()).toHaveTextContent(/back to wide/i);
    await user.click(back());

    expect(cardCount()).toBe(40);
    expect(selectedOnCanvas()).toBe("file:wide/f059.ts");
    expect(document.querySelector(".crumb-note")?.textContent).toContain(
      "60 files",
    );
  });

  it("leaves through the one Escape cascade, one step at a time", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={neighbourly()} />);

    await selectFile(user, "wide/f059.ts");
    await user.click(screen.getByRole("button", { name: "Magnify f059.ts" }));
    expect(cardCount()).toBe(4);

    // Lens off — the view underneath, selection intact.
    await user.keyboard("{Escape}");
    expect(cardCount()).toBe(40);
    expect(selectedOnCanvas()).toBe("file:wide/f059.ts");

    // Then the selection, then the region: the stack it always was.
    await user.keyboard("{Escape}");
    expect(selectedOnCanvas()).toBeNull();
    expect(back()).toHaveTextContent(/back to regions/i);
    await user.keyboard("{Escape}");
    expect(screen.getByTestId("region-wide")).toBeInTheDocument();
  });

  it("magnifies a file with no relating edges to itself alone, and says so", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={neighbourly()} />);

    await selectFile(user, "narrow/f002.ts");
    await user.click(screen.getByRole("button", { name: "Magnify f002.ts" }));

    expect(cardCount()).toBe(1);
    expect(document.querySelector(".crumb-note")?.textContent).toContain(
      "imports nothing and nothing imports it",
    );
  });

  it("keeps the lens for a pointer at a file it draws, revealing underneath", async () => {
    // A pointer is a pointer wherever it is made: clicking the hidden
    // neighbour's search hit inside the lens selects it and reveals its
    // region through auto-reveal's own mechanism — so that leaving finds the
    // pointed-at card actually drawn, not put back behind the cut.
    const user = userEvent.setup();
    render(<MapExplorer map={neighbourly()} />);

    await selectFile(user, "wide/f059.ts");
    await user.click(screen.getByRole("button", { name: "Magnify f059.ts" }));

    await selectFile(user, "wide/f000.ts");
    expect(cardCount()).toBe(4);
    expect(selectedOnCanvas()).toBe("file:wide/f000.ts");

    await user.click(back());
    expect(cardCount()).toBe(60);
    expect(selectedOnCanvas()).toBe("file:wide/f000.ts");
    expect(
      screen.getByRole("button", { name: "Show the top 40" }),
    ).toBeInTheDocument();
  });

  it("drops the lens for a pointer at a file it does not draw", async () => {
    // The lens draws four files; a search hit on any other file needs the
    // canvas underneath, so the pointer takes the reader there — the same
    // journey the hit makes when no lens is up.
    const user = userEvent.setup();
    render(<MapExplorer map={neighbourly()} />);

    await selectFile(user, "wide/f059.ts");
    await user.click(screen.getByRole("button", { name: "Magnify f059.ts" }));
    expect(cardCount()).toBe(4);

    await selectFile(user, "narrow/f000.ts");

    expect(cardCount()).toBe(3);
    expect(selectedOnCanvas()).toBe("file:narrow/f000.ts");
    expect(document.querySelector(".crumb-note")?.textContent).toContain(
      "3 files",
    );
  });
});
