// The FILES tab: folding a region away, and narrowing what is left.
//
// Driven through `<MapExplorer/>` with real user events. Nothing here asserts
// anything about size or layout — jsdom lays nothing out — so every claim is
// about which controls and which rows exist after a gesture, which is what
// actually broke: a flat list of every file in the repository, with one
// region's forty-five entries pushing the other seven off the bottom.
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { beforeEach, describe, expect, it } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";
import smallMap from "./fixtures/small-map.json";

const map = smallMap as KnowledgeGraph;

async function openFiles(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("tab", { name: "Files" }));
}

/** Every file row currently on screen, by the path each one shows. */
function rows(): string[] {
  return [...document.querySelectorAll(".file-name")].map(
    (b) => b.textContent ?? "",
  );
}

/** The region headings, which stay whatever is folded. */
function headings(): string[] {
  return screen
    .getAllByRole("button", { expanded: false })
    .concat(screen.queryAllByRole("button", { expanded: true }))
    .filter((b) => b.classList.contains("files-region-toggle"))
    .map((b) => b.textContent ?? "");
}

beforeEach(() => {
  localStorage.clear();
});

describe("the files tab folds its regions", () => {
  it("opens on the shape of the repository, not on every file in it", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await openFiles(user);

    // Headings and counts, and no rows at all until one is asked for. This is
    // the whole change: the tab used to render every file immediately.
    expect(headings().length).toBeGreaterThan(1);
    expect(rows()).toEqual([]);
  });

  it("shows a region's files when its heading is pressed, and hides them again", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openFiles(user);

    const first = screen
      .getAllByRole("button")
      .filter((b) => b.classList.contains("files-region-toggle"))[0];
    if (first === undefined) {
      throw new Error("no region heading to press");
    }
    expect(first).toHaveAttribute("aria-expanded", "false");

    await user.click(first);
    expect(first).toHaveAttribute("aria-expanded", "true");
    const opened = rows();
    expect(opened.length).toBeGreaterThan(0);

    await user.click(first);
    expect(first).toHaveAttribute("aria-expanded", "false");
    expect(rows()).toEqual([]);
  });

  it("folds each region on its own", async () => {
    // Two regions open at once, and closing one leaves the other. A single
    // open-region state would pass every assertion above and fail this.
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openFiles(user);

    const toggles = screen
      .getAllByRole("button")
      .filter((b) => b.classList.contains("files-region-toggle"));
    expect(toggles.length).toBeGreaterThan(1);
    const [a, b] = toggles;
    if (a === undefined || b === undefined) {
      throw new Error("need two regions");
    }

    await user.click(a);
    const justA = rows();
    await user.click(b);
    const both = rows();
    expect(both.length).toBeGreaterThan(justA.length);

    await user.click(a);
    const justB = rows();
    expect(justB.length).toBe(both.length - justA.length);
    expect(justB.length).toBeGreaterThan(0);
  });
});

describe("the files tab filters what it shows", () => {
  it("narrows to matching paths and opens what it matched", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openFiles(user);

    // Everything folded, so any row appearing is the filter's doing rather
    // than something that was already on screen.
    expect(rows()).toEqual([]);

    const all = map.nodes.filter((n) => n.kind === "file");
    const target = all[0];
    if (target === undefined) {
      throw new Error("fixture has no files");
    }
    const stem = target.name.split(".")[0] ?? target.name;

    await user.type(screen.getByLabelText("Filter files"), stem);

    const shown = rows();
    expect(shown.length).toBeGreaterThan(0);
    expect(shown.every((p) => p.toLowerCase().includes(stem.toLowerCase()))).toBe(
      true,
    );
    // And it is a filter, not a no-op: something was left out.
    expect(shown.length).toBeLessThan(all.length);
  });

  it("says so when nothing matches, rather than looking empty", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openFiles(user);

    await user.type(screen.getByLabelText("Filter files"), "zzz-no-such-path");

    expect(screen.getByText("No files match.")).toBeVisible();
    expect(rows()).toEqual([]);
    // Every heading is gone too. Eight empty headings is how a filter that
    // works still looks like one that does not.
    expect(
      screen
        .queryAllByRole("button")
        .filter((b) => b.classList.contains("files-region-toggle")),
    ).toEqual([]);
  });

  it("restores the fold when the filter is cleared", async () => {
    // The filter opens regions while it is running. Clearing it must put them
    // back, not leave every region open — which is the state the reader was
    // trying to escape.
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openFiles(user);

    const field = screen.getByLabelText("Filter files");
    await user.type(field, "src");
    expect(rows().length).toBeGreaterThan(0);

    await user.clear(field);
    expect(rows()).toEqual([]);
  });

  it("counts what it matched against what it hid", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openFiles(user);

    await user.type(screen.getByLabelText("Filter files"), "src");

    const counts = [...document.querySelectorAll(".files-region-count")].map(
      (el) => el.textContent ?? "",
    );
    expect(counts.length).toBeGreaterThan(0);
    // "3 of 7", not "3": a bare count while filtering hides that anything was
    // hidden, which is exactly what the reader needs to know.
    expect(counts.every((c) => / of /.test(c))).toBe(true);
  });

  it("keeps the symbols expander working on a filtered row", async () => {
    // The rows a filter produces are the same rows, not a lesser rendering of
    // them: whatever they could do before, they can still do.
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openFiles(user);
    await user.type(screen.getByLabelText("Filter files"), "src");

    const expander = document.querySelector<HTMLElement>(".file-expand");
    if (expander === null) {
      throw new Error("no file in the filtered list contains symbols");
    }
    expect(expander).toHaveAttribute("aria-expanded", "false");

    await user.click(expander);

    expect(expander).toHaveAttribute("aria-expanded", "true");
    expect(document.querySelectorAll(".symbol-list li").length).toBeGreaterThan(
      0,
    );
  });
});

describe("the files tab remembers what you were doing (story 22)", () => {
  // The panel unmounts in two ways — switching to another tab, and folding
  // the whole sidebar away — and state kept inside it died with it both
  // times. These drive each gesture as a round trip and assert the panel
  // comes back as it was left. Nothing here asserts where the state lives;
  // hoisting is the mechanism, surviving the round trip is the behaviour.

  /** Open region headings until a row with a symbols expander is on screen —
   * not every region in the fixture holds one. */
  async function openUntilExpander(
    user: ReturnType<typeof userEvent.setup>,
  ): Promise<HTMLElement> {
    for (const toggle of screen
      .getAllByRole("button")
      .filter((b) => b.classList.contains("files-region-toggle"))) {
      await user.click(toggle);
      const expander = document.querySelector<HTMLElement>(".file-expand");
      if (expander !== null) {
        return expander;
      }
    }
    throw new Error("no region in the fixture contains a file with symbols");
  }

  it("keeps the filter across a tab round trip", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openFiles(user);

    await user.type(screen.getByLabelText("Filter files"), "src");
    const filtered = rows();
    expect(filtered.length).toBeGreaterThan(0);

    await user.click(screen.getByRole("tab", { name: "Info" }));
    await openFiles(user);

    // The text is still in the box and still filtering: same rows, not a
    // fresh unfiltered tab that happens to have a value somewhere.
    expect(screen.getByLabelText("Filter files")).toHaveValue("src");
    expect(rows()).toEqual(filtered);
  });

  it("keeps the folds and the open symbol list across a tab round trip", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openFiles(user);

    const expander = await openUntilExpander(user);
    const opened = rows();
    expect(opened.length).toBeGreaterThan(0);
    await user.click(expander);
    expect(document.querySelectorAll(".symbol-list li").length).toBeGreaterThan(0);

    await user.click(screen.getByRole("tab", { name: "Info" }));
    await openFiles(user);

    expect(rows()).toEqual(opened);
    expect(
      document.querySelector<HTMLElement>(".file-expand"),
    ).toHaveAttribute("aria-expanded", "true");
    expect(document.querySelectorAll(".symbol-list li").length).toBeGreaterThan(0);
  });

  it("keeps the filter across folding and unfolding the sidebar", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openFiles(user);

    await user.type(screen.getByLabelText("Filter files"), "src");
    const filtered = rows();
    expect(filtered.length).toBeGreaterThan(0);

    await user.click(screen.getByRole("button", { name: "Hide the side panel" }));
    expect(screen.queryByLabelText("Filter files")).toBeNull();
    await user.click(screen.getByRole("button", { name: /Panel/ }));

    // The FILES tab is still the selected tab — that state already lives in
    // the explorer — so the panel is back without another click, filter and
    // all.
    expect(screen.getByLabelText("Filter files")).toHaveValue("src");
    expect(rows()).toEqual(filtered);
  });

  it("keeps the folds and the open symbol list across folding and unfolding the sidebar", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openFiles(user);

    const expander = await openUntilExpander(user);
    const opened = rows();
    expect(opened.length).toBeGreaterThan(0);
    await user.click(expander);

    await user.click(screen.getByRole("button", { name: "Hide the side panel" }));
    await user.click(screen.getByRole("button", { name: /Panel/ }));

    expect(rows()).toEqual(opened);
    expect(
      document.querySelector<HTMLElement>(".file-expand"),
    ).toHaveAttribute("aria-expanded", "true");
  });
});

describe("the filter and the header's search are different things", () => {
  it("matches paths only, leaving summaries to the header's search", async () => {
    // Both boxes are on screen at once and the distinction is the only reason
    // having two is defensible: the header searches the whole map and takes
    // you somewhere, this narrows what is already in front of you. A filter
    // that also matched symbol names would be the header's box, in a worse
    // position, with no way to tell which one you were using.
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);
    await openFiles(user);

    // A word in a **file's own** summary and in no path.
    //
    // It has to be a file's summary, and the first version of this test got
    // that wrong: it used a word from a *function's* summary, which the filter
    // could never have matched whatever it did, because it only ever looks at
    // file rows. Tampering the filter to read summaries left the test green.
    // The word has to be one the tampered version would find.
    const word = "lines";
    const described = map.nodes.filter(
      (n) => n.kind === "file" && (n.summary ?? "").toLowerCase().includes(word),
    );
    expect(
      described.length,
      `no *file* summary says "${word}", so a filter reading summaries would still find nothing`,
    ).toBeGreaterThan(0);
    expect(
      map.nodes.every((n) => !n.path.toLowerCase().includes(word)),
      `"${word}" is in a path, so this proves nothing`,
    ).toBe(true);

    await user.type(screen.getByLabelText("Filter files"), word);

    expect(rows()).toEqual([]);
    expect(screen.getByText("No files match.")).toBeVisible();

    // And the header's box does find it, so this is a division of labour
    // rather than a hole. Asserting only the first half would be satisfied by
    // a filter that matched nothing at all.
    await user.type(screen.getByLabelText("Search nodes"), word);
    const results = within(screen.getByLabelText("Search results"));
    expect(results.queryByText("No matches")).toBeNull();
    expect(
      results.getAllByText(described[0]?.name ?? "").length,
    ).toBeGreaterThan(0);
  });
});
