# Ticket 28 — Export offers a format without saying what it is for

**Status:** done
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 8 — export a single self-contained redacted HTML file to share with
someone who has nothing installed
**Blocks:** none
**Blocked by:** none

## Problem

There are two ways a map leaves the dashboard, and the UI offers only the one
most readers do not want.

**Export** downloads the raw knowledge graph as JSON. It is a *machine*
format: its value is that it conforms to the published versioned contract, so
another tool, a script, or a diff can consume it. Handed to a person, it is a
few hundred kilobytes of JSON.

**`codeatlas share`** writes the thing a person wants — one self-contained
HTML file, the same renderer inlined, opens by double-click with no server and
nothing installed, LLM prose redacted against an allowlist over the contract,
and the artifact states what it removed. That is story 8, and the UI never
mentions it outside a `title` tooltip on the Export button.

Asked directly on 2026-08-11, having used the built dashboard: *"I see it
exports a json file, what do I do with it?"* A tooltip is hover-only, absent
on touch, and unreadable by anyone who did not already suspect there was
something to hover over. The question is the evidence that it does not work as
an explanation.

There is also a real hazard the UI does not state: **the JSON is not
redacted.** On an enriched map it carries every generated summary. The share
artifact redacts precisely that, so a reader who reaches for Export because it
is the visible button gets the unredacted one by default.

## Acceptance criteria

- [x] The share route is visible in the UI as something other than a tooltip,
      and says plainly that it produces a self-contained page needing nothing
      installed. The top-bar button now reads **Share / Export**, so the word
      is in the chrome without any interaction, and the panel behind it names
      the route in a heading.
- [x] Export says what it is for — the map as data, against the published
      contract — rather than only its file format. It names the contract and
      the map's own `version`, so the sentence stays true across contract
      revisions.
- [x] The UI states that the exported JSON is unredacted whenever the map
      being viewed carries `llm` provenance, and says nothing about redaction
      when there is no enriched prose to redact. **Keyed slightly differently
      than written — see Notes.**
- [x] Viewing a share artifact, which cannot run a CLI, does not advertise a
      command the reader has no way to run. Driven through `<App/>` with a
      real embedded payload, so the `shared` flag is proven end to end rather
      than passed by hand.
- [x] The dashboard still runs `codeatlas share` for nobody: it prints or
      copies the command, it does not attempt to execute anything (ADR-0006 —
      the dashboard makes zero external requests and has no shell).
- [x] No change to what either route emits. This ticket is about what the UI
      says, not what it produces. Pinned byte-for-byte: the download's blob
      equals `JSON.stringify(map, null, 2)` and its filename equals
      `mapFilename(map)`.

## Notes

**The redaction criterion is keyed on `!shared && enriched > 0`, not on `llm`
provenance alone, and the criterion as written was wrong.** The share
allowlist classifies `Node.provenance` as share-safe: it keeps the provenance
and replaces the *prose*. So a share artifact carries `llm`-provenance nodes
with nothing left to leak, and a warning keyed on provenance would have fired
there and been false — telling the reader their already-redacted snapshot was
unredacted. The count shown is every enriched slot across all four redactable
fields (`Node.summary`, `Layer.name`, `DomainFlow.name`, `TourStep.label`),
not just node summaries — see the correction at the foot of this section for
the edge where that number is still short. There is a test for the
false-positive case specifically.

The count is also a different unit from the header's provenance tally, which
counts *nodes*: on `small-map.json` the header says "1 enriched" while this
warning says "2 LLM-written prose fields", and both are right. The warning
names its unit — "node summaries, layer and flow names, tour narration" — so
the two read as different measurements rather than as a contradiction.

**Escape goes through the explorer's cascade, not through the menu.** Ticket
22 rebuilt that cascade after `/crosscheck` found a dead zone, and a second
Escape handler in a second component is exactly how that hole reopens. The
menu's open state therefore lives in `MapExplorer` beside `pathOpen`, and the
cascade is now: search overlay → share/export menu → path panel → one step
back. A test asserts the ordering by opening a region first: if the menu
layer were missing, Escape would drop the drill-in instead.

**Seventeen mutations, all killed** — the two `!shared` guards, the
`enriched > 0` guard, the four-collection count, the button label in both
directions, the cascade layer, the outside-click close, both focus-restore
guards, the copy-state reset, the copied string, the download payload, the
contract version, and the no-network assertion. Two were worth the trouble
specifically: "the copy path performs no fetch" would be easy to leave as a
sentence rather than a test (adding a `fetch` call to the copy path fails
it), and the four-collection count is asserted against a written-out `9`
rather than a sum of the same lengths the implementation adds, so dropping a
collection changes one side of the comparison and not both.

### What `/crosscheck` found, and what changed because of it

Six findings across the two axes, all real, all fixed. The two that mattered:

**A dead CSS rule, so the one line meant to stand out did not.**
`.export-route p` is specificity 0-1-1 and mutes every paragraph in the
panel; `.export-warning` at 0-1-0 lost to it, and the unredacted warning
rendered in the same grey as the body copy above it. Now selected as
`.export-route p.export-warning`, and given the caution surface
`.share-banner` already uses — iris, which is *enriched prose* everywhere in
this stylesheet, which is exactly what the sentence warns about.

**Four criteria were ticked on tests that could not fail.** The button label
was reached by `/export/i` in every test, which matches both arms of
`shared ? "Export" : "Share / Export"` — so criterion 1's stated mechanism,
the word being in the chrome, was the one claim nothing pinned. Likewise
`small-map.json` enriches one node and one layer, so a count that ignored
flows and tour steps looked right; and both focus-restore guards survived
mutation. Fixed by asserting the exact label in both modes, by counting
against an all-enriched map where each of the four collections contributes,
and by two focus tests — one for mount (the menu must not seize focus as the
page loads) and one for a reader who tabbed out of the panel before closing
it (closing must not drag them back).

Also fixed: focus was dropped to `<body>` when the panel closed with focus
inside it; the command did not say where the artifact lands or that the path
argument defaults to the working directory; a `vi.stubGlobal("fetch")` was
never unstubbed.

**What has not been looked at is how it looks.** Same limitation as ticket
22's icon size: no browser can be driven in this environment, so the 320px
panel, its contrast, and its behaviour on a narrow window are reasoned about,
not seen. Two specifics were settled by reading rather than by eye, and both
should be glanced at before shipping. Stacking: no ancestor between the panel
and the root creates a stacking context, so `z-index: 30` genuinely clears
the search results at 20 and the breadcrumb at 10. Specificity: the warning
fix above is arithmetic, not a test — `styles.css` is never loaded in the
jsdom suite, so no test in this repo can see a cascade.

**One correction to a claim made earlier in this ticket.** The count is every
`llm`-provenance slot across the four *prose* collections, which is what the
allowlist replaces with the marker. It is not the whole of what
`codeatlas share` strips: the allowlist is deny-by-default, so a map carrying
fields the contract does not name loses those too, uncounted here. This CLI
never emits such a map, but a third-party producer (story 16) could.

**One deliberate cost.** Export used to be one click and is now two. That is
the trade the ticket asks for: the single click was the fast path to the
artifact most readers did not want, on an enriched map without saying it was
the unredacted one.

Deliberately not in scope: making the dashboard *generate* the share
artifact. That would move ADR-0006's allowlist redaction into TypeScript, and
the whole reason `export.ts` does not redact today is that two copies of one
security policy in two languages means the copy that drifts is the one that
leaks. The CLI stays the only thing that writes a share artifact.
