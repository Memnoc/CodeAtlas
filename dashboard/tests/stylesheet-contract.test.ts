// The stylesheet contract: the layout truths stories 20 and 22 rest on,
// asserted against the real `styles.css`.
//
// jsdom lays nothing out and never loads a stylesheet, so no component test
// in this repository can see paint. That gap hid three real walkthrough bugs
// in two days — off the right edge, off the top, and painted *under* the lit
// canvas — each through a fully green suite. The chain that remains testable
// without a browser is: gesture → state (component tests, jsdom) and
// state → geometry/stacking (this file, on the stylesheet itself). What is
// left to trust after both links hold is the CSS engine, which is the same
// thing every browser user already trusts.
//
// These are drift guards in the exact sense of `tests/routes.rs`: each
// assertion names the shipped bug that comes back if it reds.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

// Comments stripped before any matching: the stylesheet documents its own
// past bugs in prose, and a parser that reads "z-index: 101" out of a comment
// warning against z-index fails four tests over words that bind nothing.
const css = readFileSync(
  resolve(process.cwd(), "src/app/styles.css"),
  "utf8",
).replace(/\/\*[\s\S]*?\*\//g, "");

/** Every `selector { body }` block, flattened; enough for this stylesheet,
 * which nests only media queries (their inner blocks still match). */
function blocks(): Array<{ selector: string; body: string }> {
  const found: Array<{ selector: string; body: string }> = [];
  const re = /([^{}]+)\{([^{}]*)\}/g;
  for (const m of css.matchAll(re)) {
    found.push({ selector: (m[1] ?? "").trim(), body: m[2] ?? "" });
  }
  return found;
}

function zIndexOf(selector: string): number | null {
  const b = blocks().find((x) => x.selector === selector);
  const m = b?.body.match(/z-index:\s*(-?\d+)/);
  return m?.[1] !== undefined ? Number(m[1]) : null;
}

describe("story 20 — the walkthrough paints above the page, all of it", () => {
  it("gives the lit element no z-index, ever", () => {
    // The third shipped bug, verbatim. `[data-walkthrough-lit]` at 101 rose
    // above the overlay (100); the card's 102 is local to the overlay's
    // stacking context, so the lit canvas painted over the card and cut it
    // off at the canvas's top edge — step 7 of 12, found by the user's eyes
    // after 190 green tests. The dim is a box-shadow ring with a transparent
    // hole, so the target never needed raising: the absence of this rule IS
    // the fix, and this assertion is what keeps the absence.
    for (const b of blocks()) {
      if (b.selector.includes("data-walkthrough-lit")) {
        expect(
          b.body,
          `a z-index on ${b.selector} recreates the step-7 stacking bug`,
        ).not.toMatch(/z-index/);
        expect(
          b.body,
          `position on ${b.selector} creates a stacking context the same way`,
        ).not.toMatch(/position:/);
      }
    }
  });

  it("stacks the overlay above every page-level layer", () => {
    // The other half of the same invariant: the overlay must clear whatever
    // z-indexes the page uses (sticky tabs, breadcrumb, callout …), because
    // the card and the dim both live inside it. A page layer creeping above
    // the overlay's 100 would poke through the dim — same bug, new address.
    const overlay = zIndexOf(".walkthrough");
    expect(overlay).not.toBeNull();
    const family = [".walkthrough", ".walkthrough-card", ".walkthrough-spotlight"];
    for (const b of blocks()) {
      const m = b.body.match(/z-index:\s*(-?\d+)/);
      if (m?.[1] === undefined || family.includes(b.selector)) {
        continue;
      }
      expect(
        Number(m[1]),
        `${b.selector} stacks at ${m[1]}, at or above the walkthrough overlay (${overlay})`,
      ).toBeLessThan(overlay ?? 0);
    }
  });

  it("bounds the card to the viewport and lets it scroll", () => {
    // The second shipped bug's stylesheet half: the clamps in Walkthrough.tsx
    // assume the card is never larger than the window less a gap per edge —
    // that assumption lives here and nowhere else.
    const card = blocks().find((b) => b.selector === ".walkthrough-card");
    expect(card).toBeDefined();
    expect(card?.body).toMatch(/max-height:\s*calc\(100vh/);
    expect(card?.body).toMatch(/overflow-y:\s*auto/);
    expect(card?.body).toMatch(/width:\s*min\(/);
  });
});

describe("story 5 — a fan rather than a knot", () => {
  it("leaves the points edges land on unpainted", () => {
    // The bug this stops before it ships: a card used to expose two handles
    // and now exposes one per edge touching it, so React Flow's painted 6px
    // dot — fine twice — becomes a dotted rule along the edge of every
    // well-connected card, competing with the very lines the spread exists to
    // pull apart. The line's own end is the mark; the handle is only where
    // React Flow measures, and the canvas is not connectable, so nothing is
    // ever dragged from one.
    const handle = blocks().find((b) => b.selector === ".react-flow__handle");
    expect(handle, ".react-flow__handle must be styled here").toBeDefined();
    expect(handle?.body).toMatch(/background:\s*transparent/);
    expect(handle?.body).toMatch(/border:\s*0/);
  });
});

describe("story 22 — folding really gives the canvas the space", () => {
  // The component tests prove the gesture toggles the class and the panel
  // unmounts; these prove the class *means* something. Together the chain is
  // closed: fold → .workspace-folded (jsdom) → narrower first track (here) →
  // the second track is 1fr, so every surrendered pixel goes to the canvas.
  function firstTrackPx(selector: string): number {
    const b = blocks().find((x) => x.selector === selector);
    const m = b?.body.match(/grid-template-columns:\s*(\d+)px\s+(.+?);/);
    expect(m, `${selector} must state "<N>px <rest>" columns`).toBeTruthy();
    expect(m?.[2]).toContain("1fr");
    return Number(m?.[1]);
  }

  it("hands the panel's width to the canvas track when folded", () => {
    const open = firstTrackPx(".workspace");
    const folded = firstTrackPx(".workspace-folded");
    expect(folded).toBeLessThan(open);
    // Not merely smaller — a rail. Half the panel back would satisfy a bare
    // less-than and still fail the story.
    expect(folded).toBeLessThan(open / 4);
  });
});

describe("story 26 — the conversation is a bounded column beside the canvas", () => {
  // The jsdom half proves the gestures put the thread in the workspace next
  // to the canvas; this half proves the workspace and the column mean what
  // the move claims: a bounded, internally-scrolling right-hand track that
  // costs the canvas nothing while it is absent.
  const answer = () => blocks().find((b) => b.selector === ".answer");

  it("bounds the column inside the reference band, as one named constant", () => {
    // The V1 reference material's side panels ran ~360–400px; the ticket
    // pins the column inside that band, and pins it as a *named* constant so
    // the width and its reason live in one place. A literal in the width
    // rule would satisfy a looser match and drift silently.
    const b = answer();
    expect(b, ".answer must be styled here").toBeDefined();
    const named = b?.body.match(/--conversation-column:\s*(\d+)px/);
    expect(
      named,
      ".answer must state its bound as --conversation-column: <N>px",
    ).toBeTruthy();
    const px = Number(named?.[1]);
    expect(px).toBeGreaterThanOrEqual(360);
    expect(px).toBeLessThanOrEqual(400);
    // Bounded, not fixed: the constant is the ceiling, and narrower
    // viewports get less — never more.
    expect(b?.body).toMatch(/width:\s*min\(var\(--conversation-column\)/);
  });

  it("scrolls the thread inside the column", () => {
    // The band this replaces grew with every exchange and pushed the canvas
    // off the screen; the column's whole point is that six turns scroll
    // internally while the canvas keeps its size mid-read.
    expect(answer()?.body).toMatch(/overflow-y:\s*auto/);
  });

  it("gives the column its own workspace track and the canvas the rest", () => {
    // `auto` and not a fixed track: an absent column (no question asked)
    // must cost the canvas nothing, and the 1fr canvas track — already
    // pinned by story 22's guard above — keeps the remainder while it is
    // present.
    for (const selector of [".workspace", ".workspace-folded"]) {
      const b = blocks().find((x) => x.selector === selector);
      expect(
        b?.body,
        `${selector} must end its columns with an auto track for the conversation`,
      ).toMatch(/grid-template-columns:[^;]*1fr\)\s+auto\s*;/);
    }
  });

  it("invents no stacking context for the column", () => {
    // V1's three walkthrough placement bugs were all stacking-context
    // inventions, and the export menu (30) must keep painting over the
    // column through the existing order — from normal flow, without a rung
    // of its own.
    expect(answer()?.body).not.toMatch(/z-index/);
    expect(answer()?.body).not.toMatch(/position:/);
  });
});
