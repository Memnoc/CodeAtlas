// Going in is easy; the explorer's problem was going back out. Overview →
// region → file is a stack, and "back" has to mean one step up it, in the
// place the reader is looking, with a name that says where it goes.
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";
import { openRegion, selectedOnCanvas } from "./drive.js";
import smallMap from "./fixtures/small-map.json";
import tourMap from "./fixtures/tour-map.json";

const map = smallMap as KnowledgeGraph;

const back = () => screen.queryByTestId("back");

/** Selects a file through the search overlay, which also opens the region
 * holding it — so this lands in the same place as pressing the card.
 *
 * Not by pressing the card, deliberately. React Flow makes every node
 * draggable through d3-drag, whose mousedown handler reads
 * `event.view.document`; jsdom leaves `view` null on the events user-event
 * dispatches, so the press throws asynchronously inside d3. The click still
 * lands and the assertion still holds, but vitest reports the throw as an
 * unhandled error, and standing noise that looks like a failure is how a real
 * failure gets ignored. */
async function selectFile(
  user: ReturnType<typeof userEvent.setup>,
  name: string,
): Promise<void> {
  await user.type(screen.getByLabelText("Search nodes"), name);
  await user.click(
    within(screen.getByLabelText("Search results")).getByText(name),
  );
  await user.clear(screen.getByLabelText("Search nodes"));
}

describe("a way back", () => {
  it("offers nothing to go back to from the overview", () => {
    render(<MapExplorer map={map} />);
    expect(back()).not.toBeInTheDocument();
  });

  it("goes back to the overview from an opened region, and says so", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openRegion(user, "Source code");

    const control = back();
    expect(control).toHaveTextContent(/back to regions/i);

    await user.click(control!);
    // The overview draws region cards again.
    expect(screen.getByTestId("region-docs")).toBeInTheDocument();
    expect(back()).not.toBeInTheDocument();
  });

  it("goes back one level from a selected file, not all the way out", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await selectFile(user, "main.ts");
    expect(selectedOnCanvas()).toBe("file:src/main.ts");

    // Named for where it goes, which at this depth is the region.
    expect(back()).toHaveTextContent(/back to source code/i);
    await user.click(back()!);

    expect(selectedOnCanvas()).toBeNull();
    // Still inside the region — one step, not two.
    expect(screen.queryByTestId("region-docs")).not.toBeInTheDocument();
    expect(back()).toHaveTextContent(/back to regions/i);
  });

  it("is reachable and operable from the keyboard", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openRegion(user, "Source code");

    back()!.focus();
    expect(back()).toHaveFocus();
    await user.keyboard("{Enter}");
    expect(screen.getByTestId("region-docs")).toBeInTheDocument();
  });
});

describe("Escape goes back too", () => {
  it("clears a file selection first, leaving the region open", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await selectFile(user, "main.ts");

    await user.keyboard("{Escape}");

    expect(selectedOnCanvas()).toBeNull();
    expect(screen.queryByTestId("region-docs")).not.toBeInTheDocument();
  });

  it("leaves the region on a second press", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await selectFile(user, "main.ts");

    await user.keyboard("{Escape}");
    await user.keyboard("{Escape}");

    expect(screen.getByTestId("region-docs")).toBeInTheDocument();
  });

  it("does nothing at the top, rather than something surprising", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await user.keyboard("{Escape}");
    expect(screen.getByTestId("region-docs")).toBeInTheDocument();
  });

  it("belongs to the search overlay while that is open", async () => {
    // Escape is shared, so precedence has to be stated: the overlay is the
    // innermost thing on screen and gets it first. Without this the reader
    // dismissing a search would also lose the region they were reading.
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openRegion(user, "Source code");
    await user.type(screen.getByLabelText("Search nodes"), "main");

    await user.keyboard("{Escape}");

    expect(screen.queryByLabelText("Search results")).not.toBeInTheDocument();
    expect(screen.queryByTestId("region-docs")).not.toBeInTheDocument();
  });
});

describe("the guided tour's way back", () => {
  it("greys Previous at the first step rather than hiding it", async () => {
    // A multi-step tour, because a one-step tour cannot tell a disabled
    // Previous from one that simply never enables. The fixture lists four
    // steps and the panel shows three: one names a node this map does not
    // contain, and a step that cannot be pointed at is dropped.
    const user = userEvent.setup();
    render(<MapExplorer map={tourMap as KnowledgeGraph} />);
    await user.click(
      within(screen.getByRole("radiogroup", { name: "View" })).getByRole(
        "radio",
        { name: "Learn" },
      ),
    );
    const tour = () => within(screen.getByLabelText("Guided tour"));
    await user.click(tour().getByRole("button", { name: "Start tour" }));

    expect(tour().getByRole("button", { name: "Previous" })).toBeDisabled();
    await user.click(tour().getByRole("button", { name: "Next" }));
    expect(tour().getByRole("button", { name: "Previous" })).toBeEnabled();

    // And it really goes back, rather than only lighting up.
    await user.click(tour().getByRole("button", { name: "Previous" }));
    expect(screen.getByLabelText("Guided tour")).toHaveTextContent(
      /step 1 of 3/i,
    );
  });
});
