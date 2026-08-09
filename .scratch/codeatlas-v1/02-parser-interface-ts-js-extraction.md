# 02 — Parser interface + TS/JS structural extraction

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** Scanning a TypeScript/JavaScript repo now yields function
and class nodes inside each file node, with line ranges, `contains` edges,
and mechanical summaries ("TypeScript file, 214 lines: 3 functions"). This
ticket establishes the one parser interface every later language implements;
files the parser cannot handle degrade to bare file nodes instead of failing
the run (spec story 15).

**Blocked by:** 01 — Walking skeleton.

**Status:** done

- [x] A per-language parser interface exists; TS/JS is its first
      implementation, grammar compiled into the binary (no runtime downloads,
      ADR-0006)
- [x] Function and class nodes carry typed IDs, line ranges, and `contains`
      edges from their file
- [x] Every node gets a mechanical summary and `provenance: structural`
- [x] A file with syntax errors or an unsupported extension still appears as
      a file node; the run completes
- [x] Fixture-repo test asserts the expected function/class nodes and
      schema-validity of the map
