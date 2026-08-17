<!--
The body every GitHub Release's notes are cut from. The release workflow
fills the {{slots}} from what it measured on the artifacts it just built;
nothing in a slot is typed by hand, and no slot value lives in this file —
a number written here would be a promise, and this repository publishes
measurements or nothing.

Everything outside the slots is standing prose: it ships with every
release, so each sentence must be true of the tagged binaries themselves.
Each claim below is held by a committed test named in docs/SECURITY.md;
if a sentence stops being true, this template is what must change.
-->

## What changed

{{highlights}}

## What needs no key, and what needs Claude

`scan`, `serve`, `diff` and `share` need no key and no account: they never
open a non-loopback socket, and the dashboard is compiled in, so the one
downloaded file is the whole install. `serve --open-code`, which lets the
dashboard show a mapped file's source, is the same kind of thing — it
changes what your own browser can be told, never what leaves the host.

Two flags reach a model, and only these two: `scan --enrich`, which buys
prose for the map, and `serve --ask`, which answers questions about it.
Both need Claude, in one of two ways: your own API key
(`--provider claude` — `ANTHROPIC_API_KEY` or an `ant auth login`
profile, billed per token to that account), or the Claude CLI you are
already logged into (`--provider cli:claude` — drawing on that
subscription's allowance; CodeAtlas never handles the credential). What a
model receives on each path is bounded and documented in
`docs/SECURITY.md`, and it never includes your file contents. Without a
provider you still get the complete structural map — enrichment relabels
what the scan found, it never creates it.

The `-sealed` binary beside each default one is the build where this
section is enforced by the compiler: built `--no-default-features`, it
contains neither path to a model, every no-key command above works, and
`--enrich` and `--ask` refuse with a message saying the build has no
backend. It is the binary to hand a security review.

## Artifacts

<!-- Filled by the workflow: one row per artifact it built and attested —
     a default and a `-sealed` binary for each supported target, named
     codeatlas-<tag>-<target>[-sealed], with the checksums file beside
     them. -->

{{artifact-list}}

## Verify what you downloaded

Every artifact is listed in `{{checksums-file}}`; check the one you took:

```sh
{{checksums-verify-command}}
```

Every artifact carries a GitHub build-provenance attestation tying it to
the exact workflow run and commit that built it:

```sh
{{attestation-verify-command}}
```

## How this software was built

CodeAtlas is built AI-assisted, under the Northstar engineering pipeline:
every change arrives as a spec'd, ticketed slice, is built test-first, and
is cross-checked against the spec and the house standards before it lands.
The decisions are a human's, recorded as ADRs in `docs/adr/`. The same
disclosure runs through the artifact: prose a model wrote inside a map
always says so — the annotation store carries one record naming the
provider, the model and the UTC date of the last run that wrote it, and
prose bought by earlier runs rides beneath that latest record; the
dashboard badges enriched prose where it renders it, and `share` redacts
it from the exported file. The security posture — what can reach a model,
what it receives, and the committed test behind each claim — is
`docs/SECURITY.md`.

## Transparency

CodeAtlas's AI is strictly bring-your-own: `--ask` and enrichment call
Anthropic's Claude with credentials you supply, and nothing else in the
tool talks to a model — the sealed build cannot even be compiled to.
Wherever AI-written prose appears it says so: the dashboard badges
enriched text where it renders it, the annotation store carries a
machine-readable record naming the provider, the model and the UTC date
of the last run that wrote it, and `share` removes AI prose from the
exported file entirely. Interaction with the model is always labelled as
interaction with the model. This is stated as practice, verified by the
tests `docs/SECURITY.md` names — not as a reading of where any law's
lines fall — so a reader never has to guess which words a model wrote.
