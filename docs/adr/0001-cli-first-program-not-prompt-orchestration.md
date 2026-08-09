---
status: accepted
date: 2026-08-07
proposed-by: Claude Fable 5
approved-by: Memnoc
---

# ADR-0001: CodeAtlas is a CLI-first program; skills are thin wrappers

## Context

Both baseline tools (Understand Anything and its fork RepoAtlas) drive their
analysis pipeline by having an LLM read an 850–1,150-line SKILL.md and execute
bash blocks turn by turn. RepoAtlas measured the cost of that design: ~25
minutes of narrated orchestration for ~2.3 seconds of actual script work on a
458-file repo.

## Decision

CodeAtlas is a standalone command-line program that owns the entire pipeline —
scan, extract, graph-build, validate, save, and optional LLM enrichment
dispatch. Any Claude Code skill is a thin wrapper (~20 lines) that invokes the
CLI and reports its result, never an orchestrator. In plain terms: the code
runs the analysis; an AI model is only ever asked to write prose about an
already-built map.

## Considered options

- **CLI-first, skill-thin** — chosen because a program is fast (no model
  round-trips between phases), testable (including its network behavior),
  usable outside Claude Code, and needs none of the defensive prose both
  baselines carry.
- **Claude Code plugin like the baselines** — rejected because it inherits the
  orchestration tax and makes security guarantees unverifiable (a prompt's
  egress cannot be tested).
- **CLI core with rich host-session enrichment (RepoAtlas `--enrich` model)** —
  rejected because it splits pipeline ownership across two systems and keeps
  the LLM-output-repair machinery alive.
