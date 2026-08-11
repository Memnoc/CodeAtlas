# Ticket 30 — enrichment arrives with the repository

**Status:** ready
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 18 — enrichment someone else already paid for arrives with the
repository, so cloning is all I have to do
**Blocks:** none
**Blocked by:** none

## Problem

Enrichment is a per-developer purchase. `.codeatlas/` is git-ignored, so prose
one person paid for is invisible to everyone else, and a colleague without a
credential sees a structurally complete map with nothing but mechanical labels
on it.

The mechanism to fix this already exists and is one line from working: the
annotation store re-attaches on *every* scan, before the enrichment branch is
even considered, so a plain `codeatlas scan` with no credential and no network
already consumes committed prose. The only blocker is that the file cannot be
committed.

## What to build

One person enriches, commits, pushes. Everyone else clones and runs a plain
`codeatlas scan` — no credential, no network, no flags — and gets the map with
all its prose. The store says what produced that prose, so a reviewer looking
at the diff can see whether it came from Opus 5 through the API, a
subscription CLI, or something else.

## Acceptance criteria

- [ ] A scan writes `.codeatlas/.gitignore` that ignores the directory's
      contents and un-ignores the annotation store, so `git check-ignore`
      classifies the map file as ignored and the annotation store as not.
- [ ] A `.codeatlas/.gitignore` the user has edited is never clobbered — see
      Notes, this refines rather than contradicts ADR-0007.
- [ ] The store records the provider, the model, and the date that produced
      its prose.
- [ ] Those fields are additive: a store written before this ticket still
      loads and still re-attaches, without a store-version bump.
- [ ] The story's actual claim is tested end to end — enrich a fixture with a
      fake provider, discard the map file, run a plain `scan` with no provider
      selected at all, and assert the prose is back and provenance is `llm`.
- [ ] The regenerated map file stays ignored. It is ~790 KB on this repository
      and rebuilt every run; committing it would be pure diff noise.

## Notes

**ADR-0007 says "every scan writes a `.codeatlas/.gitignore`" and does not
settle what happens when one already exists.** This ticket settles it: write
the file when it is absent or byte-identical to the default, and leave a
modified one alone. Clobbering would silently discard a deliberate choice —
someone who decided *not* to publish their prose has made a real decision, and
a tool that overwrites it every scan is a tool people stop trusting with their
repository.

The reconciliation with ticket 14's redaction is recorded in ADR-0007 and does
not need restating in code: the line is the trust boundary, not the prose. A
share artifact goes to someone who does not hold the source; a committed store
goes only to people who already do.

Worth knowing before starting: enrichment has never actually run on this
repository, so there is no `annotations.json` here to look at. The fixture
route through a fake provider is the only way to produce one without spend.
