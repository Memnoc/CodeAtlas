// Named `walkthrough-steps` and not `walkthrough`, because `Walkthrough.tsx`
// sits in the same directory: on a case-insensitive filesystem (macOS, where
// the release workflow builds) `./Walkthrough.js` case-folds onto a file
// called `walkthrough.ts` and the component's exports vanish — the V3
// release dry run caught exactly that. The two names must never collide
// case-insensitively again.
//
// The walkthrough of the *application* (spec story 20) — which is not the
// codebase tour of story 6, and is never called a tour anywhere a reader can
// see. The codebase tour walks the map's own files in the order the CLI chose;
// this walks the controls of the page the map is being read in.
//
// **Hand-written prose, a list read off the live page.** The ticket left the
// choice open — hand-write the steps, or derive them from the components
// present — and the honest answer is that neither half of a step comes from
// the same place. Nothing can derive "Domain groups files by the call flows
// the map found, Layer by the directories they live in" from a DOM node;
// that sentence has to be written. But *which* of these steps a given page is
// walked through must not be written down, because the page differs: a share
// artifact has no question box, and a repository with no `codeatlas diff` run
// has no overlay toggle. So the list below is a declaration, and every walk is
// that declaration filtered against the elements actually on screen — a step
// whose element is absent is not walked, and no reader is told about a control
// they do not have.
//
// The staleness a hand-written list is accused of is then a *test* problem
// rather than a derivation problem, and it is solved as one. Two guards in
// `dashboard/tests/walkthrough.test.tsx` hold the declaration and the
// interface together in both directions: every interactive control in the
// explorer must sit inside some marked element, so a control added to the
// header tomorrow fails the suite until it is placed; and the markers on a
// fully-featured page must be exactly the ids below plus the transient bands
// named after them, so a marker without prose and prose without a marker both
// fail too.

/** The attribute that marks an element as something the walkthrough explains.
 * Put it on the band, not on every control inside it: the reader is being
 * shown where a job lives, and a spotlight on one button of a segmented pair
 * would be describing half a decision. */
export const WALKTHROUGH_MARKER = "data-walkthrough";

/** Put on the live element the current step is about, for as long as it is the
 * current step. The stylesheet uses it to lift the element clear of the dim;
 * it is also the one honest way to assert "this real control is lit", since a
 * copy of the interface could never carry it. */
export const WALKTHROUGH_LIT = "data-walkthrough-lit";

/** Where the browser remembers that the walkthrough has been offered. Local
 * storage and nothing else: recording it anywhere a request could reach would
 * break ADR-0006 for the sake of a boolean. */
export const WALKTHROUGH_SEEN_KEY = "codeatlas-walkthrough-seen";

export type WalkthroughStep = {
  /** The value of {@link WALKTHROUGH_MARKER} on the element this is about. */
  id: string;
  title: string;
  body: string;
};

/** Bands that are marked so the controls inside them are accounted for, and
 * that deliberately carry no step.
 *
 * Both are reading columns that exist only after the reader has done
 * something, which is the reason a marker and a step are two different
 * things: the conversation column appears only after a question is asked,
 * the source column (ticket 02 of V3) only after a selected node's code is
 * opened, and a step about either would spotlight an absent element on
 * every walk that did not happen to follow that gesture. The bands they are
 * reached *from* — the search row, the side panel — are explained instead.
 *
 * Written down rather than left implicit because the alternative is an
 * invariant that reads stronger than the component: "every control sits in a
 * marked band" is true, and "every marked band has prose" is true of
 * everything but these. Both guards in `tests/walkthrough.test.tsx` are stated
 * against this list, and the second requires every id in it to actually be on
 * screen — so an entry that stops naming a real band fails too. */
export const WALKTHROUGH_TRANSIENT: readonly string[] = ["answer", "source"];

/** Every step there is, in reading order — top bar first, then the search
 * band, the regions, the canvas and its panel, and last the things that take
 * the map somewhere else. */
export const WALKTHROUGH_STEPS: readonly WalkthroughStep[] = [
  {
    id: "identity",
    title: "What you are looking at",
    body:
      "The repository this map was scanned from, and how it is labelled. " +
      "Structural labels were computed from the code itself; enriched ones " +
      "were written by a model during `codeatlas scan --enrich`, and every " +
      "one of them is badged as such wherever it appears.",
  },
  {
    id: "view",
    title: "Overview, or Learn",
    body:
      "Overview keeps the panel on the right factual — what is selected, and " +
      "what it touches. Learn replaces it with the two guided reads of the " +
      "map: the codebase tour, an ordered walk through the files that carry " +
      "the architecture, and the call flows the scan found.",
  },
  {
    id: "grouping",
    title: "How the files are grouped",
    body:
      "Layer groups files by the directories they live in. Domain groups " +
      "them by the call flows the map found, so files that take part in " +
      "the same chain sit together. It changes the canvas, never the panel.",
  },
  {
    id: "search",
    title: "Finding something by name",
    body:
      "Matches on name, path, and summary, narrowing as you type. Choosing a " +
      "result selects that node, opening the region holding it if it is not " +
      "open already — a highlight nobody can see is not a selection.",
  },
  {
    id: "ask",
    title: "Asking the map a question",
    body:
      "This server was started with `serve --ask`, so the same field takes a " +
      "question in your own words. The local binary puts it to a model, " +
      "answers from the map alone — never your file contents — and cites the " +
      "nodes it used, each of which opens on the canvas.",
  },
  {
    id: "regions",
    title: "The regions of the repository",
    body:
      "One chip per region, with the number of files in it. Pressing one " +
      "draws that region's files on the canvas; pressing it again returns to " +
      "the overview.",
  },
  {
    id: "diff",
    title: "The blast radius of a change",
    body:
      "A `codeatlas diff` overlay exists for this repository. Switching it on " +
      "marks the files the diff changed and the files one hop away from them, " +
      "which is the question a reviewer is actually asking.",
  },
  {
    id: "canvas",
    title: "The canvas",
    body:
      "Regions first, files one click in — a repository has hundreds of files " +
      "and a handful of regions. Selecting a file brings what it imports " +
      "forward and steps the rest back; the trail above says where you are, " +
      "and the button beside it says how to leave.",
  },
  {
    id: "panel",
    title: "The detail panel",
    body:
      "Info describes whatever is selected, including every edge touching it " +
      "as a control that follows it. Files lists the whole repository by " +
      "region, for when you know the file you want.",
  },
  {
    id: "path",
    title: "The way between two nodes",
    body:
      "Pick two nodes and this finds the shortest chain of relationships " +
      "joining them, then draws that chain on the canvas — the answer to " +
      "“does this reach that, and through what”.",
  },
  {
    id: "focus",
    title: "Giving the space to the map",
    body:
      "The panel and the regions each fold away on their own, and this folds " +
      "both at once. A folded panel leaves a rail with the way back on it, " +
      "and what you fold is remembered in this browser.",
  },
  {
    id: "export",
    title: "Taking the map with you",
    body:
      "Two ways out, and they are not variants of one thing: the map as JSON " +
      "against the published contract, for anything that reads the format; " +
      "and the command that writes one self-contained page for someone with " +
      "nothing installed, with model-written prose redacted out of it.",
  },
  {
    id: "theme",
    title: "Light and dark",
    body:
      "Rosé Pine Dawn and Moon. The page opens in whichever your system " +
      "asks for, and a deliberate choice is remembered in this browser.",
  },
  {
    id: "walkthrough",
    title: "And back to here",
    body:
      "This walkthrough is always one press away, and it never leaves the " +
      "page: nothing about it is recorded anywhere but this browser.",
  },
];

/** The live element a step is about, or `null` when this page has no such
 * control — a share artifact has no question box, and a repository with no
 * diff run has no overlay toggle. */
export function walkthroughTarget(id: string): HTMLElement | null {
  return document.querySelector<HTMLElement>(
    `[${WALKTHROUGH_MARKER}="${id}"]`,
  );
}

/** The walk this particular page gets: the declaration above, minus the steps
 * whose subject is not on screen. */
export function resolveWalkthroughSteps(): WalkthroughStep[] {
  return WALKTHROUGH_STEPS.filter((step) => walkthroughTarget(step.id) !== null);
}

/** Whether this browser has been offered the walkthrough before. Storage
 * refusing — an opaque `file://` origin throws rather than returning null —
 * reads as "no", which costs a first-visit callout and nothing else. */
export function hasSeenWalkthrough(): boolean {
  try {
    // `?? null` and not a bare `!== null`: an absent `localStorage` yields
    // `undefined`, which would otherwise read as "yes, seen" and silently
    // suppress the callout everywhere storage is missing.
    return (
      (globalThis.localStorage?.getItem(WALKTHROUGH_SEEN_KEY) ?? null) !== null
    );
  } catch {
    return false;
  }
}

/** Records the offer as made, whether it was taken up or declined. */
export function markWalkthroughSeen(): void {
  try {
    globalThis.localStorage?.setItem(WALKTHROUGH_SEEN_KEY, "1");
  } catch {
    // A read-only or opaque origin still gets the walkthrough, just not the
    // memory of having been offered it.
  }
}
