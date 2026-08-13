# Ticket 06 — a region card says what the region is

**Status:** ready
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

- [ ] Layers carry an optional `description` in the contract; structural
      layers only.
- [ ] Domains gain nothing — they are derived from flows and are not contract
      entities, and their synthesised text is already honest (spec, Out of
      Scope).
- [ ] Old maps validate unchanged, asserted against a stored fixture map
      lacking the field.
- [ ] Schema and generated TypeScript regenerate together; the drift gate
      passes (ADR-0003 ceremony).
- [ ] The region card renders the description, and the overview stays
      readable at this repository's eight layers — a description never grows
      a card without bound.
- [ ] Provenance is badged, and mechanical text is never badged `llm`.
- [ ] A map whose layers carry no description renders exactly as it does
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
