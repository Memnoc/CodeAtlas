# 07 — Published contract: TS types + drift CI

**Spec:** docs/specs/2026-08-09-codeatlas-v1.md

**What to build:** The map format becomes the public, versioned contract of
ADR-0003: the generated JSON Schema is committed as the official artifact,
the dashboard's TypeScript types are generated from it, and CI fails whenever
committed and regenerated artifacts drift. From this point on, any producer —
including a future Northstar skill — can target the contract.

**Blocked by:** 01 — Walking skeleton.

**Status:** ready

- [ ] The generated JSON Schema is committed at a documented path with a
      semver version, and a short doc states the versioning/breaking-change
      policy
- [ ] TypeScript types are generated from the schema; the dashboard package
      consumes only these generated types
- [ ] CI regenerates schema and TS types and fails on any diff against the
      committed artifacts
- [ ] A contract test validates a known-good fixture map against the
      committed schema (not just the in-memory structs)
