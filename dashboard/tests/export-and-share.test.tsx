// Ticket 28 (spec story 8): there are two ways a map leaves the dashboard,
// and the UI has to say which is which. Export hands over the graph as data
// against the published contract; `codeatlas share` writes the page a person
// can open with nothing installed. These tests are about the words around
// both routes — and about the one thing the words have to warn of, which is
// that the JSON is not redacted and the shared page is.
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { App } from "../src/app/App.js";
import { mapFilename } from "../src/app/export.js";
import { MapExplorer } from "../src/app/MapExplorer.js";
import { SHARE_DATA_ID } from "../src/app/share.js";
import { openRegion } from "./drive.js";
import smallMap from "./fixtures/small-map.json";

const map = smallMap as KnowledgeGraph;

/** Rewrites every prose slot's provenance in one sweep — the four
 * collections the share allowlist redacts, in the one place that knows they
 * are four. */
function withProvenance(
  from: KnowledgeGraph,
  provenance: "structural" | "llm",
): KnowledgeGraph {
  return {
    ...from,
    nodes: from.nodes.map((n) => ({ ...n, provenance })),
    layers: (from.layers ?? []).map((l) => ({ ...l, provenance })),
    domain_flows: (from.domain_flows ?? []).map((f) => ({ ...f, provenance })),
    tour: (from.tour ?? []).map((s) => ({ ...s, provenance })),
  };
}

/** Nothing enriched, so a share artifact of it would redact nothing. */
const structural = withProvenance(map, "structural");

/** Everything enriched. `small-map.json` on its own enriches one node and
 * one layer, which would let the warning count only two of the four
 * redactable collections and still look right — so the counting test uses a
 * map where each of the four contributes. */
const allEnriched = withProvenance(map, "llm");

/** Written out rather than summed, so that dropping a collection from the
 * implementation's list changes the expected number instead of changing both
 * sides of the comparison together. `small-map.json`: 5 nodes, 2 layers,
 * 1 flow, 1 tour step. */
const ALL_ENRICHED_SLOTS = 9;

type User = ReturnType<typeof userEvent.setup>;

async function openMenu(user: User) {
  await user.click(screen.getByRole("button", { name: /export/i }));
  return within(screen.getByRole("group", { name: /share or export/i }));
}

describe("the share route", () => {
  it("names itself in the top bar, before anything is clicked", () => {
    render(<MapExplorer map={map} />);

    // The whole complaint was that the route lived in a hover tooltip. The
    // word has to be in the chrome, not only behind a press.
    expect(
      screen.getByRole("button", { name: "Share / Export" }),
    ).toBeVisible();
  });

  it("names the self-contained page in the UI rather than in a tooltip", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    const menu = await openMenu(user);
    // The question this ticket came from — "I see it exports a json file,
    // what do I do with it?" — is answered by text on screen or not at all.
    expect(menu.getByRole("heading", { name: /share a page/i })).toBeVisible();
    expect(menu.getByText(/self-contained/i)).toBeVisible();
    expect(menu.getByText(/nothing installed/i)).toBeVisible();
    expect(menu.getByText(/prose is redacted/i)).toBeVisible();
    // A command whose output the reader cannot find has not told them
    // anything, and the subcommand's path argument defaults to the cwd.
    expect(menu.getByText(/self-contained/i)).toHaveTextContent(
      ".codeatlas/share.html",
    );
    expect(menu.getByText(/repository root/i)).toBeVisible();
  });

  it("shows the command as text and copies it, without running anything", async () => {
    const user = userEvent.setup();
    const fetchSpy = vi.fn();
    vi.stubGlobal("fetch", fetchSpy);
    try {
      render(<MapExplorer map={map} />);

      const menu = await openMenu(user);
      // Visible either way: a copy button that fails leaves a reader who can
      // still read the command and type it.
      expect(menu.getByText("codeatlas share")).toBeVisible();

      await user.click(menu.getByRole("button", { name: /copy/i }));
      expect(await navigator.clipboard.readText()).toBe("codeatlas share");
      expect(menu.getByRole("status")).toHaveTextContent(/copied/i);
      // ADR-0006: the dashboard has no shell and makes no requests. The only
      // thing it can do with a command is show it.
      expect(fetchSpy).not.toHaveBeenCalled();
    } finally {
      vi.unstubAllGlobals();
    }
  });
});

describe("the export route", () => {
  it("says the JSON is the map as data, against the published contract", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    const menu = await openMenu(user);
    expect(menu.getByRole("heading", { name: /download the data/i })).toBeVisible();
    expect(menu.getByText(/contract/i)).toHaveTextContent(map.version);
  });

  it("warns that the JSON is unredacted when the map carries enriched prose", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    // small-map.json enriches one node summary and one layer name.
    const menu = await openMenu(user);
    expect(menu.getByText(/not redacted/i)).toHaveTextContent(
      /\b2 LLM-written prose fields\b/,
    );
  });

  it("counts every kind of prose slot the share allowlist would redact", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={allEnriched} />);

    // Four collections carry redactable prose — node summaries, layer names,
    // flow names, tour narration — and a warning that counted only the
    // obvious one would understate what the file leaks.
    const menu = await openMenu(user);
    expect(menu.getByText(/not redacted/i)).toHaveTextContent(
      new RegExp(`\\b${ALL_ENRICHED_SLOTS} LLM-written prose fields\\b`),
    );
  });

  it("counts a purchased layer description as a slot of its own", async () => {
    const user = userEvent.setup();
    // Ticket 06: the description's provenance is separate from the name's,
    // so a purchased description on an otherwise-structural layer is one
    // more field the JSON leaks — and a count keyed on `layer.provenance`
    // alone would miss every one of them.
    const described: KnowledgeGraph = {
      ...structural,
      layers: (structural.layers ?? []).map((layer) => ({
        ...layer,
        description: { text: "Purchased prose", provenance: "llm" as const },
      })),
    };
    render(<MapExplorer map={described} />);

    // Two layers, two purchased descriptions, nothing else enriched.
    const menu = await openMenu(user);
    expect(menu.getByText(/not redacted/i)).toHaveTextContent(
      /\b2 LLM-written prose fields\b/,
    );
  });

  it("says nothing about redaction when there is no enriched prose", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={structural} />);

    const menu = await openMenu(user);
    expect(menu.queryByText(/not redacted/i)).not.toBeInTheDocument();
    expect(menu.getByRole("heading", { name: /download the data/i })).toBeVisible();
  });

  it("still downloads exactly the map it was given", async () => {
    const user = userEvent.setup();
    const blobs: Blob[] = [];
    const realCreate = URL.createObjectURL;
    const realRevoke = URL.revokeObjectURL;
    URL.createObjectURL = vi.fn((blob: Blob | MediaSource) => {
      blobs.push(blob as Blob);
      return "blob:test";
    });
    URL.revokeObjectURL = vi.fn();
    let downloaded = "";
    const click = vi
      .spyOn(HTMLAnchorElement.prototype, "click")
      .mockImplementation(function (this: HTMLAnchorElement) {
        downloaded = this.download;
      });

    try {
      render(<MapExplorer map={map} />);
      const menu = await openMenu(user);
      await user.click(menu.getByRole("button", { name: /download json/i }));

      expect(downloaded).toBe(mapFilename(map));
      expect(blobs).toHaveLength(1);
      // Criterion: this ticket changes what the UI says, not what it emits.
      expect(await blobs.at(0)?.text()).toBe(JSON.stringify(map, null, 2));
    } finally {
      click.mockRestore();
      URL.createObjectURL = realCreate;
      URL.revokeObjectURL = realRevoke;
    }
  });
});

describe("in a share artifact", () => {
  const payload = {
    map: {
      ...map,
      nodes: map.nodes.map((n) =>
        n.provenance === "llm" ? { ...n, summary: "[redacted]" } : n,
      ),
    },
    redaction: {
      marker: "[redacted]",
      policy: ["Node.summary"],
      redacted: [{ field: "Node.summary", count: 2 }],
    },
  };
  const realFetch = globalThis.fetch;

  beforeEach(() => {
    const script = document.createElement("script");
    script.id = SHARE_DATA_ID;
    script.type = "application/json";
    script.textContent = JSON.stringify(payload);
    document.head.append(script);
    // Same premise as share-mode.test.tsx: a double-clicked file:// artifact
    // has no usable fetch, so any attempt to use one crashes the test.
    // @ts-expect-error simulating a runtime without usable fetch
    delete globalThis.fetch;
  });

  afterEach(() => {
    document.getElementById(SHARE_DATA_ID)?.remove();
    globalThis.fetch = realFetch;
  });

  it("advertises no command its reader has no way to run", async () => {
    const user = userEvent.setup();
    render(<App />);

    // Down to the button: naming a route in the chrome that the panel then
    // does not offer is the tooltip problem again, one level up.
    expect(screen.getByRole("button", { name: "Export" })).toBeVisible();
    expect(
      screen.queryByRole("button", { name: "Share / Export" }),
    ).not.toBeInTheDocument();

    const menu = await openMenu(user);
    expect(menu.queryByText(/codeatlas share/)).not.toBeInTheDocument();
    expect(
      menu.queryByRole("heading", { name: /share a page/i }),
    ).not.toBeInTheDocument();
    // The data route is still here — the reader can still take the map away.
    expect(
      menu.getByRole("heading", { name: /download the data/i }),
    ).toBeVisible();
  });

  it("does not call its already-redacted payload unredacted", async () => {
    const user = userEvent.setup();
    render(<App />);

    // The nodes still carry `llm` provenance — the share allowlist keeps
    // provenance and replaces the prose — so a warning keyed on provenance
    // alone would fire here and be false.
    const menu = await openMenu(user);
    expect(menu.queryByText(/not redacted/i)).not.toBeInTheDocument();
  });
});

describe("the menu itself", () => {
  it("closes on Escape without stepping back through the UI", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await openRegion(user, "Source code");
    expect(screen.getByTestId("back")).toBeVisible();

    const menu = await openMenu(user);
    expect(menu.getByRole("heading", { name: /share a page/i })).toBeVisible();
    await user.keyboard("{Escape}");

    expect(
      screen.queryByRole("group", { name: /share or export/i }),
    ).not.toBeInTheDocument();
    // Ticket 22's lesson: one Escape, one layer. The menu is the innermost
    // thing open, so the drill-in must survive closing it.
    expect(screen.getByTestId("back")).toBeVisible();
  });

  it("hands focus back to its own button rather than to the document", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    const toggle = screen.getByRole("button", { name: "Share / Export" });
    const menu = await openMenu(user);
    await user.click(menu.getByRole("button", { name: /copy/i }));
    await user.keyboard("{Escape}");

    // Without this the focused button is removed with the panel, focus falls
    // to `<body>`, and the reader's next Tab restarts from the top of the
    // document — a long way back for a keyboard-only reader.
    expect(toggle).toHaveFocus();
  });

  it("does not seize focus when the page loads", () => {
    render(<MapExplorer map={map} />);

    // The restore runs on every change of the open flag, and at mount the
    // flag is false while focus is legitimately on `<body>` — the two
    // conditions the restore looks for. Nothing should move.
    expect(
      screen.getByRole("button", { name: "Share / Export" }),
    ).not.toHaveFocus();
    expect(document.body).toHaveFocus();
  });

  it("leaves focus where the reader put it, if they tabbed out of the panel", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    // Tab straight through the panel's own controls and out the far side.
    // The menu is still open, but the reader is no longer in it — so closing
    // it must not drag them back to the toggle.
    await openMenu(user);
    const path = screen.getByRole("button", { name: "Path" });
    await user.tab();
    await user.tab();
    await user.tab();
    expect(path).toHaveFocus();

    await user.keyboard("{Escape}");
    expect(path).toHaveFocus();
  });

  it("leaves focus alone when the reader dismissed it by clicking elsewhere", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await openMenu(user);
    const search = screen.getByLabelText("Search nodes");
    await user.click(search);

    // Restoring focus unconditionally would yank it out of whatever the
    // reader just clicked, which is worse than the problem it fixes.
    expect(search).toHaveFocus();
  });

  it("forgets that it copied once it has been closed", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    let menu = await openMenu(user);
    await user.click(menu.getByRole("button", { name: /copy/i }));
    expect(menu.getByRole("status")).toHaveTextContent(/copied/i);

    await user.keyboard("{Escape}");
    menu = await openMenu(user);
    // Stale confirmation is worse than none: it says a press happened that
    // did not.
    expect(menu.getByRole("status")).toBeEmptyDOMElement();
  });

  it("closes when the reader clicks outside it", async () => {
    const user = userEvent.setup();
    render(<MapExplorer map={map} />);

    await openMenu(user);
    await user.click(screen.getByLabelText("Search nodes"));

    expect(
      screen.queryByRole("group", { name: /share or export/i }),
    ).not.toBeInTheDocument();
  });
});
