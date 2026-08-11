// Story 20, ticket 26: a walkthrough of the *application* — one control at a
// time, highlighted where it really sits, with a sentence saying what it does.
//
// Driven through `<MapExplorer/>` with real user events, the shape the spec's
// testing decisions name for this story. Three properties here are the ones
// jsdom makes easy to fake and hard to prove, so each is asserted through the
// mechanism itself rather than through an attribute that implies it:
//
//   - *inert to the keyboard* — asserted by tabbing and looking at where focus
//     actually went, because neither jsdom nor user-event implements `inert`
//     (`utils/focus/selector.js` has no such clause), so a test that read the
//     attribute would pass over a page whose background is fully tabbable.
//   - *the highlight follows a reflow* — asserted by driving the observation
//     mechanism: a controllable `ResizeObserver` and a rect that the test
//     changes between reads, so a component that measures once fails.
//   - *reduced motion* — asserted on the argument passed to `scrollIntoView`,
//     which is observable; the transition itself is CSS, which jsdom does not
//     run and this file does not pretend to check.
import { act, fireEvent, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { KnowledgeGraph } from "../src/index.js";
import { MapExplorer } from "../src/app/MapExplorer.js";
import {
  WALKTHROUGH_MARKER,
  WALKTHROUGH_SEEN_KEY,
  WALKTHROUGH_STEPS,
} from "../src/app/walkthrough.js";
import type { DiffOverlay } from "../src/app/overlay.js";
import { openLearn } from "./drive.js";
import tourMap from "./fixtures/tour-map.json";
import smallOverlay from "./fixtures/small-overlay.json";

const map = tourMap as KnowledgeGraph;
const overlay = smallOverlay as DiffOverlay;

/** A question backend that is never called: its presence is what puts the Ask
 * control on screen, which is one of the two conditional steps. */
const neverAsked = async () => ({ answer: "", citations: [] });

/** The whole interface, every optional control present — the render the two
 * drift guards below measure the step list against. */
function renderEverything() {
  return render(
    <MapExplorer map={map} overlay={overlay} onAsk={neverAsked} />,
  );
}

async function startWalkthrough(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole("button", { name: "Walkthrough" }));
}

const walkthrough = () =>
  screen.getByRole("dialog", { name: "Interface walkthrough" });

/** The live elements the walkthrough is currently pointing at. Never a copy:
 * the attribute is put on the real control by the running walkthrough. */
function litElements(): HTMLElement[] {
  return [...document.querySelectorAll<HTMLElement>("[data-walkthrough-lit]")];
}

/** The one live element the current step is about. */
function litElement(): HTMLElement {
  const lit = litElements();
  if (lit.length !== 1) {
    throw new Error(`${lit.length} elements lit at once, expected exactly 1`);
  }
  return lit[0] as HTMLElement;
}

/** Which step marker the walkthrough is standing on. */
function litId(): string | null {
  return litElement().getAttribute(WALKTHROUGH_MARKER);
}

/** Markers rendered by the interface as it currently stands. */
function markersOnScreen(): string[] {
  return [
    ...document.querySelectorAll<HTMLElement>(`[${WALKTHROUGH_MARKER}]`),
  ].map((el) => el.getAttribute(WALKTHROUGH_MARKER) ?? "");
}

const realFetch = globalThis.fetch;

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
  globalThis.fetch = realFetch;
});

describe("starting the walkthrough", () => {
  it("waits to be asked, and remembers that it offered", async () => {
    // Not sprung on the reader: the modal never opens by itself. What a first
    // visit gets is a callout beside the control, with a way to decline, and
    // declining is remembered — locally, which is the only place it may be
    // remembered at all (ADR-0006).
    const user = userEvent.setup();
    const fetchStub = vi.fn(() => {
      throw new Error("the explorer must make no request");
    });
    vi.stubGlobal("fetch", fetchStub);
    const first = renderEverything();

    expect(
      screen.queryByRole("dialog", { name: "Interface walkthrough" }),
    ).not.toBeInTheDocument();
    const callout = screen.getByRole("note", { name: "First visit" });
    expect(localStorage.getItem(WALKTHROUGH_SEEN_KEY)).toBeNull();

    await user.click(within(callout).getByRole("button", { name: "Not now" }));

    expect(
      screen.queryByRole("note", { name: "First visit" }),
    ).not.toBeInTheDocument();
    expect(localStorage.getItem(WALKTHROUGH_SEEN_KEY)).not.toBeNull();
    expect(fetchStub).not.toHaveBeenCalled();

    // And it stays declined on the next visit, while the control itself does
    // not go anywhere.
    first.unmount();
    renderEverything();
    expect(
      screen.queryByRole("note", { name: "First visit" }),
    ).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Walkthrough" })).toBeVisible();
  });

  it("starts from the callout as well as from the control", async () => {
    const user = userEvent.setup();
    renderEverything();

    await user.click(
      within(screen.getByRole("note", { name: "First visit" })).getByRole(
        "button",
        { name: "Show me around" },
      ),
    );

    expect(walkthrough()).toBeInTheDocument();
    expect(localStorage.getItem(WALKTHROUGH_SEEN_KEY)).not.toBeNull();
  });

  it("survives storage that refuses, as a double-clicked artifact does", async () => {
    // Same posture as the theme: some browsers give a `file://` page an opaque
    // origin where touching localStorage throws. The walkthrough is worth more
    // than the memory of having offered it.
    vi.stubGlobal("localStorage", {
      getItem() {
        throw new DOMException("denied", "SecurityError");
      },
      setItem() {
        throw new DOMException("denied", "SecurityError");
      },
    });
    const user = userEvent.setup();
    renderEverything();

    await startWalkthrough(user);
    expect(walkthrough()).toBeInTheDocument();
  });
});

describe("the walkthrough itself", () => {
  it("highlights one real element of the live interface at a time", async () => {
    const user = userEvent.setup();
    renderEverything();
    const identity = document.querySelector<HTMLElement>(
      `[${WALKTHROUGH_MARKER}="identity"]`,
    );

    await startWalkthrough(user);

    // The lit element is the one already on the page — not a picture of it,
    // not a copy inside the dialog.
    expect(litElement()).toBe(identity);
    expect(walkthrough().contains(litElement())).toBe(false);
    expect(document.querySelector(".explorer")?.contains(litElement())).toBe(
      true,
    );
    // And the cut-out is placed on that element rather than drawn abstractly.
    expect(
      walkthrough().querySelector(".walkthrough-spotlight"),
    ).toBeInTheDocument();

    await user.click(within(walkthrough()).getByRole("button", { name: "Next" }));
    expect(litElements()).toHaveLength(1);
    expect(litElement()).not.toBe(identity);
  });

  it("carries forward, back, and a dismissal reachable at every step", async () => {
    const user = userEvent.setup();
    renderEverything();
    await startWalkthrough(user);
    const steps = WALKTHROUGH_STEPS.length;
    const panel = () => within(walkthrough());

    const visited: string[] = [];
    for (let i = 1; i <= steps; i++) {
      expect(panel().getByText(`Step ${i} of ${steps}`)).toBeInTheDocument();
      // The way out is on screen at every single step, not only at the end.
      expect(
        panel().getByRole("button", { name: "Close walkthrough" }),
      ).toBeVisible();
      visited.push(litId() ?? "");
      if (i < steps) {
        await user.click(panel().getByRole("button", { name: "Next" }));
      }
    }
    expect(visited).toEqual(WALKTHROUGH_STEPS.map((s) => s.id));

    // Back really goes back, rather than only lighting up.
    await user.click(panel().getByRole("button", { name: "Back" }));
    expect(panel().getByText(`Step ${steps - 1} of ${steps}`)).toBeVisible();
    expect(litId()).toBe(WALKTHROUGH_STEPS[steps - 2]?.id);

    await user.click(panel().getByRole("button", { name: "Close walkthrough" }));
    expect(
      screen.queryByRole("dialog", { name: "Interface walkthrough" }),
    ).not.toBeInTheDocument();
    // Nothing left lit on a page nobody is being walked through.
    expect(litElements()).toHaveLength(0);
  });

  it("stops at both ends of the walk", async () => {
    const user = userEvent.setup();
    renderEverything();
    await startWalkthrough(user);
    const panel = () => within(walkthrough());

    expect(panel().getByRole("button", { name: "Back" })).toBeDisabled();
    for (let i = 1; i < WALKTHROUGH_STEPS.length; i++) {
      await user.click(panel().getByRole("button", { name: "Next" }));
    }
    // The last step's forward control is the way out, so there is never a
    // dead press at the end of the walk.
    expect(panel().queryByRole("button", { name: "Next" })).toBeNull();
    await user.click(panel().getByRole("button", { name: "Done" }));
    expect(
      screen.queryByRole("dialog", { name: "Interface walkthrough" }),
    ).not.toBeInTheDocument();
  });

  it("returns focus to the control that opened it", async () => {
    const user = userEvent.setup();
    renderEverything();
    await startWalkthrough(user);

    await user.click(
      within(walkthrough()).getByRole("button", { name: "Close walkthrough" }),
    );

    expect(screen.getByRole("button", { name: "Walkthrough" })).toHaveFocus();
  });
});

describe("the keyboard while the walkthrough runs", () => {
  it("moves focus to the step content, and again on every step", async () => {
    const user = userEvent.setup();
    renderEverything();

    await startWalkthrough(user);

    const card = walkthrough().querySelector(".walkthrough-card");
    expect(document.activeElement).toBe(card);
    // The step text is what focus lands on, so a screen reader reads the
    // explanation rather than the button that produced it.
    expect(card).toHaveTextContent(WALKTHROUGH_STEPS[0]?.title ?? "");

    await user.click(within(walkthrough()).getByRole("button", { name: "Next" }));
    expect(document.activeElement).toBe(card);
    expect(card).toHaveTextContent(WALKTHROUGH_STEPS[1]?.title ?? "");
  });

  it("leaves nothing behind it reachable by tabbing", async () => {
    // The property, driven rather than described: tab all the way round and
    // look at where focus actually went. Asserting an `inert` attribute would
    // pass here whatever the page did, because neither jsdom nor user-event
    // implements it.
    const user = userEvent.setup();
    renderEverything();
    const explorer = document.querySelector(".explorer");
    const background = [
      screen.getByLabelText("Search nodes"),
      screen.getByRole("button", { name: "Path" }),
      screen.getByRole("button", { name: "Walkthrough" }),
    ];
    expect(explorer).not.toHaveAttribute("inert");

    await startWalkthrough(user);

    // The declaration a real browser acts on, checked for presence only —
    // it is worth knowing that React emits it as a boolean rather than as a
    // permanent `inert=""`, and worth nothing at all as evidence about the
    // keyboard, which is what the rest of this test is for.
    expect(explorer).toHaveAttribute("inert");

    const reached: Element[] = [];
    // Far enough round the document to have reached the right panel had the
    // background been tabbable at all: the chrome holds thirteen controls
    // before it.
    for (let i = 0; i < 25; i++) {
      await user.tab();
      const active = document.activeElement;
      if (active !== null) {
        reached.push(active);
      }
      expect(walkthrough().contains(document.activeElement)).toBe(true);
    }
    // Not a vacuous pass: tabbing genuinely moved focus around the card.
    expect(new Set(reached).size).toBeGreaterThan(1);
    for (const control of background) {
      expect(reached).not.toContain(control);
    }
  });

  it("closes on Escape through the explorer's one cascade", async () => {
    // Dispatched on the document rather than on the focused card, and that is
    // the point: a handler living inside the walkthrough would never see this
    // event, so a green assertion here is evidence the layer really is in
    // `MapExplorer`'s single cascade (ticket 22) rather than a third listener.
    const user = userEvent.setup();
    renderEverything();
    await startWalkthrough(user);

    fireEvent.keyDown(document, { key: "Escape" });

    expect(
      screen.queryByRole("dialog", { name: "Interface walkthrough" }),
    ).not.toBeInTheDocument();
  });

  it("takes Escape ahead of every other layer, being modal over them", async () => {
    const user = userEvent.setup();
    renderEverything();
    // A region open, so there is a step back for Escape to take if the
    // walkthrough does not take it first.
    await user.click(
      screen
        .getAllByRole("button")
        .filter((b) => b.classList.contains("region-chip"))[0] as HTMLElement,
    );
    expect(screen.getByTestId("back")).toBeInTheDocument();

    await startWalkthrough(user);
    await user.keyboard("{Escape}");

    expect(
      screen.queryByRole("dialog", { name: "Interface walkthrough" }),
    ).not.toBeInTheDocument();
    // One layer per press: the reader is still inside the region they opened.
    expect(screen.getByTestId("back")).toBeInTheDocument();
  });

  /** Starts the walkthrough the way a keyboard reader does — and that is the
   * whole point of it. A pointer press on the launcher already closes the
   * search overlay and the export menu, through the outside-click handler
   * each of them has, so a click would pass whatever the walkthrough did.
   * Enter on a focused button produces no `pointerdown`, so only the
   * explorer's own tidying-up can close them. */
  async function startFromTheKeyboard(
    user: ReturnType<typeof userEvent.setup>,
  ) {
    const launcher = screen.getByRole("button", { name: "Walkthrough" });
    launcher.focus();
    // The parking has to have taken, or this proves nothing about keyboards.
    expect(document.activeElement).toBe(launcher);
    await user.keyboard("{Enter}");
  }

  it("puts the search results away rather than dimming them", async () => {
    // The cascade only has one order, so two layers that both claim to be
    // innermost must not coexist underneath it.
    const user = userEvent.setup();
    renderEverything();
    await user.type(screen.getByLabelText("Search nodes"), "main");
    expect(screen.getByLabelText("Search results")).toBeInTheDocument();

    await startFromTheKeyboard(user);

    expect(walkthrough()).toBeInTheDocument();
    expect(screen.queryByLabelText("Search results")).not.toBeInTheDocument();
    // The query itself survives, as it does for every other dismissal.
    expect(screen.getByLabelText("Search nodes")).toHaveValue("main");
  });

  it("closes the export menu rather than dimming it", async () => {
    const user = userEvent.setup();
    renderEverything();
    await user.click(screen.getByRole("button", { name: "Share / Export" }));
    expect(
      screen.getByLabelText("Share or export this map"),
    ).toBeInTheDocument();

    await startFromTheKeyboard(user);

    expect(walkthrough()).toBeInTheDocument();
    expect(
      screen.queryByLabelText("Share or export this map"),
    ).not.toBeInTheDocument();
  });
});

describe("the walkthrough and the codebase tour", () => {
  it("names the two walks distinctly", async () => {
    // With the codebase tour actually on screen, which is the only state in
    // which the two could be confused for each other.
    const user = userEvent.setup();
    renderEverything();
    await openLearn(user);
    expect(screen.getByLabelText("Codebase tour")).toBeVisible();

    const starters = screen
      .getAllByRole("button")
      .map((b) => (b.textContent ?? "").trim())
      .filter((text) => /tour|walkthrough/i.test(text));
    expect(new Set(starters)).toEqual(new Set(["Start tour", "Walkthrough"]));

    await startWalkthrough(user);
    expect(walkthrough()).toBeInTheDocument();
  });

  it("does not leave the codebase tour half-walked", async () => {
    const user = userEvent.setup();
    renderEverything();
    await openLearn(user);
    const tour = () => within(screen.getByLabelText("Codebase tour"));
    await user.click(tour().getByRole("button", { name: "Start tour" }));
    await user.click(tour().getByRole("button", { name: "Next" }));
    expect(tour().getByText("Step 2 of 3")).toBeInTheDocument();

    await startWalkthrough(user);
    await user.click(
      within(walkthrough()).getByRole("button", { name: "Close walkthrough" }),
    );

    // Back at its own starting line rather than parked mid-walk behind the
    // thing that interrupted it.
    expect(screen.queryByText("Step 2 of 3")).not.toBeInTheDocument();
    expect(
      tour().getByRole("button", { name: "Start tour" }),
    ).toBeInTheDocument();
  });

  it("cannot be started from inside the walkthrough", async () => {
    const user = userEvent.setup();
    renderEverything();
    await openLearn(user);
    const start = screen.getByRole("button", { name: "Start tour" });

    await startWalkthrough(user);

    // Reachable by neither key nor pointer while the walkthrough runs: the
    // page behind it is not merely dimmed. Twenty-five presses is past the
    // thirteen chrome controls and the panel's own tabs, so a background that
    // was merely dim rather than inert would have arrived here.
    for (let i = 0; i < 25; i++) {
      await user.tab();
      expect(document.activeElement).not.toBe(start);
    }
  });
});

describe("the step list against the interface it describes", () => {
  it("explains every control the explorer renders", async () => {
    // The drift guard in the direction that matters most: a control added to
    // the chrome tomorrow belongs to no marked band, and this fails until
    // somebody decides which band it is in — or gives it a step of its own.
    renderEverything();
    const explorer = document.querySelector(".explorer");
    if (explorer === null) {
      throw new Error("no explorer rendered");
    }
    const controls = [
      ...explorer.querySelectorAll<HTMLElement>(
        'button, input, select, textarea, a[href], [role="radiogroup"]',
      ),
    ];
    expect(controls.length).toBeGreaterThan(10);

    const unexplained = controls
      .filter((el) => el.closest(`[${WALKTHROUGH_MARKER}]`) === null)
      .map((el) => el.outerHTML.slice(0, 120));
    expect(unexplained).toEqual([]);
  });

  it("says something about every part it marks, and marks every part it names", () => {
    // The other direction: a marker with no step is a highlight with nothing
    // to say, and a step with no marker is prose about something that is not
    // on the page.
    renderEverything();

    expect([...markersOnScreen()].sort()).toEqual(
      WALKTHROUGH_STEPS.map((s) => s.id).sort(),
    );
  });

  it("walks only the controls this particular page has", async () => {
    // Which settles the question the ticket left open. The prose is written by
    // hand — nothing can derive "Domain groups by the call flows the map
    // found" from a DOM node — but *which* steps are walked is read off the
    // live interface, so a page without a question box or a diff overlay is
    // never told about one.
    const user = userEvent.setup();
    const plain = render(<MapExplorer map={map} />);
    await startWalkthrough(user);

    const trimmed = WALKTHROUGH_STEPS.length - 2;
    expect(
      within(walkthrough()).getByText(`Step 1 of ${trimmed}`),
    ).toBeInTheDocument();
    const walked: string[] = [];
    for (let i = 1; i <= trimmed; i++) {
      walked.push(litId() ?? "");
      if (i < trimmed) {
        await user.click(
          within(walkthrough()).getByRole("button", { name: "Next" }),
        );
      }
    }
    expect(walked).not.toContain("ask");
    expect(walked).not.toContain("diff");

    // The same page with both controls present walks both extra steps.
    plain.unmount();
    localStorage.clear();
    renderEverything();
    await startWalkthrough(user);
    expect(
      within(walkthrough()).getByText(
        `Step 1 of ${WALKTHROUGH_STEPS.length}`,
      ),
    ).toBeInTheDocument();
  });
});

describe("the spotlight against a page that moves", () => {
  /** A `ResizeObserver` the test can fire, replacing the no-op in setup.ts.
   * Only the callbacks watching the element under test are fired, so React
   * Flow's own observers are left alone. */
  class FakeResizeObserver {
    static live: FakeResizeObserver[] = [];
    readonly targets = new Set<Element>();
    constructor(readonly callback: ResizeObserverCallback) {
      FakeResizeObserver.live.push(this);
    }
    observe(target: Element) {
      this.targets.add(target);
    }
    unobserve(target: Element) {
      this.targets.delete(target);
    }
    disconnect() {
      this.targets.clear();
    }
  }

  /** Fires the observers watching `target`, the way a browser would after a
   * layout pass. Wrapped in `act` because this is an environment event no
   * user gesture produces, so nothing else will flush the render it causes. */
  function reflow(target: Element): number {
    let fired = 0;
    act(() => {
      for (const observer of FakeResizeObserver.live) {
        if (observer.targets.has(target)) {
          fired += 1;
          observer.callback([], observer as unknown as ResizeObserver);
        }
      }
    });
    return fired;
  }

  function rect(top: number, left: number, width: number, height: number) {
    return {
      top,
      left,
      width,
      height,
      right: left + width,
      bottom: top + height,
      x: left,
      y: top,
      toJSON: () => ({}),
    } as DOMRect;
  }

  function geometryOf(): string {
    const spotlight = walkthrough().querySelector<HTMLElement>(
      ".walkthrough-spotlight",
    );
    if (spotlight === null) {
      throw new Error("no spotlight rendered");
    }
    const s = spotlight.style;
    return `${s.top} ${s.left} ${s.width} ${s.height}`;
  }

  beforeEach(() => {
    FakeResizeObserver.live = [];
    vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  });

  it("re-cuts itself where the element ended up, not where it started", async () => {
    // jsdom lays nothing out: every `getBoundingClientRect` is zeroes, so a
    // test that reflowed the page and compared two zeroes would assert
    // nothing at all. The rect is therefore driven from here — it genuinely
    // differs between the two reads, so a walkthrough that measured once and
    // kept the answer fails this.
    const user = userEvent.setup();
    renderEverything();
    const identity = document.querySelector<HTMLElement>(
      `[${WALKTHROUGH_MARKER}="identity"]`,
    );
    if (identity === null) {
      throw new Error("no element to measure");
    }
    let current = rect(10, 20, 100, 30);
    vi.spyOn(identity, "getBoundingClientRect").mockImplementation(
      () => current,
    );

    await startWalkthrough(user);
    expect(litElement()).toBe(identity);
    expect(geometryOf()).toBe("10px 20px 100px 30px");

    // The page reflows underneath it: same element, somewhere else.
    current = rect(240, 500, 320, 44);
    expect(reflow(identity), "nothing was observing the lit element").toBe(1);

    expect(geometryOf()).toBe("240px 500px 320px 44px");
  });

  it("stops watching an element it has moved off", async () => {
    const user = userEvent.setup();
    renderEverything();
    const identity = document.querySelector<HTMLElement>(
      `[${WALKTHROUGH_MARKER}="identity"]`,
    );
    if (identity === null) {
      throw new Error("no element to measure");
    }

    await startWalkthrough(user);
    expect(reflow(identity)).toBe(1);
    await user.click(within(walkthrough()).getByRole("button", { name: "Next" }));

    expect(reflow(identity)).toBe(0);
  });
});

describe("reduced motion", () => {
  /** The media query, which jsdom answers `false` to for everything. */
  function stubReducedMotion(reduce: boolean) {
    vi.stubGlobal(
      "matchMedia",
      (query: string) =>
        ({
          matches: query.includes("reduced-motion") && reduce,
          media: query,
        }) as MediaQueryList,
    );
  }

  /** jsdom has no `scrollIntoView`; the walkthrough guards for that, so the
   * spy is both the stand-in and the assertion. */
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

  it("eases a step into view when motion is welcome", async () => {
    stubReducedMotion(false);
    const scrollIntoView = spyOnScrolling();
    const user = userEvent.setup();
    renderEverything();

    await startWalkthrough(user);

    expect(scrollIntoView).toHaveBeenCalledWith(
      expect.objectContaining({ behavior: "smooth" }),
    );
  });

  it("cuts straight to it when the reader asked for less motion", async () => {
    stubReducedMotion(true);
    const scrollIntoView = spyOnScrolling();
    const user = userEvent.setup();
    renderEverything();

    await startWalkthrough(user);

    expect(scrollIntoView).toHaveBeenCalledWith(
      expect.objectContaining({ behavior: "auto" }),
    );
    expect(scrollIntoView).not.toHaveBeenCalledWith(
      expect.objectContaining({ behavior: "smooth" }),
    );
  });
});
