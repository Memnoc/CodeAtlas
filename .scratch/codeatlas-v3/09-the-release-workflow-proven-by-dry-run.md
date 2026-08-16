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

**Status:** done

- [x] A release workflow beside CI, triggered by `v*` tags, with a dry-run
      entry point that builds and verifies everything and publishes
      nothing
- [x] Default and sealed binaries for Linux x86_64-musl, Linux
      aarch64-musl, macOS arm64 and macOS x86_64, named
      `codeatlas-<tag>-<target>` with a `-sealed` variant beside each;
      Linux binaries proven fully static; each artifact smoke-run on its
      own runner (scan a fixture tree, watch the mapped line)
- [x] The full CI gates re-run inside the workflow before anything would
      publish
- [x] A SHA-256 checksums file covering every artifact; build-provenance
      attestation produced and verified for each
- [x] The sealed probe run against the built release artifacts — sealed
      subject, default binary as live control — inside the workflow
- [x] The aarch64 open question (native arm runner versus cross-compile)
      resolved and recorded here on completion
- [x] A real dry run executed and linked here, every leg green
- [x] The `v0.1.0` tag is NOT cut in this ticket — that is the
      post-`/harden` ship action, gated by the fresh-machine walk
      (ADR-0014, spec story 15)

## Completion record (2026-08-16)

**The green dry run, every leg:**
<https://github.com/Memnoc/CodeAtlas/actions/runs/31963531524> —
dispatched via `workflow_dispatch` on `main`; six CI-gate jobs, four
build legs, the checksums/attestation/notes leg and the fresh-runner
checksums verification all green; the publish job skipped, which is the
dry run's inertness proof (it is gated on
`github.event_name == 'push' && startsWith(github.ref, 'refs/tags/v')`,
and a dispatch runs on a branch ref by construction). Artifacts as
built, named from the honestly derived placeholder `v0.1.0-dry` (Cargo
version + `-dry` — no tag exists, so no tag name is pretended):
`codeatlas-v0.1.0-dry-<target>` and `-sealed` for
`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-musl`,
`aarch64-apple-darwin`, `x86_64-apple-darwin`, plus
`codeatlas-v0.1.0-dry-checksums.txt`. Attestation was created for all
nine subjects and `gh attestation verify` passed for each inside the
run (the Sigstore log received hashes only; nothing was published).

**The aarch64 resolution: native arm runner (`ubuntu-24.04-arm`), not
cross-compilation.** The spec predicted the C grammar objects would make
native likely, and the dry run confirmed it: with nothing but
`musl-tools` (whose `musl-gcc` targets aarch64 natively on that runner,
so the seven grammars' C objects need no cross toolchain), the leg went
green on the first attempt — `file` reads statically linked ELF aarch64,
`ldd` says "not a dynamic executable", and both binaries smoke-ran and
took the sealed probe on real arm hardware. Cross-compilation could
never have run those last two steps on the builder at all; the runner
class is free for public repositories, so the cross-compile option had
no remaining advantage.

**Guards proven able to fail:**

- The smoke assertion tripped in-workflow via the `tamper_smoke`
  dispatch input (run
  <https://github.com/Memnoc/CodeAtlas/actions/runs/31963969990>):
  three build legs red at exactly the smoke step — "FAIL: expected the
  line 'mapped 2 files' from a three-file fixture, got: mapped 3 files"
  — and nowhere earlier; the run was then cancelled, its point made.
- Checksums are verified, not just generated: a separate job on a fresh
  runner re-downloads the artifacts and the checksums file it never
  wrote and runs `sha256sum --check --strict`, plus an eight-line count.

**What the first red taught:** dry run 1
(<https://github.com/Memnoc/CodeAtlas/actions/runs/31959629928>) failed
on both macOS legs — the first macOS builds this repository ever ran —
because `walkthrough.ts` and `Walkthrough.tsx` collide on a
case-insensitive filesystem and `./Walkthrough.js` resolved to the wrong
module. Fixed by renaming the steps module `walkthrough-steps.ts`; no
other pair of repository paths collides case-insensitively. Exactly the
class of defect the dry-run seam exists to catch before a tag does.
