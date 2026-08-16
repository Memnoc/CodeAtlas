// Ticket 02 of V3 (ADR-0013), the dashboard half of open code: selecting a
// node offers opening its source, and the offer exists exactly when the
// capabilities route says the serving binary was started with `--open-code`.
//
// Seam 4, gesture→state at the jsdom boundary, driven through `<App/>` with
// `fetch` stubbed — the same shape as `ask.test.tsx`, because the two
// features gate the same way: a route of the binary's, discovered at load,
// never probed. The stub throws on any URL nothing serves, so a request
// nobody meant to make fails the test rather than leaving it. Where the
// source column *is* (a workspace sibling of the canvas) is state jsdom can
// see; how wide it paints is the stylesheet contract's half
// (`stylesheet-contract.test.ts`), the split the conversation column cut.
import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { App } from "../src/app/App.js";
import { CAPABILITIES_ROUTE } from "../src/app/ask.js";
import { SOURCE_ROUTE, type SourceEnvelope } from "../src/app/source.js";
import { SHARE_DATA_ID } from "../src/app/share.js";
import { openRegion, selectedOnCanvas } from "./drive.js";
import smallMap from "./fixtures/small-map.json";

const map = smallMap as KnowledgeGraph;

/** Enough of a `Response` for the call sites that read one. */
function response(status: number, body: unknown): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
  } as unknown as Response;
}

type Reply = { status: number; body: unknown };

/** Twelve honest lines, so a lit range has unlit lines on both sides. */
const TWELVE_LINES = Array.from(
  { length: 12 },
  (_, i) => `line ${i + 1} of main.ts`,
).join("\n");

/** A stand-in for `codeatlas serve`: the map, no diff overlay, a capability
 * answer for both flags, and — when the test scripts one — the source route
 * itself. */
function servedBy(options: {
  open_code: boolean;
  source?: (id: string) => Reply;
}) {
  const fetchStub = vi.fn(async (input: RequestInfo | URL) => {
    const url = String(input);
    if (url === "/api/map") {
      return response(200, map);
    }
    if (url === "/api/diff") {
      return response(404, { error: "no diff overlay" });
    }
    if (url === CAPABILITIES_ROUTE) {
      return response(200, { ask: false, open_code: options.open_code });
    }
    if (url.startsWith(`${SOURCE_ROUTE}?`)) {
      const id = new URLSearchParams(url.split("?")[1]).get("id") ?? "";
      const reply = (options.source ??
        (() => ({
          status: 200,
          body: {
            source: TWELVE_LINES,
            path: "src/main.ts",
            truncated: false,
          } satisfies SourceEnvelope,
        })))(id);
      return response(reply.status, reply.body);
    }
    throw new Error(`the dashboard requested ${url}, which nothing serves`);
  });
  vi.stubGlobal("fetch", fetchStub);
  return fetchStub;
}

/** Renders the app and waits for the served map to arrive. */
async function servedDashboard() {
  render(<App />);
  await screen.findByLabelText("Search nodes");
}

/** Selects a node through the search overlay — the same place as pressing
 * the card, without pressing the card: React Flow's d3-drag reads
 * `event.view.document` on mousedown, which jsdom leaves null
 * (`magnify.test.tsx` records the same detour). */
async function selectViaSearch(
  user: ReturnType<typeof userEvent.setup>,
  text: string,
): Promise<void> {
  await user.type(screen.getByLabelText("Search nodes"), text);
  await user.click(
    within(screen.getByLabelText("Search results")).getByText(text),
  );
  await user.clear(screen.getByLabelText("Search nodes"));
}

const detail = () => within(screen.getByLabelText("Node detail"));
const openControl = () => detail().getByRole("button", { name: "Open code" });

/** The source-route requests actually made. */
function sourceCalls(stub: ReturnType<typeof servedBy>): string[] {
  return stub.mock.calls
    .map(([url]) => String(url))
    .filter((url) => url.startsWith(SOURCE_ROUTE));
}

const realFetch = globalThis.fetch;

afterEach(() => {
  vi.unstubAllGlobals();
  globalThis.fetch = realFetch;
  document.getElementById(SHARE_DATA_ID)?.remove();
});

describe("the open affordance, exactly when the binary offers open code", () => {
  it("offers opening code on a node selected in the drill view", async () => {
    const user = userEvent.setup();
    servedBy({ open_code: true });
    await servedDashboard();

    await openRegion(user, "Source code");
    await selectViaSearch(user, "main.ts");

    expect(openControl()).toBeVisible();
  });

  it("does not exist when the binary was started without --open-code", async () => {
    // Runtime discovery, not a build-time constant: the same dashboard
    // bytes serve both, and the capability route is what tells them apart.
    // Absent rather than disabled — a control that can never work is not a
    // control (ADR-0013's affordance rule).
    const user = userEvent.setup();
    const fetchStub = servedBy({ open_code: false });
    await servedDashboard();

    await selectViaSearch(user, "main.ts");

    expect(screen.getByLabelText("Node detail")).toBeInTheDocument();
    expect(
      detail().queryByRole("button", { name: "Open code" }),
    ).not.toBeInTheDocument();
    expect(sourceCalls(fetchStub)).toEqual([]);
  });
});

describe("opening a file node", () => {
  it("renders its source beside the map, losing neither map nor selection", async () => {
    const user = userEvent.setup();
    servedBy({ open_code: true });
    await servedDashboard();
    await openRegion(user, "Source code");
    await selectViaSearch(user, "main.ts");

    await user.click(openControl());

    // The source, in a column docked in the workspace as the canvas's
    // sibling — the conversation column's own docking, because the point is
    // the same: the code and the map that named it on screen together.
    const column = await screen.findByLabelText("Source");
    expect(within(column).getByText("line 1 of main.ts")).toBeVisible();
    expect(within(column).getByText("src/main.ts")).toBeVisible();
    const workspace = document.querySelector(".workspace");
    expect(column.parentElement).toBe(workspace);
    expect(column.previousElementSibling).toBe(
      workspace?.querySelector("main.canvas"),
    );
    // The map is still there to use, and the selection that opened this is
    // still standing: opening is a reading column, not a navigation step.
    expect(
      document.querySelector('.react-flow__node[data-id="file:src/main.ts"]'),
    ).not.toBeNull();
    expect(selectedOnCanvas()).toBe("file:src/main.ts");
    expect(screen.getByLabelText("Node detail")).toBeInTheDocument();
  });

  it("asks the wire for the file by node id, percent-encoded", async () => {
    // The route's own contract (ticket 01): `GET /api/source?id=<node-id>`,
    // the id encoded because it honestly carries `:` and `/`.
    const user = userEvent.setup();
    const fetchStub = servedBy({ open_code: true });
    await servedDashboard();
    await selectViaSearch(user, "main.ts");

    await user.click(openControl());
    await screen.findByLabelText("Source");

    expect(sourceCalls(fetchStub)).toEqual([
      `${SOURCE_ROUTE}?id=file%3Asrc%2Fmain.ts`,
    ]);
  });
});

describe("opening a symbol", () => {
  /** jsdom has no `scrollIntoView`; the panel guards for that, so the spy
   * is both the stand-in and the assertion — `walkthrough.test.tsx`'s own
   * detour. */
  function spyOnScrolling(): ReturnType<typeof vi.fn> {
    const scrollIntoView = vi.fn();
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      writable: true,
      value: scrollIntoView,
    });
    return scrollIntoView;
  }

  afterEach(() => {
    delete (HTMLElement.prototype as { scrollIntoView?: unknown })
      .scrollIntoView;
  });

  it("resolves to its containing file client-side and lands lit at its range", async () => {
    // The wire speaks file nodes only (ADR-0013): the map already knows
    // which file holds `main`, so the request names `file:src/main.ts`, and
    // the symbol's own `range` (lines 3–9 in the fixture) is what the panel
    // lights and scrolls to.
    const user = userEvent.setup();
    const scrolled = spyOnScrolling();
    const fetchStub = servedBy({ open_code: true });
    await servedDashboard();
    await selectViaSearch(user, "main");

    await user.click(openControl());
    const column = within(await screen.findByLabelText("Source"));

    expect(sourceCalls(fetchStub)).toEqual([
      `${SOURCE_ROUTE}?id=file%3Asrc%2Fmain.ts`,
    ]);
    // Lit exactly the contract's 1-based inclusive range: 3 through 9, with
    // unlit lines standing on both sides.
    const lit = [...document.querySelectorAll(".source-line-lit")].map((el) =>
      el.getAttribute("data-line"),
    );
    expect(lit).toEqual(["3", "4", "5", "6", "7", "8", "9"]);
    expect(
      column.getByText("line 2 of main.ts").classList.contains(
        "source-line-lit",
      ),
    ).toBe(false);
    // And landed there: the panel asked the browser to bring the range's
    // first line into view.
    expect(scrolled).toHaveBeenCalled();
    const landedOn = scrolled.mock.instances.at(-1) as HTMLElement;
    expect(landedOn.getAttribute("data-line")).toBe("3");
    // Which lines are lit is said in prose too, for the reader who scrolls.
    expect(column.getByText(/lines 3–9/)).toBeVisible();
  });

  it("opens a file node at its top, nothing lit and no scroll", async () => {
    // The contrast that keeps the range a statement about symbols: a file
    // opens as a file, from line one.
    const user = userEvent.setup();
    const scrolled = spyOnScrolling();
    servedBy({ open_code: true });
    await servedDashboard();
    await selectViaSearch(user, "main.ts");

    await user.click(openControl());
    await screen.findByLabelText("Source");

    expect(document.querySelector(".source-line-lit")).toBeNull();
    expect(scrolled).not.toHaveBeenCalled();
  });
});

describe("the envelope's two disclosures", () => {
  it("renders a truncated envelope's notice visibly, above source still shown", async () => {
    // The server cuts past its cap rather than refusing (ADR-0013), so the
    // panel's half of that honesty is a notice the reader cannot miss — a
    // file that ends mid-thought must read as cut, never as complete.
    const user = userEvent.setup();
    servedBy({
      open_code: true,
      source: () => ({
        status: 200,
        body: {
          source: "the beginning of something much larger",
          path: "src/main.ts",
          truncated: true,
        },
      }),
    });
    await servedDashboard();
    await selectViaSearch(user, "main.ts");

    await user.click(openControl());
    const column = within(await screen.findByLabelText("Source"));

    expect(column.getByText(/truncated/i)).toBeVisible();
    // Disclosed, not refused: what was served is still there to read.
    expect(
      column.getByText("the beginning of something much larger"),
    ).toBeVisible();
  });

  it("shows a deleted file's 404 as the server's own words, never an empty panel", async () => {
    // A mapped file deleted since the scan draws an honest 404 from the
    // wire (ticket 01); the panel's job is to keep it honest on screen —
    // a control that silently does nothing is the worst way to be wrong.
    const user = userEvent.setup();
    servedBy({
      open_code: true,
      source: () => ({
        status: 404,
        body: {
          error:
            "src/main.ts is in the map but no longer on disk — re-run " +
            "`codeatlas scan` for a map that says so",
        },
      }),
    });
    await servedDashboard();
    await selectViaSearch(user, "main.ts");

    await user.click(openControl());
    const column = within(await screen.findByLabelText("Source"));

    expect(await column.findByRole("alert")).toHaveTextContent(
      /no longer on disk — re-run `codeatlas scan`/,
    );
    // Still a panel with a way out, not a dead end.
    expect(
      column.getByRole("button", { name: "Close the source" }),
    ).toBeVisible();
  });
});

describe("putting the source away", () => {
  it("closes on its own control, keeping the selection, returning focus", async () => {
    const user = userEvent.setup();
    servedBy({ open_code: true });
    await servedDashboard();
    await selectViaSearch(user, "main.ts");
    await user.click(openControl());
    await screen.findByLabelText("Source");

    await user.click(screen.getByRole("button", { name: "Close the source" }));

    expect(screen.queryByLabelText("Source")).not.toBeInTheDocument();
    // The selection survives its reading matter being put away…
    expect(selectedOnCanvas()).toBe("file:src/main.ts");
    // …and the keyboard lands back on the control that opened it (ticket
    // 17's focus-return discipline), ready to open it again.
    expect(document.activeElement).toBe(openControl());
  });

  it("closes through the one Escape cascade, before the step back", async () => {
    const user = userEvent.setup();
    servedBy({ open_code: true });
    await servedDashboard();
    await openRegion(user, "Source code");
    await selectViaSearch(user, "main.ts");
    await user.click(openControl());
    await screen.findByLabelText("Source");

    await user.keyboard("{Escape}");

    // One layer per press: the column goes, the selection stays.
    expect(screen.queryByLabelText("Source")).not.toBeInTheDocument();
    expect(selectedOnCanvas()).toBe("file:src/main.ts");
  });
});

describe("the other places a node is already selected", () => {
  it("offers opening under the magnify lens, and opening keeps the lens", async () => {
    const user = userEvent.setup();
    servedBy({ open_code: true });
    await servedDashboard();
    await openRegion(user, "Source code");
    await selectViaSearch(user, "main.ts");

    await user.click(screen.getByRole("button", { name: "Magnify main.ts" }));

    // The lens draws the neighbourhood; the selection's detail — and with
    // it the offer — is still standing.
    await user.click(openControl());
    expect(await screen.findByLabelText("Source")).toBeInTheDocument();
    // Still magnified: opening is a reading column, not a navigation step,
    // so the lens the reader ground is not taken from them.
    expect(screen.getByTestId("back")).toHaveTextContent(
      /back to Source code/i,
    );
    expect(selectedOnCanvas()).toBe("file:src/main.ts");
  });

  it("offers opening on a symbol picked in the Files panel, landing lit", async () => {
    const user = userEvent.setup();
    const fetchStub = servedBy({ open_code: true });
    await servedDashboard();

    await user.click(screen.getByRole("tab", { name: "Files" }));
    const srcFiles = within(
      screen.getByLabelText("Files in Source code", { selector: "section" }),
    );
    await user.click(srcFiles.getByRole("button", { name: /Source code/ }));
    await user.click(
      srcFiles.getByRole("button", {
        name: "Show the 1 symbols in src/main.ts",
      }),
    );
    await user.click(srcFiles.getByRole("button", { name: /function\s*main/ }));

    await user.click(openControl());
    await screen.findByLabelText("Source");

    // The same client-side roll-up as everywhere else: the wire was asked
    // for the containing file, and the symbol's range is lit.
    expect(sourceCalls(fetchStub)).toEqual([
      `${SOURCE_ROUTE}?id=file%3Asrc%2Fmain.ts`,
    ]);
    expect(document.querySelectorAll(".source-line-lit").length).toBe(7);
  });

  it("is absent in a share artifact, which has no server to serve source", async () => {
    // ADR-0013's trust boundary: a share recipient is precisely someone who
    // does not hold the code. The artifact runs with no usable fetch at
    // all, so the capability reads as off by construction — asserted here
    // rather than assumed from it.
    const user = userEvent.setup();
    const script = document.createElement("script");
    script.id = SHARE_DATA_ID;
    script.type = "application/json";
    script.textContent = JSON.stringify({
      map,
      redaction: {
        marker: "[redacted]",
        policy: ["Node.summary"],
        redacted: [],
      },
    });
    document.head.append(script);
    // @ts-expect-error simulating a runtime without usable fetch
    delete globalThis.fetch;

    render(<App />);
    await screen.findByLabelText("Search nodes");
    await selectViaSearch(user, "main.ts");

    expect(screen.getByLabelText("Node detail")).toBeInTheDocument();
    expect(
      detail().queryByRole("button", { name: "Open code" }),
    ).not.toBeInTheDocument();
  });
});
