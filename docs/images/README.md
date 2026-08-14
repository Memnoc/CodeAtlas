# README images

The banners the top-level [README](../../README.md) embeds. Every image
ships as a dark/light pair and the README's `<picture>` blocks follow the
reader's GitHub theme, so both halves of a pair must exist for its slot to
work everywhere.

The filenames are the contract: a re-exported image with the same name is a
drop-in replacement and no document changes. A renamed one is a broken slot.

| Pair | What it shows | Where it lives |
| --- | --- | --- |
| `plate-{dark,light}.png` | Nameplate: the pitch and the day's map counts. | Masthead, before the intro. |
| `viz-{dark,light}.png` | Why a map: the code on the left becomes the picture on the right. Concept card, illustrative code. | After the intro. |
| `pipeline-{dark,light}.png` | How it works: scan → map.json → optional `--enrich` → read or share. | How it works. |
| `legend-{dark,light}.png` | How to read the map: region, edge, elevation, provenance. | Opens the tour. |
| `tree-{dark,light}.png` | The shape of a repository: root, branch regions, files at the tips. Concept card, illustrative repo. | Tour. |
| `constellation-{dark,light}.png` | Cards that know each other: uses, used-by, and the route between two corners. Concept card, illustrative repo. | Tour. |
| `twoviews-{dark,light}.png` | Structural vs Domain grouping, one toggle apart. | Tour. |
| `provenance-{dark,light}.png` | The two label kinds: `structural` vs `llm`, and what `share` strips. | Closes the tour. |

The concept cards (`viz`, `tree`, `constellation`) draw an illustrative
repository, not this one — the README's showcase copy says so, and must
keep saying so as long as they are published.

## Held pairs — exported, not published

Two pairs exist in the design file but stay out of `docs/images/` until
their copy stops contradicting the product:

- **`hero-*`** says "in nine themes" — the dashboard ships two (Rosé Pine
  Dawn and Moon; `dashboard/src/app/theme.ts` states this in its first
  line). Drop the clause, or say two.
- **`walkthrough-*`** says "Thirteen steps" with a "13 steps" chip, and its
  screenshot shows "STEP 1 OF 13" — the shipped walkthrough has fourteen
  steps and the count moves with the UI. Drop the number entirely.

After re-export, add the files here and give each a `<picture>` block at
the `HELD BANNERS` comment in the top-level README.

## Retired pairs — 2026-08-14

The seven capture-heavy pairs from the V1 showcase (`regions`, `flows`,
`focus`, `search`, `ask`, `panel`, `share`) came out with the final banner
set: their embedded captures predate V2's drill view, magnify and
conversation column, and the `panel` and `ask` pairs showed UI that no
longer ships (the retired importer ranking; the answer band the column
replaced). Git history keeps the files; a future capture pass against the
V2 dashboard can bring the ideas back under the same names.

## Re-exporting

The source design is kept with the design assets, outside this repository
(theme and accent are Tweaks). Re-export by snapshotting the `#shot-<name>`
frames at 2×, then overwrite the files here under the same names. The
screenshots embedded in the frames come from a real `codeatlas serve` — on
UI changes, rebuild (`cargo build --release`), restart, and re-capture
rather than editing pixels.

Two rules for the copy inside the images, learned the usual way:

- **No product numbers.** "Thirteen steps" and "nine themes" were both
  wrong within a day of export — step counts and feature tallies move with
  every ticket. Counts that come from the day's map (files, regions,
  enriched slots) are fine; the README says the shots belong to their day.
  Illustrative counts on the concept cards are fine for the same reason the
  README's copy declares them illustrative.
- **Read the screenshots before exporting.** The first export immortalised
  a live UI bug ("2 importss") that a minute of reading would have caught —
  it became a fix, but the pictures had already framed it.
