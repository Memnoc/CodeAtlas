---
status: accepted
date: 2026-08-16
proposed-by: Memnoc
approved-by: Memnoc
---

# ADR-0013: Open code is a flag-gated serve route, highlighted by the vendored grammars

## Context

The dashboard can name a file but never show it; the V2 spec parked "open
code" (opening the selected file or symbol as highlighted source) with three
recorded blockers, one of which — "syntax highlighting has no path under
[ADR-0006](./0006-zero-egress-enforced-by-compile-time-feature-gate.md)" —
the V3 interview found stale: seven tree-sitter grammars are already
compiled into the binary, and upstream's `tree-sitter-highlight` runs on
exactly those grammars in-process. The remaining two blockers are real and
are what this decision answers: a source route falsifies
`docs/SECURITY.md`'s what-a-connection-can-be-told claim, and source cannot
ride a share artifact under
[ADR-0011](./0011-no-layout-library-a-share-ceiling-enforces-it.md)'s
two-megabyte ceiling.

## Decision

`serve --open-code` registers a source route; without the flag the route
does not exist — the `--ask` pattern, so a plain `serve` still never serves
your source and the audit sentence stays literally true. Only files that
are nodes in the map are servable (map membership is the allowlist; there
is no path-addressed file serving), opened at file or symbol level via the
contract's existing `range`, highlighted server-side by the vendored
grammars with a plain-text fallback, returned as a JSON envelope
(highlighted HTML plus path, language, and truncation metadata), and
truncated past a named size cap with the truncation disclosed rather than
refused. In plain terms: with the flag, the map can show you the code
itself; without it, the server still cannot.

## Considered options

- **Flag-gated route, map-membership allowlist, server-side tree-sitter
  highlighting** — chosen because a loopback port is readable by any local
  user while file permissions are not, so default-off is a real
  confidentiality line on shared machines, not paperwork; map membership
  leaves no path-traversal class to defend; and the vendored grammars make
  highlighting zero-egress with zero dashboard-bundle growth.
- **An always-on source route** — rejected: it widens what any local
  process can read on a multi-user host and falsifies the
  never-serves-your-source posture for every `serve`, not just consenting
  ones.
- **A client-side highlighting library** — rejected: it bundles into the
  dashboard and therefore into the share artifact, pressing ADR-0011's
  ceiling for a feature share does not carry.
- **Source in share artifacts** — rejected: a share recipient is precisely
  someone who does not hold the source; embedding it contradicts the
  redaction trust boundary before the ceiling is even reached.
- **Refusing over-cap files outright** — rejected: the
  refuse-don't-truncate precedent exists because a truncated *question* is
  silently a different question; a truncated *display* that says so is
  honest, and most of a large file's value is at its top.

## Consequences

`docs/SECURITY.md`'s "what a connection can be told" list gains the
flag-gated route, and the route drift tests force that naming — the
document cannot silently go stale. The capabilities route grows a second
boolean beside `ask`, which is how the dashboard and the share artifact
know the feature is absent. The boundary this decision does **not** move:
open code changes what the reader's *browser* can be told, and nothing
about what a *model* receives — the ask path still never sends file
contents, and feeding opened source into ask's slice would be a separate
decision with its own bounds story. A file that changed on disk since the
scan is served live; a deleted one draws an honest 404.
