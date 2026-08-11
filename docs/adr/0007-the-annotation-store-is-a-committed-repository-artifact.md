---
status: accepted
date: 2026-08-11
proposed-by: Claude Opus 5
approved-by: Memnoc
---

# ADR-0007: The annotation store is a committed repository artifact

## Context

LLM enrichment is purchased per developer: `.codeatlas/` is git-ignored, so
prose one person paid for is invisible to everyone else, and a colleague
without credentials sees a purely mechanical map. In many organisations only
administrators can obtain an API key at all, which makes enrichment
unreachable for most of a team rather than merely expensive.

## Decision

`.codeatlas/annotations.json` is a committed repository artifact: every scan
writes a `.codeatlas/.gitignore` that ignores the regenerated map and
publishes the annotation store, and the store records which provider, model,
and date produced its prose. In plain terms: one person enriches, commits the
result, and everyone who clones the repository gets the explanations for free
and offline.

## Considered options

- **Committed by default via a tool-written nested `.gitignore`** — chosen
  because the artifact has to exist without ceremony; the person who paid for
  enrichment should not also have to remember to publish it.
- **An explicit `codeatlas export-annotations` command** — rejected because a
  second command that must be remembered relocates the friction instead of
  removing it, and an artifact that only sometimes exists cannot be relied on
  by anyone downstream.
- **Leave `.codeatlas/` ignored and document the workaround** — rejected;
  documentation without a mechanism is the posture this project exists to
  avoid.

## Consequences

This commits LLM-written prose into the repository while ADR-0006's share
artifact redacts the same prose out. Both are correct because **the line is
the trust boundary, not the prose**: a share artifact goes to a recipient
chosen at send time who does not have the source, so its sender cannot audit
who reads it; a committed annotation store goes only to people who already
have the code it describes, and so discloses nothing they could not already
read.

The store gains provider, model, and date fields, because prose entering code
review needs to say what produced it. `knowledge-graph.json` stays ignored —
it is regenerated every run (791 KB on this repository) and would be pure
diff noise.
