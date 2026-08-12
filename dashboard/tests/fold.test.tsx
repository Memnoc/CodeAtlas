// Folding the frame away to get more canvas. Driven through `<MapExplorer/>`
// with real user events, because every property here is about what is on the
// page after a press, and none of it is arithmetic.
//
// What this file deliberately does *not* claim: that anything got bigger.
// jsdom lays nothing out, so the canvas is zero pixels wide folded and zero
// pixels wide open, and a test comparing the two would pass over a fold that
// did nothing at all. The width belongs to `grid-template-columns` in
// `styles.css`, which is never loaded here. What is assertable — and what
// would actually break — is which controls exist afterwards, that nothing
// folds away the only way to reach something, and that the fold is
// remembered.
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";
import { CHROME_KEY, readChrome } from "../src/app/chrome.js";
import type { DiffOverlay } from "../src/app/overlay.js";
import { WALKTHROUGH_MARKER } from "../src/app/walkthrough.js";
import smallMap from "../tests/fixtures/small-map.json";
import smallOverlay from "../tests/fixtures/small-overlay.json";

const map = smallMap as KnowledgeGraph;
const overlay = smallOverlay as DiffOverlay;

const panel = () => document.querySelector(".rightpanel");
const chips = () =>
  screen.queryAllByRole("button").filter((b) => b.classList.contains("region-chip"));

beforeEach(() => {
  localStorage.clear();
});

describe("folding the side panel", () => {
  it("takes the panel away and leaves the way back", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    expect(panel()).not.toBeNull();

    await user.click(screen.getByRole("button", { name: "Hide the side panel" }));

    expect(panel()).toBeNull();
    // The class is the jsdom half of story 22's space claim: this proves the
    // gesture applies it, and tests/stylesheet-contract.test.ts proves the
    // class narrows the grid track it names. Neither alone says the canvas
    // grew; together they leave only the CSS engine to trust.
    expect(document.querySelector(".workspace")?.classList.contains("workspace-folded")).toBe(true);
    // The rail is the whole point: a fold with no visible way back is a
    // feature that loses the reader their panel permanently.
    const back = screen.getByRole("button", { name: /Panel/ });
    expect(back).toBeVisible();

    await user.click(back);
    expect(panel()).not.toBeNull();
  });

  it("unmounts the panel rather than hiding it, so the walkthrough skips it", async () => {
    // The interface walkthrough resolves its steps against the elements on the
    // page. A panel hidden with CSS would still be found and then spotlighted
    // as a hole of no size — a dimmed page with nothing lit in it. This is the
    // assertion that keeps the fold honest about which mechanism it used.
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    const marked = () =>
      document.querySelector(`[${WALKTHROUGH_MARKER}="panel"]`);
    expect(marked()).not.toBeNull();

    await user.click(screen.getByRole("button", { name: "Hide the side panel" }));

    expect(marked()).toBeNull();
  });
});

describe("folding the region chips", () => {
  it("takes the chips away but not the diff overlay toggle beside them", async () => {
    // The toggle sits in the chip row and is not a chip. Folding the row
    // wholesale would make the diff overlay unreachable, which is the one
    // thing a fold must never do.
    const user = userEvent.setup();
    render(<MapExplorer map={map} overlay={overlay} />);
    expect(chips().length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: /regions?$/ }));

    expect(chips()).toHaveLength(0);
    expect(screen.getByLabelText("Diff overlay")).toBeVisible();
  });

  it("says how many regions it folded away, so the row still means something", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    const count = () =>
      screen
        .getByRole("button", { name: /regions?$/ })
        .querySelector(".chiprow-total")?.textContent;
    const before = count();
    expect(before).toMatch(/^\d+ regions?$/);

    await user.click(screen.getByRole("button", { name: /regions?$/ }));

    // The glyph flips; the count is what tells the reader what is behind it,
    // and a row reading nothing but an arrow says nothing.
    expect(count()).toBe(before);
  });
});

describe("the Focus control", () => {
  it("folds both at once and brings both back", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    const focus = () => screen.getByRole("button", { name: "Focus" });
    expect(focus()).toHaveAttribute("aria-pressed", "false");

    await user.click(focus());

    expect(panel()).toBeNull();
    expect(chips()).toHaveLength(0);
    expect(focus()).toHaveAttribute("aria-pressed", "true");

    await user.click(focus());

    expect(panel()).not.toBeNull();
    expect(chips().length).toBeGreaterThan(0);
    expect(focus()).toHaveAttribute("aria-pressed", "false");
  });

  it("reads as pressed after a fold made with one of the other two controls", async () => {
    // Otherwise the reader folds the panel from the panel, then presses an
    // unpressed-looking Focus and folds the chips as well — the opposite of
    // what a control showing "off" promises.
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await user.click(screen.getByRole("button", { name: "Hide the side panel" }));

    expect(screen.getByRole("button", { name: "Focus" })).toHaveAttribute(
      "aria-pressed",
      "true",
    );

    // And pressing it now restores rather than folding further.
    await user.click(screen.getByRole("button", { name: "Focus" }));
    expect(panel()).not.toBeNull();
    expect(chips().length).toBeGreaterThan(0);
  });
});

describe("remembering the fold", () => {
  it("comes back folded", async () => {
    const user = userEvent.setup();
    const first = render(<MapExplorer map={map} />);
    await user.click(screen.getByRole("button", { name: "Focus" }));
    first.unmount();

    render(<MapExplorer map={map} />);

    expect(panel()).toBeNull();
    expect(chips()).toHaveLength(0);
  });

  it("opens everything when the stored value is nonsense", () => {
    // A reader who cannot see the panel and does not know it exists has no way
    // to ask for it back, so every unreadable case has to fail open. Storage
    // written by an older or newer build is the realistic source of this.
    localStorage.setItem(CHROME_KEY, "{not json");
    expect(readChrome()).toEqual({ panel: false, chips: false });

    localStorage.setItem(CHROME_KEY, JSON.stringify({ panel: "yes" }));
    expect(readChrome()).toEqual({ panel: false, chips: false });

    localStorage.setItem(CHROME_KEY, JSON.stringify(["panel"]));
    render(<MapExplorer map={map} />);
    expect(panel()).not.toBeNull();
  });
});

describe("what does not fold", () => {
  it("keeps the search field and the top bar whatever is folded", async () => {
    // Folding is for things the reader can see again by unfolding. Search is
    // how you find a node you cannot see, which makes it the last thing that
    // should disappear when the reader asks for more map.
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await user.click(screen.getByRole("button", { name: "Focus" }));

    expect(screen.getByLabelText("Search nodes")).toBeVisible();
    expect(
      within(screen.getByRole("radiogroup", { name: "View" })).getByRole("radio", {
        name: "Overview",
      }),
    ).toBeVisible();
  });
});
