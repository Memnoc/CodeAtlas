# Ticket 01 — the share artifact has a ceiling

**Status:** done
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 23 — a committed test that fails when the share artifact exceeds
its ceiling, so that growth is a decision, never an accident
**Blocks:** none formally — but it is first on purpose: every visualization
ticket after it is then measured under the constraint rather than against it
**Blocked by:** none — can start immediately

## Problem

The share artifact has grown roughly 60% since it was first measured, and
nothing notices. It measured 1,353,614 bytes for this repository's own map on
2026-08-13. There is no size at which anything fails, so the artifact can
become too big to hand to anyone one commit at a time, with no single commit
to blame.

[ADR-0011](../../docs/adr/0011-no-layout-library-a-share-ceiling-enforces-it.md)
makes this test the *enforcement* of the no-layout-library decision: the
rejection of dagre and elkjs is only as durable as something that fails when
a dependency's weight lands in the artifact.

## What to build

A committed test that exports a share artifact and fails when it exceeds two
megabytes, naming the measured size and the ceiling.

## Acceptance criteria

- [x] Exporting a share artifact and measuring its bytes is a committed test;
      above the ceiling it fails.
- [x] The ceiling lives in one named constant, with ADR-0011 cited where it
      is defined. The constant states its unit unambiguously — bytes, with
      the megabyte definition written down once.
- [x] The failure message carries both numbers, measured and ceiling, so the
      person who trips it knows how far over they are without re-running
      anything.
- [x] Proven able to fail: lower the constant below the current size, watch
      it trip with the right message, restore it.
- [x] It rides the existing share suite and adds no dependency (ADR-0006).

## Notes

Measure what a user receives — the artifact's own bytes on disk, not the
payload before templating and not a compressed size no one downloads.

Do not add a warning band, a trend file, or a second threshold. One number,
one failure. The ceiling exists so that raising it is a visible decision in a
diff; anything softer restores the situation this ticket closes.

## Built

The test scans this repository and shares it, the way the self-scan test in
`tests/scan.rs` dogfoods: the fixture map the other share tests use would
weigh the embedded dashboard and almost nothing else, and a ceiling with that
much slack could not trip. Measured on 2026-08-13 with this ticket's changes
in the tree: `.codeatlas/share.html` for CodeAtlas's own map is 1,364,909
bytes against the 2,097,152-byte ceiling. Proven able to fail by lowering
`SHARE_CEILING_BYTES` to 1,000,000: the test reported "the share artifact is
1364909 bytes — 364909 bytes over the ceiling of 1000000 bytes (ADR-0011)".
