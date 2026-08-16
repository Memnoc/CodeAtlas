---
status: accepted
date: 2026-08-16
proposed-by: Memnoc
approved-by: Memnoc
---

# ADR-0014: Distribution is attested GitHub Releases, sealed beside default

## Context

V2 shipped 2026-08-14 and the standing polish-before-distribution decision
makes distribution the V3 lap; nothing has ever been tagged or released,
the repository is public, and CI already gates all three feature
configurations plus the sealed byte probe on every push.

## Decision

Releases are tag-triggered (`v0.1.0` first — 0.x is the honest signal while
no external user exists) GitHub Releases carrying **both** the default and
the sealed binary for four targets — Linux x86_64-musl, Linux aarch64-musl,
macOS arm64, macOS x86_64 — each release with a SHA-256 checksums file and
GitHub build-provenance attestation, no GPG. The release workflow re-runs
the full CI gates and runs `scripts/sealed-probe.sh` against the release
artifacts themselves, and the first public tag is additionally gated by a
fresh-machine walk written into the V3 spec as a story (download → scan a
repository that is not CodeAtlas → serve → ask), so "ready to distribute"
is verified by `/harden` like any other story. In plain terms: one
downloaded file works, its origin is machine-verifiable, and the build
where exfiltration is a compile error ships beside the standard one.

## Considered options

- **GitHub Releases with attested static binaries, sealed beside default**
  — chosen because the dashboard is already embedded so a single file
  works with zero runtime dependencies; the sealed binary is the audit
  story in distributable form, and probing the shipped bytes is stronger
  evidence than probing a CI build; attestation is free, machine-verifiable
  provenance.
- **Publishing to crates.io** — deferred to its own future decision: the
  build embeds the dashboard via `build.rs`, so `cargo install` would
  require node/npm on every user's machine — a support surface with no
  demand yet. The crate name was checked free on 2026-08-16 and is left
  unregistered; squatting an empty crate is its own decision nobody has
  made.
- **GPG signing** — rejected: key-management burden on a solo project with
  no verifier audience; checksums plus workflow attestation cover integrity
  and origin.
- **A Windows target** — deferred until someone asks: several suites lean
  on Linux-only probes and no request exists.
- **Default-configuration binaries only** — rejected: shipping the sealed
  build is the strongest claim this project's distribution can make, and
  the one a security review downloads first.

## Consequences

Release notes carry the provenance paragraph — built AI-assisted under the
Northstar pipeline; annotation-store prose self-discloses provider, model
and date — and say loudly that no key is needed for scan/serve/diff/share.
The README's Quick start gains a download-first install section, with
build-from-source demoted to second. The map contract keeps its own
versioning (0.5.0 at the time of writing), deliberately separate from the
binary's tags. The annotation store must become forward-compatible
(unknown sections preserved on read and rewrite) **before** the first
public tag — the last moment at which no distributed binary can yet
silently drop a newer store's purchased prose.
