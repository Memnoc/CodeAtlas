# Ticket 33 — story 2's convention checklist becomes a fixture table

**Status:** ready
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 2 — the map captures relationships, not just a file tree (as
rewritten 2026-08-11 with a finite convention checklist)
**Blocks:** none
**Blocked by:** none

## Problem

Story 2 failed three consecutive `/harden` walks — TypeScript (ticket 17),
Rust (18), Python (20) — and each failure was found by widening the
per-language probe rather than by changing the standard. That is not bad luck.
Its scope was open-ended by construction: "every import convention in six
languages" has no bottom, so there is always a seventh convention and the
story can never be finished.

The spec rewrote it on 2026-08-11 as a closed checklist. This ticket makes
that checklist real, so that coverage is something a reader can see rather
than something a walk discovers.

## What to build

A fixture table with one row per convention per language, where a gap reads as
a failing row rather than as an absence nobody notices. After this ticket,
"does Go resolve an aliased import?" is answered by looking, not by writing a
new probe.

## Acceptance criteria

- [ ] Every row of the spec's checklist has a named test, in each of the six
      V1 languages where that convention exists.
- [ ] Rows for conventions a language does not have are marked as
      not-applicable rather than silently missing, so the table's shape shows
      coverage at a glance.
- [ ] The three **non-edge** rows are covered as carefully as the positive
      ones: a call whose receiver is a value rather than a module, a call into
      a package outside the repository, and an import resolving to no file in
      the repository. These are where the resolver invents edges, and ticket
      21 shipped a fabricated-edge bug that seven mutations missed because no
      fixture had a decoy.
- [ ] Existing fixtures are reused where they already cover a row; only the
      gaps get new ones.
- [ ] Every row either passes, or is **filed as its own ticket** and listed
      here with its number. This ticket is complete when the table is complete
      and every failing row has somewhere to live — not when every convention
      in six languages works.

## Notes

**The escape hatch in the last criterion is deliberate and was agreed
explicitly.** Three walks found one gap each; this table will probably find
more, and some may be as large as ticket 21 was. Without the hatch this ticket
is unbounded and cannot fit a session. With it, the *table* is the deliverable
and the fixes are sequenced honestly.

Fourteen rows across six languages is up to eighty-four cells, which sounds
enormous and is not: most rows already have a test somewhere in
`crates/codeatlas/tests/scan.rs` from tickets 17, 18, 20 and 21. The work is
mostly finding them, naming them consistently, and writing down which cells
are empty.

The reason to do this at all, rather than accepting story 2 as perpetually
open: `/harden` walks the numbered story list, and a story that cannot be
finished means the release cannot be finished either.
