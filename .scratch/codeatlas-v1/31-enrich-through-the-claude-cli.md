# Ticket 31 — enrich without ever handling a credential

**Status:** ready
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 19 — enrich through a Claude CLI I am already logged into, so that
CodeAtlas never handles a credential
**Blocks:** 32
**Blocked by:** none — ticket 29 landed the `--provider` flag

## Problem

For anyone outside Anthropic there is exactly one way to enrich: an
`ANTHROPIC_API_KEY`. In many organisations only administrators can obtain one,
which puts the entire explanatory half of the product out of reach for most of
a team — including the person who would otherwise produce the committed store
of ticket 30.

[ADR-0008](../../docs/adr/0008-enrichment-through-an-authenticated-claude-cli-behind-its-own-feature.md)
settles the shape. ADR-0004 had parked this and rejected it partly because
"output is free text (the repair problem returns)"; the CLI's `--json-schema`
retires that objection, so the same typed-slot exchange works through a
subprocess as through HTTPS.

## What to build

Someone with a Claude Code seat and no API key runs
`codeatlas scan --enrich --provider cli:claude` and gets an enriched map.
CodeAtlas never sees a credential — the CLI uses its own. Every failure mode
leaves a complete structural map behind.

## Acceptance criteria

- [ ] A new `agent-cli` Cargo feature, in the default set and **separate from
      `network`**. A subprocess contains no HTTP client, and filing it under
      `network` would make that feature's name false.
- [ ] `--provider cli:claude` fills the same typed slots as the API provider,
      through the same provider trait, from a schema-constrained completion.
- [ ] The child process is a completion, not an agent: no tools, no MCP
      servers, and a working directory outside the repository. Without this
      the CLI can read files through its own tooling and void the standing
      guarantee that the model never receives file contents.
- [ ] The child's environment is an explicit allowlist, and
      `ANTHROPIC_API_KEY` is **not** in it — `cli:` must unambiguously mean
      the CLI's own credential rather than silently billing the API through a
      subprocess.
- [ ] Every failure mode leaves a complete, schema-valid structural map
      (story 14): the program is not installed, it is installed but not
      logged in, it exits non-zero, or its output does not parse.
- [ ] `cli:` with any program other than `claude` is rejected. A generic
      `cli:<program>` would make "CodeAtlas executes whatever you name it" a
      true sentence.
- [ ] Tested at **seam 3** against a fake executable that echoes canned JSON:
      assertions cover the argv it was invoked with, the environment it did
      and did not receive, the working directory, and what the provider made
      of what came back.
- [ ] The fake-executable injection point compiles only under
      `test-provider`, exactly as the `fake:` and `fail` backends already do,
      so no shipped binary gains a way to run an arbitrary program.
- [ ] No test spawns the real `claude`. No test performs network I/O.

## Notes

**The four things that actually break here are all below the provider trait**,
which is why seam 3 exists: argv construction, environment scrubbing, stdout
parsing, and exit-code handling. Seam 2 cannot see any of them — a fake
`EnrichmentProvider` would pass while every one of those was wrong.

Verified on this machine at ticket-writing time (`claude` 2.1.227): the flags
that matter are `-p`, `--output-format json`, `--json-schema <schema>`, and
`--model`. Two cautions found while reading the help. `--bare` looks
attractive for a minimal invocation but explicitly *"skips keychain reads"* and
takes auth strictly from `ANTHROPIC_API_KEY` — the exact opposite of what this
provider is for, so do not use it. And the exact envelope `--output-format
json` wraps the schema-constrained result in has not been confirmed against a
live run; confirm it at implementation time rather than guessing, since the
parser depends on it.

**What ticket 29 left for this ticket.** Provider selection is now one
surface: `--provider` beats `CODEATLAS_ENRICH_PROVIDER` beats the build
default, and every message that names the alternatives — `--provider` help,
`--model` help, the unknown-spec error — renders from a single
`recognised_specs()` list through one shared sentence. **Adding `cli:claude`
means adding it to that list and nothing else**; no message keys on a feature
name, precisely so that an `agent-cli`-without-`network` build does not
describe itself as having no backend. Do not reintroduce a `#[cfg(feature =
"network")]` in any user-facing string.

The provider's own default model should be left alone rather than pinned to
`claude-opus-5` like the API provider: a subscription's entitlement varies, and
pinning a model the seat cannot use turns a working setup into an error.
