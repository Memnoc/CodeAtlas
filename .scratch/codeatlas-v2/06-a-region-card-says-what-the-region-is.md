# Ticket 06 — a region card says what the region is

**Status:** done
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 7 — a region card describes the region in prose (the mechanical
half here, the purchased half in ticket 07); 9 — the new field is optional, so
every map that validated yesterday validates today
**Blocks:** 07
**Blocked by:** none — can start immediately

## Problem

The card a newcomer clicks to enter the densest region of this repository
says "Files under crates/". They already knew that: the region's name is the
directory name, and the description repeats it. The overview tells the reader
where things live and never what they are.

## What to build

Layers carry an optional description in the map contract, and the region card
renders it.

## Acceptance criteria

- [x] Layers carry an optional `description` in the contract; structural
      layers only.
- [x] Domains gain nothing — they are derived from flows and are not contract
      entities, and their synthesised text is already honest (spec, Out of
      Scope).
- [x] Old maps validate unchanged, asserted against a stored fixture map
      lacking the field.
- [x] Schema and generated TypeScript regenerate together; the drift gate
      passes (ADR-0003 ceremony).
- [x] The region card renders the description, and the overview stays
      readable at this repository's eight layers — a description never grows
      a card without bound.
- [x] Provenance is badged, and mechanical text is never badged `llm`.
- [x] A map whose layers carry no description renders exactly as it does
      today.

## Notes

**One design decision to make explicitly, with a recommendation.** `Layer`
today has a single `provenance` covering its name, and the mechanical name is
published by the scan for enrichment to relabel. Follow that shape: the scan
publishes the mechanical sentence as the description, and enrichment replaces
it in ticket 07. That means the description needs provenance of its own —
otherwise a layer with a mechanical name and purchased prose can only be
badged with one answer, and either badge would be a lie about half the card.

The alternative — leaving the field absent until purchased and letting the
dashboard keep synthesising — is defensible and cheaper, but it splits the
fallback across two languages, which is the shape that drifts. Pick one,
record why in the ticket, and make the reader's outcome the tiebreak: they
must never see an empty card, and never see mechanical text wearing an
enrichment badge.

## Decision (recorded 2026-08-14)

**The scan publishes the mechanical sentence, and the description carries
provenance of its own** — the recommendation, taken. `Layer.description` is
an optional object, `{ text, provenance }` (`LayerDescription` in the
contract), not a string plus a sibling provenance field. Why:

- **The layer-name shape is the proven one.** The scan publishes, enrichment
  relabels, the annotation store carries it by derivation hash. Ticket 07
  needs only a new store map keyed by layer ID with the layers' existing
  `inputs_hash` (the sorted member paths — `semantic_hashes` already computes
  it) and a reattach that sets `description = { text, provenance: llm }`.
  No new mechanism, no corner painted.
- **One provenance per authored thing.** A flat `description` +
  `description_provenance` pair can drift apart (a producer publishing one
  without the other), and — decisive — the share walker reads each object's
  *own* `provenance` to decide redaction. As a sub-object, a purchased
  description on a mechanically named layer is redacted and the mechanical
  name ships, per part, with **zero changes to the redaction mechanism**;
  as a sibling field, the walker would have needed a special case keyed on
  the layer's name-provenance, which either leaks purchased prose or redacts
  plain structure. The schema also forces `text` and `provenance` to travel
  together (`required` inside `LayerDescription`).
- **One fallback, one language-border rule.** The mechanical sentence is the
  exact text the dashboard synthesised ("Files under crates/" / "Files at
  the repository root"), so a map with the field renders exactly as one
  without it — and the dashboard keeps its synthesis only for maps that
  predate 0.5.0 (and for blank-text descriptions, which fall back rather
  than render an empty card).

The tiebreak held both ways: the reader never sees an empty card (fallback
on absent *and* blank text, asserted), and mechanical text never wears an
enrichment badge — the badge renders only for `llm` description provenance,
and the scan test refuses a mechanical sentence published as `llm`.

*Correction from the crosscheck (2026-08-14): the clause "the header tally
and panels carry that disclosure" below was inaccurate — the tally counts
only nodes, and no panel badges layer names. The outcome (names unbadged on
cards) was nonetheless the correct pin of the pre-existing rendering;
CONTEXT.md's Provenance entry now states the true badging rule.*

On-card badging is the description's alone: the card badges the description
when (and only when) a model wrote it, and the *name* stays unbadged on the
card whatever its provenance — criterion 7 pins today's rendering for
description-less maps, and today's cards render enriched names unbadged
(the header tally and panels carry that disclosure). Per combination:
mech name + mech description → no badge; mech name + llm description → one
`llm` badge on the description row; llm name + mech description → no badge;
llm name + llm description → one `llm` badge, description row. The badge
sits outside the single-line-ellipsis text so a long description can never
clip its own label away, and the clamp (plus the projection's fixed
226×112 card) is what keeps a description from growing a card.

## Guards proven able to fail (2026-08-14)

Each mutation applied, watched fail, and reverted; suites green after.

- **Description absent from a fresh scan** (`semantics.rs` publishing
  `None`): `every_structural_layer_carries_a_mechanical_description` failed
  — `layer .github publishes no description`.
- **Mechanical text published as `llm`** (`provenance: Provenance::Llm` at
  the scan): same test failed — `mechanical description wearing enrichment
  provenance`.
- **Field made required** (`Option` removed from `map.rs`, schema
  regenerated): `a_map_written_before_layer_descriptions_still_validates`
  failed — `` `description` must stay optional — a map written before it
  cannot carry it: ["id", "name", "description", "provenance"]``.
- **Classification row missing** (`LayerDescription.text` deleted from
  `FIELD_CLASSIFICATIONS`): the exhaustiveness gate failed naming it —
  `contract fields missing from the share classification table:
  ["LayerDescription.text"]`.
- **Row misclassified `ShareSafe`**:
  `layer_descriptions_are_redacted_by_their_own_provenance` failed with the
  purchased sentence shipping verbatim (`SECRET-ENRICHED-DESCRIPTION…` where
  `[redacted]` was required).
- **Blank-text fallback dropped** (`regions.ts` accepting empty text):
  `synthesises when the map publishes no description — never an empty card`
  failed.
- **Badge rendered unconditionally** (`nodes.tsx`): both `never badges
  mechanical text as enrichment` and `renders a description-less map exactly
  as it did before the field existed` failed.
- **Badge never rendered**: `renders a purchased description with an llm
  badge on that part alone` failed.
- **Export count keyed on `layer.provenance` alone** (`ExportMenu.tsx`
  dropping the description term): `counts a purchased layer description as a
  slot of its own` failed.

The RED runs of 2026-08-14 also stand as proof for the new tests
themselves: all failed before the implementation existed (scan test on the
missing field, contract test on the unpublished property, share test on the
unredacted text, projection and render tests on the missing
`descriptionProvenance` and card text).

## Built (2026-08-14)

Contract **0.4.0 → 0.5.0** (new optional field = minor, per
`contract/README.md`): `MAP_CONTRACT_VERSION`, the schema `$id`, both
READMEs, `contract/map.schema.json` and `dashboard/src/map.generated.ts`
moved together; regeneration re-run and byte-stable (drift gate green).

`semantics::describe_layer` holds the mechanical sentence and
`assign_layers` publishes it for every structural layer under `structural`
provenance. Domains gained nothing — `domainRegions` marks its synthesised
text `structural` explicitly. `share.rs` classifies `Layer.description`
ShareSafe (container), `LayerDescription.text` **RedactedWhenLlm**
(follows `Layer.name`: mechanical prose restates directory structure the
artifact already ships; enriched prose may paraphrase proprietary logic),
`LayerDescription.provenance` ShareSafe. The dashboard's `Region` gained
`descriptionProvenance`; the card and the info panel's region rows badge
`llm` descriptions; the export menu's leak warning counts purchased
descriptions as their own slots. New stored fixture
`tests/fixtures/maps/known-good-layered.json` (layers, no descriptions)
sibling to ticket 02's optionality pattern.

Suites (2026-08-14): cargo test 258/258; sealed 220/220; agent-cli 246/246;
fmt clean; clippy ×3 clean; dashboard 278/278 + typecheck clean. Share
artifact of this repository measured **1,524,558 bytes** on 2026-08-14
(ceiling 2,097,152; test green). Ticket 07's slot: purchased descriptions go
in the annotation store keyed by layer ID, carried by the layer's existing
derivation hash, reattached as `{ text, provenance: llm }`.
