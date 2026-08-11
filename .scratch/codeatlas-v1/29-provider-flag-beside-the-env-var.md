# Ticket 29 — provider selection gets a flag, not just an env var

**Status:** done
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 19 — supporting. Provider selection is the surface story 19's
second backend arrives on.
**Blocks:** 31
**Blocked by:** none

## Problem

The only way to choose an enrichment backend is `CODEATLAS_ENRICH_PROVIDER`.
Nobody finds an environment variable, so in practice there is one provider and
no way to learn otherwise — which is the same discoverability failure that
made the whole credential problem invisible until someone asked out loud.

Ticket 31 adds a second real backend. It should land on a selection surface
that is already clean rather than widening a hidden one: make the change easy,
then make the easy change.

## What to build

`codeatlas scan --enrich --provider <spec>` selects the backend. The
environment variable keeps working for anyone already scripting against it,
and `--help` says which specs exist, so the second credential path is
discoverable by the ordinary means of reading the help.

## Acceptance criteria

- [x] `--provider <spec>` on `scan`, requiring `--enrich`, in the same shape
      as the existing `--model`.
- [x] An explicit flag beats an explicit environment variable; with no flag,
      the variable is honoured exactly as it is today. Tested in **both**
      directions — see Notes, the one-directional version passes vacuously.
- [x] `--help` names the recognised specs for the build it is running in, so
      the recognised set is discoverable without reading source.
- [x] An unrecognised spec fails with a message that lists what *is*
      recognised — and the structural map is still written, because provider
      selection failing is an enrichment failure (story 14).
- [x] In the sealed build the flag exists and explains that enrichment is not
      available in this build, rather than being silently absent. Verified
      against a **genuinely sealed binary** in `scripts/sealed-probe.sh`, with
      the default build as a live control — no `cargo test` build can reach
      this branch at all. See Notes.
- [x] Driven at the CLI boundary (seam 1) against the real binary, not by
      unit-testing the resolver alone. Three unit tests sit beneath it for
      facts the rendered help cannot pin — see Notes.

## Notes

Provider resolution already has a well-factored seam — `provider_from_spec`
takes an `Option<&str>` precisely so selection is unit-testable, with
`default_provider` behind it deciding per build configuration. This ticket
should feed the flag into that existing function rather than growing a second
resolution path beside it.

Recognised specs differ per build (`network`, `agent-cli`, `test-provider`
each add their own), which is why the help text has to be built from the
compiled-in set rather than hardcoded.

## What the work found

**Two help strings were lying in the sealed build.** A test asserting that a
sealed build names no provider it cannot select failed twice, on strings this
ticket had not planned to touch: `--enrich` said *"The default provider is the
Claude API"* and `--model` said *"Model for the Claude enrichment provider
(default: claude-opus-5)"*. Neither is true where the Claude provider does not
exist. Both are now built from the compiled-in set like `--provider`'s, with
`--model` in a sealed build saying plainly that it has nothing to modify.

The blunt version of that assertion — *the word "claude" appears nowhere in
sealed help* — was too strong to keep, because explaining **why** a provider is
absent requires naming it. It was replaced with an assertion that the sealed
build explains itself, plus a unit test pinning the recognised list directly.

**Precedence needs testing in both directions.** "The flag beats the
environment variable" passes just as well if the variable is ignored
altogether, which is a different and worse behaviour. There is a second test
where the variable names a working backend and the flag names the failing one,
and the run must fail.

**Three facts moved to unit tests** because clap wraps help at the terminal
width, so asserting on a rendered list tests the wrapping as much as the
content: which specs the build recognises, that an unknown spec is reported
with the alternatives, and that an explicit spec outranks a fallback.

**`ProviderChoice` replaced two positional `Option<&str>` parameters** that
travelled together through three call layers and were silently swappable at
every one.

**Seven mutations, all killed**: precedence reversed, the flag ignored, the
error listing nothing, the help naming nothing, `requires = "enrich"` dropped,
`claude` offered unconditionally, and the sealed explanations removed. The last
two fail only in the sealed configuration, which is why CI runs both.

### What `/crosscheck` found

**The worst finding would have broken ticket 31.** `model_help()` and both
"no backend" messages keyed on `#[cfg(feature = "network")]`, which reads
correctly today and becomes a lie the moment ADR-0008's `agent-cli` feature
exists: a build with the CLI provider and no HTTP client would have announced
itself as a sealed build with no backend, while `--provider cli:claude`
worked. That is the third configuration ADR-0008 exists to make expressible,
and this ticket would have shipped help that lies about it. Everything now
derives from `recognised_specs()` through one shared `recognised_sentence()`,
and **no message keys on a feature name**.

**Two of the ticked criteria rested on assertions that could not fail** — the
fourth and fifth time this project has hit that:

- The sealed test asserted the page contained "sealed build", which
  `--model`'s text satisfied on its own. `--provider`'s explanation could have
  been deleted whole and the test would still pass. It now asserts on
  `--model`'s own paragraph, and `--provider`'s is covered by the probe.
- The help test asserted only that `fake:` was listed, which is true in every
  configuration. A hardcoded string naming every spec that has ever existed
  would have passed — the exact failure the help is built dynamically to
  prevent. It now asserts `claude` appears **exactly when** the build has it.

**Criterion 5 was unverifiable where it was being tested.** The "recognises
none" branch needs a build with neither `network` nor `test-provider`, and the
self dev-dependency means no `cargo test` build is ever that. It moved to
`scripts/sealed-probe.sh`, which is the only place a genuinely sealed binary
exists, with the default binary as a live control. Both new probe assertions
were mutation-checked against real binaries.

Also fixed: the module header still described env-var-only selection; the
`ProviderChoice` doc claimed three call layers when it is unpacked at the
first; `provider_from_spec`'s doc claimed to decide precedence it does not
decide; a unit test named `an_explicit_spec_outranks_the_environment` never
touched the environment; and an import was orphaned below three functions.

One `/crosscheck` finding was **not** acted on: that `provider_help` and
`unknown_provider` duplicate a shape. They now share `recognised_sentence()`,
which was the substance of it.

**Unrelated, found and fixed:** `cargo fmt --all --check` is a CI step and had
been failing since ticket 21 — `tests/scan.rs` went in unformatted and nobody
noticed, because the local loop runs tests and clippy but never fmt. CI has
been red on `main` since. The workspace is formatted in this commit, which is
why the diff touches `tests/scan.rs`.
