# CodeAtlas — pitch and ADR agenda

> Digested from the founder's pitch, 2026-08-07. Input for the `/adr` interview,
> to be read alongside `docs/research/2026-08-07-baseline-repoatlas-understand-anything.md`
> once that research completes.

## The pitch

CodeAtlas helps developers visualize and navigate codebases. It is inspired by
RepoAtlas (local checkout at `/home/memnoc/Code/RepoAtlas`), a hard fork of
Understand Anything — the shape language of both is exactly what CodeAtlas
should feel like, and Understand Anything proved genuinely useful in real use
(an interview). But both baselines are slow (~25 minutes per run) and carry
features the user neither needed nor understood.

CodeAtlas is a from-scratch rebuild: largely the same capabilities, but compact
and performant — run it against a codebase quickly and get a precise map of its
structure **and** the relationships between its components. Tech is chosen
deliberately; only what is needed goes in.

## ADR agenda

1. **Deterministic vs. LLM split in the pipeline.** The ~25-minute runs of the
   baselines are suspected to come from LLMs reading all the code. Structure
   and relationships (files, functions, imports, dependencies) are mechanically
   extractable by static analysis in seconds; the LLM's real value is the
   semantic layer (what a module *means*). Decide: deterministic-first
   pipeline, with the LLM as an optional annotator? Is there a structure-only
   fast mode with no LLM at all?

2. **Caching and storage model.** Design incremental analysis in from day one:
   graph keyed by file content hashes, re-runs only re-analyze changed files
   and the edges that touch them. Second runs should drop from minutes to
   seconds. Decide the storage format and invalidation rules.

3. **The map format as a public contract.** CodeAtlas should define a compact,
   documented intermediate format ("produce this file and I'll render it").
   That keeps the tool standalone while letting other producers exist — in
   particular a future Northstar skill that emits the map file for CodeAtlas to
   render. Decide the schema's scope and stability guarantees. (Building the
   Northstar skill itself is deferred; the contract decision is not.)

4. **V1 capability cut.** "Largely the same capabilities" needs a concrete
   list. Which baseline features make V1: knowledge graph, dashboard, tours,
   domain/business flows, diff/PR analysis, onboarding guides, chat over the
   graph, sharing/export? Ground the cut in the research doc's
   essential-vs-incidental findings.

5. **Security as a first-class requirement.** If pitched at work, security is
   the first audit gate. Working assumption: the company has approved passing
   code through a trusted LLM (Claude); *everything else must be offline*. No
   third-party services, no telemetry, no external calls from the dashboard;
   all artifacts stay local. Anything less makes CodeAtlas a pet-project tool,
   never industry-ready. Decide: the exact data-boundary guarantee, whether a
   fully-offline (no-LLM) mode exists for stricter environments (pairs
   naturally with agenda item 1), and what a shareable export may contain
   (redaction).

## Open questions parked for later

- Building the actual Northstar → CodeAtlas producer skill (after the map
  contract exists).
