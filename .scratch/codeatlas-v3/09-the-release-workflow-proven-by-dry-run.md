# 09 — The release workflow, proven by dry run

**Spec:** `docs/specs/2026-08-16-codeatlas-v3.md` — a fresh session reads
this first, then ADR-0014.

**What to build:** A version tag builds the default and sealed binaries
for all four targets, with checksums and build-provenance attestation,
gated by the full CI suite and the sealed probe run against the built
artifacts themselves — and a dry-run entry point proves all of it while
publishing nothing. This ticket ends with a green dry run, not a release.

**Blocked by:** 07 — The store survives strangers; 08 — The release
documents. (Working the open-code tickets first is the spec's recorded
sequencing; it is not an edge — the workflow builds whatever `main`
holds.)

**Status:** in-progress

- [ ] A release workflow beside CI, triggered by `v*` tags, with a dry-run
      entry point that builds and verifies everything and publishes
      nothing
- [ ] Default and sealed binaries for Linux x86_64-musl, Linux
      aarch64-musl, macOS arm64 and macOS x86_64, named
      `codeatlas-<tag>-<target>` with a `-sealed` variant beside each;
      Linux binaries proven fully static; each artifact smoke-run on its
      own runner (scan a fixture tree, watch the mapped line)
- [ ] The full CI gates re-run inside the workflow before anything would
      publish
- [ ] A SHA-256 checksums file covering every artifact; build-provenance
      attestation produced and verified for each
- [ ] The sealed probe run against the built release artifacts — sealed
      subject, default binary as live control — inside the workflow
- [ ] The aarch64 open question (native arm runner versus cross-compile)
      resolved and recorded here on completion
- [ ] A real dry run executed and linked here, every leg green
- [ ] The `v0.1.0` tag is NOT cut in this ticket — that is the
      post-`/harden` ship action, gated by the fresh-machine walk
      (ADR-0014, spec story 15)
