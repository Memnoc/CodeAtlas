# Dashboard UI reference — the shape to aim at

Two screenshots supplied 2026-08-10 as the model for the CodeAtlas dashboard
rework. Captured here so the reference is dated and digested rather than left
in a chat scrollback; this is **input** to `/adr` → `/to-spec` → `/to-tickets`,
alongside the existing seven-ticket sketch, not a substitute for running them.

Source images:

- `~/Pictures/Screenshots/Screenshot_2026-08-10_12-29-37.png` — whole window
- `~/Pictures/Screenshots/Screenshot_2026-08-10_12-29-50.png` — regions panel

The subject is a 49-file project (`remote-org-chart`) in 10 regions with 252
relationships — roughly a third of CodeAtlas's own 159 files, which matters:
part of why it reads well is that it is showing less. That was already the
sketch's "key insight", and this reference does not contradict it.

Notably the reference is itself in **Rosé Pine Moon**, which is where the theme
request came from; the palettes now shipped in `a37ce60` already match it.

## What is on screen

**Top header** — project name; a `STRUCTURAL · 145 structural` count badge; two
segmented toggles, `Overview | Learn` and `Domain | Structural`; then `Export`,
`Path`, a theme control reading `Rosé Pine Moon`, and a help affordance.

**Search row** — full-width field ("Search nodes by name, summary, or tags…")
with a `Fuzzy | Semantic` segmented control at the right.

**Status strip** — a collapsed amber notice, "Knowledge graph freshness could
not be verified".

**Layer chips** — `10 layers` followed by one coloured dot per layer with its
count.

**Canvas** — layer cards rather than file cards: each has a coloured left
border, a `LAYER` eyebrow, a complexity word (`simple` / `moderate`) at the top
right, then name, description, and `N files`. Edges are curved and carry counts
(`4 links`, `16 imports`, `14 imports`). A `PROJECT OVERVIEW` pill sits top
left, keyboard-shortcut hint top right, zoom/fit/lock controls bottom left,
minimap bottom right.

**Right panel**, tabbed `INFO | FILES`:

- Project name, prose description, then "49 files in 10 regions, 252
  relationships between them."
- Language chips with counts (`TypeScript 41`, `CSS 4`, …).
- **START HERE** — "Calls other code; nothing calls it." Rows of symbol name
  plus source path.
- **EVERYTHING LEANS ON** — "The files the most other files reach into." Rows
  of path plus `← N files`.
- **REGIONS** — a card per region: colour dot, name, `N files`, a proportional
  bar, and a description.
- **HOW THEY CONNECT** — begins below the fold; content unseen.

## What CodeAtlas already has, and what it does not

Sorted by what the work would actually cost, which is the useful axis:

**Already in the map, merely not rendered** — layer names and membership,
per-node summaries, `domain_flows`, the tour, provenance, edge kinds and
counts. The `Domain | Structural` toggle maps onto `domain_flows` versus
`layers`; `Overview | Learn` maps onto the region view versus the guided tour.
Region descriptions exist as enriched layer names today, though not as prose.

**Pure rendering over data already present** — layer cards with file counts,
layer chips, edge count labels, the language chips (derivable by extension),
minimap and zoom controls (React Flow ships both), the "N files in M regions,
K relationships" line.

**Needs a ranking rule that does not exist yet** — START HERE and EVERYTHING
LEANS ON. Both are fan-in/fan-out rankings, and the sketch's second insight
applies directly: the tour, progressive disclosure, the complexity band and now
these two panels are all asking "which nodes matter". Decide it once, in Rust,
and let every consumer read it — otherwise the tour highlights a file the
overview calls unimportant.

**Needs new contract fields** — per-region prose descriptions, the
`simple`/`moderate` complexity word, and region file counts as a published
number rather than something each consumer recomputes. Contract changes mean
schema plus TS regeneration, so these carry the ceremony ADR-0003 sets out.

**Out of scope as drawn** — `Semantic` search implies embeddings, which is a
different product decision entirely, and `Export`/`Path` are unspecified.

## One discrepancy worth resolving before ticketing

The two screenshots disagree about what a "file count" is. The header chips say
`website (16)`, `(root) (14)`, `src (8)`; the REGIONS cards say `website 4
files`, `(root) 4 files`, `src 8 files`. The progress bars track neither
consistently — `website` at 4 files draws a longer bar than `src` at 8.

So the chips and the cards are counting different things (plausibly all nodes
versus file nodes), and the bar is scaled by a third thing. Whatever CodeAtlas
builds should count one thing and say which; a panel where two numbers for the
same region disagree is worse than a panel with one number.
