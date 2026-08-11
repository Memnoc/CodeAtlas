# Ticket 32 — three build configurations, and the claims that describe them

**Status:** blocked
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 9 — a sealed build plus an egress suite, so approving CodeAtlas is
a code review rather than a trust exercise (as amended 2026-08-11)
**Blocks:** none
**Blocked by:** 31 (the CLI provider) **and** 34 (`serve --ask`)

## Problem

Two new egress routes exist by the time this ticket starts, and the documents
that describe the security posture still describe one. `README.md` and
`docs/SECURITY.md` both say `--enrich` is the only egress-capable command and
`api.anthropic.com` its only possible destination. Both sentences became false
the moment a subprocess provider and an ask route shipped.

Worse, the mechanism that proves the sealed build is clean **cannot see either
of them**. `sealed.rs` works by asserting no networking crates are linked; a
subprocess adds no dependency, so that test passes whether or not the CLI
provider is compiled in. It is a guard that cannot fail — the exact failure
mode this project has hit three times.

## What to build

An auditor can compile and test three configurations, each with a claim that
is true of it and a test that enforces the claim: both features, neither, and
the CLI without the HTTP client. The documents hold to one sentence, and every
claim in them names the test that makes it so.

## Acceptance criteria

- [ ] The sealed build rejects `cli:claude` with the "not available in this
      build" message, and still writes the structural map.
- [ ] A byte probe over the sealed binary finds no `claude` program string —
      and the **same probe finds it present in the default build**, or the
      probe asserts nothing. The existing `cargo tree` control is the
      precedent for this and stays as it is.
- [ ] CI gains a third configuration,
      `--no-default-features --features agent-cli`. This is the posture
      ADR-0008 exists to make expressible — HTTP client absent, approved CLI
      permitted — and it is a claim rather than a guarantee until something
      runs it.
- [ ] `README.md` and `docs/SECURITY.md` hold to story 9's sentence verbatim:
      *CodeAtlas has exactly two ways to reach a model — an HTTPS POST to
      `api.anthropic.com`, and spawning the already-authenticated `claude`
      CLI. Each sits behind its own Cargo feature; each is reachable only from
      `scan --enrich` and `serve --ask`. The sealed build has neither.*
- [ ] `docs/SECURITY.md` §1's list of never-touch-the-network commands
      accounts for `serve --ask` being the exception to `serve`, and §2's
      "All network code is the Claude provider" is corrected — it is the
      sentence ADR-0006 already had to fix for the same reason.
- [ ] `docs/SECURITY.md` states what a model receives on the **question**
      path as well as the enrichment one: a node's id, kind, name,
      repo-relative path and existing summary, for at most
      `ask::CONTEXT_NODES` nodes with each summary capped at
      `ask::MAX_SUMMARY_CHARS` — never file contents. Added 2026-08-11:
      `enrich::ask::NodeContext` carries a comment pointing here, and until
      this lands that pointer names a paragraph that does not exist.
- [ ] Every claim added or changed names the code and the committed test that
      enforces it, which is the standard `docs/SECURITY.md` already holds
      itself to.
- [ ] The netns egress suite still pins the surface: plain `serve` succeeds
      inside a namespace with no route out, `serve --ask` and `--enrich` do
      not.

## Notes

**Why this is blocked by two tickets rather than one.** Story 9's sentence
names `serve --ask`. Writing it after 31 but before 34 would make the
documents false in the other direction — claiming a route that does not exist
yet. Having 31 and 34 each update the docs for their own half was the
alternative, and it was rejected because the one sentence the whole posture
rests on would then be written twice and drift.

**The probes need a live control or they are theatre.** `sealed.rs` already
gets this right: `default_dependency_tree_contains_the_http_client_control`
exists purely to prove the sealed assertion reads real data. The byte probe
needs the same treatment. Note that `cargo test` builds always carry the
`test-provider` feature through the self dev-dependency, so no locally built
test binary is the genuinely sealed artifact — which is why the existing byte
probe lives in `scripts/sealed-probe.sh` and runs in CI. The new probe belongs
beside it, for the same reason.
