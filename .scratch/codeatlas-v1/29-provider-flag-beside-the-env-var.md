# Ticket 29 — provider selection gets a flag, not just an env var

**Status:** ready
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

- [ ] `--provider <spec>` on `scan`, requiring `--enrich`, in the same shape
      as the existing `--model`.
- [ ] An explicit flag beats an explicit environment variable; with no flag,
      the variable is honoured exactly as it is today.
- [ ] `--help` names the recognised specs for the build it is running in, so
      the recognised set is discoverable without reading source.
- [ ] An unrecognised spec fails with a message that lists what *is*
      recognised — and the structural map is still written, because provider
      selection failing is an enrichment failure (story 14).
- [ ] In the sealed build the flag exists and explains that enrichment is not
      available in this build, rather than being silently absent.
- [ ] Driven at the CLI boundary (seam 1) against the real binary, not by
      unit-testing the resolver alone.

## Notes

Provider resolution already has a well-factored seam — `provider_from_spec`
takes an `Option<&str>` precisely so selection is unit-testable, with
`default_provider` behind it deciding per build configuration. This ticket
should feed the flag into that existing function rather than growing a second
resolution path beside it.

Recognised specs differ per build (`network`, `agent-cli`, `test-provider`
each add their own), which is why the help text has to be built from the
compiled-in set rather than hardcoded.
