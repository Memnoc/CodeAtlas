# The browser walk — stories 3, 6, 7, 8, plus 20 and 22

Fifteen minutes at `http://127.0.0.1:4173/`, before `/harden`.

**Why this list exists.** No test in this repository can see layout. jsdom
lays nothing out and never loads `styles.css`, so every claim the dashboard
suite makes about appearance is arithmetic rather than evidence. On
2026-08-12 that gap hid two real defects — the walkthrough card off the right
edge and then off the top — through 181 green tests, and both were found in
the first two minutes of somebody looking. These four stories have **never**
been looked at.

**How to use it.** Each section says what to do and what *wrong* looks like.
The second part matters more: "it looked fine" from someone who did not know
what to distrust is close to worthless, and every entry under **suspect**
below is a specific thing this codebase has reason to worry about.

Mark each story **pass**, **fail**, or **unverifiable**. Anything that fails
becomes a ticket; `/harden` will want the verdicts.

---

## Story 3 — the interactive dashboard

*"a graph canvas, search, layer grouping, node detail, so that I navigate the
map instead of reading JSON"*

1. Open the page. The canvas should draw **8 region cards**, one per layer,
   named — *GitHub CI Configuration*, *Rust Core Crates*, *TypeScript
   Dashboard UI* and so on.
2. Click **Rust Core Crates**. It drills in and draws that layer's files.
3. Click a file card. The panel on the left fills with its detail — summary,
   path, and every edge touching it as a clickable control.
4. Type `serve` in the search field. Pick a result. It should select that node
   on the canvas, drilling in if needed.
5. Flip **Domain** / **Structural**. The canvas regroups; the panel keeps
   describing whatever is selected.
6. Use the breadcrumb and the **← Back** control to get out again.

**Suspect:**
- **Cards overlapping, or edges drawn through them.** The layout is
  hand-rolled — layered by dependency depth with barycentre sweeps — and was
  tuned to 263 edge crossings on a 44-file region. This repo's Rust layer is
  159 files. That is well past anything it was measured on.
- **Labels truncated or spilling.** Layer names are now model-written and
  longer than the directory names the cards were sized for.
- **A region card reading `.scratch`** — the one layer whose enriched name
  reverted, because its contents changed after enrichment. Correct behaviour,
  but check it looks deliberate rather than broken next to seven prose names.
- **The header tally.** It reads `STRUCTURAL · 170 structural, 1131 enriched`
  — `structural` twice, once meaning the grouping mode and once the
  provenance. Known wart, undecided; judge whether it actually confuses.

## Story 6 — domain flows and the guided tour

*"so that I learn the architecture in the order it makes sense"*

1. Switch to **Learn**. The codebase tour panel appears with **12 steps**.
2. **Start tour.** Each step should select a node *and* narrate it in a
   sentence written for a newcomer.
3. Step all the way through. Watch the canvas follow.
4. Look at the flows panel. There are **324 domain flows** in this map.

**Suspect:**
- **324 flows in a side panel.** That is a lot. Check it is browsable rather
  than an unreadable dump — this is the single most likely thing on this page
  to be quietly unusable.
- **Tour narration that says nothing.** Each label was model-written from the
  path and its import fan-in/fan-out. Read three or four. If they are generic
  ("This file contains code for the project"), the tour is decoration.
- **A tour step that selects nothing**, or whose narration describes a
  different file than the one it lit.

## Story 7 — the diff overlay

*"a diff's changed nodes and their one-hop blast radius, so that I can judge
the risk of a change"*

The overlay is currently **empty** — the worktree is clean, so there is
nothing to diff. Make something to see:

```
!echo "// scratch" >> crates/codeatlas/src/serve.rs
!./target/release/codeatlas diff .
```

Then reload the page (no restart needed — `serve` reads the overlay per
request). Undo the edit afterwards with `git checkout crates/codeatlas/src/serve.rs`.

1. The **Diff overlay** toggle appears in the chip row with its counts.
2. Turn it on. Changed nodes take one colour, affected nodes another.
3. Drill into a region and confirm the marks are on the right files.

**Suspect:**
- **Gold against gold.** This is the weakest rule in `styles.css` and the file
  says so itself: `--affected` and `--link` are *the same hex*, separated only
  by surface — affected fills a **card**, link strokes an **edge**. With an
  overlay loaded they appear together and only shape tells them apart. This is
  the one place in the whole design that was knowingly compromised. Look hard.
- Changed-vs-affected being hard to tell apart at a glance.
- Counts in the chip row disagreeing with what is marked. The label warns that
  counts are of nodes including symbols while the canvas draws files.

## Story 8 — the shared, redacted export

*"a single self-contained redacted HTML file, so that I can share the map with
someone who has nothing installed"*

1. Open **Share / Export**. Both routes should be explained, not just listed.
2. Run the command it gives you for the self-contained page.
3. **Open the file by double-clicking it** — a real `file://` origin, not
   through the server. That is the case the whole feature exists for.

**Suspect:**
- **Model-written prose surviving the redaction.** This is the only item on
  this list that is a *security* finding rather than a cosmetic one. Search
  the page for a sentence you recognise from the dashboard's summaries. The
  allowlist redaction is done in Rust and deliberately not reimplemented in
  TypeScript; if enriched prose is in that file, stop and say so.
- A blank page on `file://` — an opaque origin, where `localStorage` throws
  and every path that assumes a server is wrong.
- The share banner missing, so a reader cannot tell it is a redacted copy.

## Story 20 — the interface walkthrough (re-check)

Both of today's fixes are unverified by any test — jsdom cannot see either.

1. Press **Walkthrough**. Step all the way through, **14 steps**.
2. At **every** step, check the card is fully on screen: prose not cut off,
   **Back** and **Next** both fully visible.

**Suspect:** the steps whose control sits at the right end of the toolbar —
Diff overlay, Focus, Share/Export, theme, Walkthrough itself — and any step
whose prose runs long on a short window. Those were the two failures. Resize
the window narrow and short, then walk it again.

## Story 22 — the fold (new, never seen)

1. Fold the side panel with the **‹** in the tab strip. It should collapse to
   a narrow rail with a vertical **PANEL** label and a way back.
2. Unfold it. Fold the region chips with the **▾** beside the count.
3. Press **Focus**. Both fold. Press it again; both return.
4. Fold something, reload the page. It should still be folded.

**Suspect:**
- **The 30px rail.** Is the vertical label legible, or is it a smear? Is the
  target big enough to hit?
- **The canvas actually getting the space.** No test can show this. If the
  canvas does not visibly grow, the feature does nothing.
- Running the walkthrough *while folded* — the panel step should be skipped,
  not spotlight an empty rail.

---

## Recording it

For each: **pass**, **fail**, or **unverifiable**, one line of why. `/harden`
writes them into the spec's `## Verification` section, and anything that fails
becomes a ticket rather than a fix on the spot.
