# Ticket 32 — three build configurations, and the claims that describe them

**Status:** done
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 9 — a sealed build plus an egress suite, so approving CodeAtlas is
a code review rather than a trust exercise (as amended 2026-08-11)
**Blocks:** none
**Blocked by:** 31 (the CLI provider) **and** 34 (`serve --ask`) — both done
2026-08-11, so this is unblocked

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

- [x] The sealed build rejects `cli:claude` with the "not available in this
      build" message, and still writes the structural map.
- [x] A byte probe over the sealed binary finds no `claude` program string —
      and the **same probe finds it present in the default build**, or the
      probe asserts nothing. The existing `cargo tree` control is the
      precedent for this and stays as it is.
- [x] CI gains a third configuration,
      `--no-default-features --features agent-cli`. This is the posture
      ADR-0008 exists to make expressible — HTTP client absent, approved CLI
      permitted — and it is a claim rather than a guarantee until something
      runs it.
- [x] `README.md` and `docs/SECURITY.md` hold to story 9's sentence verbatim:
      *CodeAtlas has exactly two ways to reach a model — an HTTPS POST to
      `api.anthropic.com`, and spawning the already-authenticated `claude`
      CLI. Each sits behind its own Cargo feature; each is reachable only from
      `scan --enrich` and `serve --ask`. The sealed build has neither.*
- [x] `docs/SECURITY.md` §1's list of never-touch-the-network commands
      accounts for `serve --ask` being the exception to `serve`, and §2's
      "All network code is the Claude provider" is corrected — it is the
      sentence ADR-0006 already had to fix for the same reason.
- [x] `docs/SECURITY.md` states what a model receives on the **question**
      path as well as the enrichment one: a node's id, kind, name,
      repo-relative path and existing summary, for at most
      `ask::CONTEXT_NODES` nodes with each summary capped at
      `ask::MAX_SUMMARY_CHARS` — never file contents. Added 2026-08-11:
      `enrich::ask::NodeContext` carries a comment pointing here, and until
      this lands that pointer names a paragraph that does not exist.
- [x] Every claim added or changed names the code and the committed test that
      enforces it, which is the standard `docs/SECURITY.md` already holds
      itself to.
- [x] The netns egress suite still pins the surface: plain `serve` succeeds
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

## What the work found

**The "not available in this build" message already existed, under different
words, and was left alone.** The criterion quotes ADR-0008's paraphrase; what
a sealed binary actually says is `unknown enrichment provider "cli:claude".
This build recognises none: it was compiled without any enrichment backend
(ADR-0006 sealed build).` That is the same claim, and it is rendered from
`recognised_sentence()`, which ticket 29 made the single source every message
naming a backend goes through. Coining a new literal for this one path would
have put a second wording beside it and broken the property
`every_message_that_lists_backends_renders_the_same_sentence` exists to hold.
The tests assert the existing sentence instead.

**The byte probe is specific, and its control is not.** Measured on release
builds of all three configurations, `claude` appears 0 times sealed, 6 times
default, and 2 times with `agent-cli` and no `network`; `api.anthropic.com`
and `ureq` appear 0 / 1 / 0 and 0 / 25 / 0. So the sealed assertion is sharp.
The control is not: the default binary contains `claude` both as the CLI
backend's program name and inside the API backend's default model
`claude-opus-5`, so it would still pass if `agent-cli` alone were dropped from
the default feature set. That is recorded as a limitation in
`docs/SECURITY.md` rather than papered over, and what actually discriminates
is the pair of configurations — the `agent-cli`-without-`network` build
carries the string and no HTTP client at all.

**Three tampers, because a probe nobody has watched fail is a probe nobody has
tested.** The `claude` byte scan was run with the `agent-cli`-without-`network`
release binary as its sealed subject: that binary passes the `api.anthropic.com`
and `ureq` checks and trips on `claude` alone, so the new needle is doing the
work rather than riding on the old ones. The behavioural refusal was run
against the same binary and tripped, and its control was run with the sealed
binary standing in as the control and tripped. The netns test was tampered
twice — once by dropping `--ask` from the served command, which made the POST
land on the 405 branch, and once by pointing its control at a route that 404s,
which produced *the control failed … so the 502 above proves nothing*. The new
dependency-tree probe was pointed at a tree that does link `ureq` and named
all nine offenders.

**A 502 on its own is worth nothing, so the netns test asserts a pair.**
`POST /api/ask` failing inside the namespace is equally consistent with a
server that never started, a port never bound, or loopback never brought up.
The script therefore asks the same server for `GET /api/map` immediately
afterwards and requires 200. That control doubles as story 14's rule applied
to a route: a backend that cannot answer must not take the server down.

**`enrich_is_the_only_path_that_needs_egress_and_it_degrades_cleanly` was
renamed.** ADR-0009 made its name false — there are two such paths now — and a
false claim in a test name is the kind that survives longest, because nobody
reads a green test. It is
`enrich_needs_egress_and_degrades_cleanly_without_it`, named for what it
asserts rather than for what it used to imply about everything else.

**The third configuration got a tree probe as well as a CI job.** The
criterion asks only that CI run `--no-default-features --features agent-cli`,
and compiling it does prove the feature combination is coherent. But the
posture ADR-0008 named is *HTTP client absent, approved CLI permitted*, and
the absent half was resting on an inference — `agent-cli = []` adds no
dependency, so of course the tree is clean. It is now asserted:
`the_agent_cli_configuration_links_no_networking_crates_either`, beside the
existing sealed assertion and sharing its live control. A future dependency
added under that feature would otherwise have broken the posture silently,
since the sealed test never turns the feature on.

### Two accidental invocations of the reader's real Claude CLI

While measuring refusal messages I twice ran a release binary in a way that
spawned the user's authenticated `claude` CLI, on a one-file scratch
repository, two enrichment slots each. Recorded rather than quietly fixed,
because both were avoidable and the second one is a trap worth naming.

The first was `codeatlas scan --enrich --provider cli:claude` against the
**default** release binary, run to read its error message — except that spec
is not an error there, it is the working backend. The second was a tampered
copy of `sealed-probe.sh` pointed at the **`agent-cli`-without-`network`**
binary, where section 2's plain `scan --enrich` — no `--provider` at all —
also spawned it: a build with exactly one backend defaults to that backend,
which `default_provider`'s own doc comment says plainly and which is right for
a shipped binary, but which means *no* `--enrich` invocation of that build is
inert. Every later tamper was rebuilt to run only `--provider cli:nonsense`,
which is refused at selection and spawns nothing.

The same hazard is now designed out of the committed script. Section 2b names
`cli:claude`, and that is safe only because section 1 runs first and aborts on
any binary containing the program string — a default binary passed by mistake
as the sealed subject never reaches the line that would spawn anything. The
2b control cannot use `cli:claude` at all for that reason, so it asks both
binaries for a `cli:` spec no build accepts and requires the control to refuse
it *by naming* `cli:claude`, which only a build with the backend does.

### Left undone

There is no test asserting that story 9's sentence appears verbatim in
`README.md` and `docs/SECURITY.md`. It was checked mechanically against the
spec while writing them and both match word for word, but nothing in CI will
notice if one drifts. A doc-scanning test would be a new pattern in this
repository — no test reads a Markdown file today except as scan fixture input
— and the honest place for that decision is the ticket that wants it, not this
one.
