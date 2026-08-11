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

## What `/crosscheck` found

**Two of the documents said something false about the very string the ticket
exists to look for.** `scripts/sealed-probe.sh` and `docs/SECURITY.md`
guarantee 3 both claimed that only the `agent-cli` feature emits `claude`,
naming `agent_cli::PROGRAM`, `SPEC` and the help that renders them. The
`network` feature emits it four times on its own — `DEFAULT_MODEL =
"claude-opus-5"` in `src/enrich/claude.rs` and the `"claude"` spec literals in
`src/enrich.rs` — and each document printed the disproof one line below the
claim: 6 in the default build is 4 + 2, not 2. Both now carry the four-way
measurement rather than the three-way one, `grep -c -a` on release builds:

| build | `claude` | `api.anthropic.com` | `ureq` |
| --- | --- | --- | --- |
| sealed (`--no-default-features`) | 0 | 0 | 0 |
| `network` only | 4 | 1 | 22 |
| `agent-cli` only | 2 | 0 | 0 |
| default (both) | 6 | 1 | 25 |

The `network`-only row is the one the three-configuration table could not
show, and it is the whole of the correction. Removing false security claims is
what this ticket is for, so this came first.

**The byte probe's section-0 control passed for a reason that was not the
reason it gave.** It required the control binary to contain `claude` and
reported that as "the CLI program name", which `claude-opus-5` satisfies by
itself; a default build that had lost `agent-cli` entirely would still have
passed it. The control needle is now `cli:claude` — `agent_cli::SPEC`, which
only that feature puts in a binary, and which contains `claude` as a
substring, so one check proves the section-1 scan reads real data *and* proves
the backend it scans for is present. `ureq` joined the same loop, because the
comment above it promised the control covered every string the sealed binary
must lack and it covered two of three. Section 1 keeps the broad `claude`
needle deliberately: the sealed subject must contain neither route, so a
needle matching both is the right one there.

**The recorded limitation named evidence that does not exist.**
`docs/SECURITY.md` said what discriminates a dropped `agent-cli` is the pair
of configurations, the `agent-cli`-without-`network` build carrying the string
and no HTTP client. Nothing byte-scans that binary: its CI leg runs clippy,
build and test and never invokes the probe script, whose control half asks for
`api.anthropic.com` and `ureq` — precisely what that configuration is defined
by not having. What actually catches the drop is section 0's new needle and
section 2b's `cli:nonsense` control, and the limitation now names those and
states plainly that the third configuration's byte-level evidence is a
recorded hand measurement rather than a committed check.

**Guarantee 2 said the spawned program is "not configurable".** True of any
shipped binary and false of the repository, because `cli-exec:<path>` runs
whatever it is pointed at under `test-provider`. That seam is now named in the
document beside the guarantee it qualifies, with the reason it exists and the
reason no released build carries it, rather than left for a reader to find in
`src/enrich.rs` and wonder about.

**`README.md`'s opening sentence made the opt-in the model name.** It promised
no non-loopback socket "unless you ask for a model by name", but a default
build reaches the API from `scan --enrich` with no `--provider` at all —
`default_provider` picks one, which is right for a shipped binary and makes
the sentence wrong. The opt-in is the flag, so the sentence now names the two
flags.

**The question-path paragraph was accurate per node and incomplete as a
message.** It listed id/kind/name/path/summary "and nothing else", which is a
claim about the nodes; the message also carries the project name and the
reader's question. Both are now stated. Because the document holds every claim
to a committed test and none covered the message *around* the nodes,
`a_question_carries_the_project_the_question_and_its_nodes_and_no_more` was
added beside the enrichment path's equivalent in `src/enrich/prompt.rs`. It
asserts the preamble whole rather than by `contains`, since a third section
appearing beside the project and the question is exactly what a substring
check cannot see.

**A CI step name described a script two tickets out of date.** It still said
"no API host, no HTTP client, `--enrich` refuses" for a script that also scans
for `claude`, exercises `cli:claude` and `cli:nonsense`, reads two help
paragraphs and checks `serve --ask`.

### The duplication, and what was left alone

The `sealed` and `agent-cli` jobs were two hand-maintained copies of nine
steps differing in a feature string, which is how copies drift. They are one
`feature-configuration` matrix; `name:` is set from the matrix rather than
left to GitHub's default so each leg keeps the exact check name it had as a
job, and `fail-fast` is off because one configuration failing must not cancel
the other's verdict. The three probe steps are guarded on `matrix.probe`.

`tests/sealed.rs`'s two no-networking assertions shared a filter and an
empty-tree guard by copy; they now share `networking_crates_linked_by` and
differ in the flags they pass and the sentence they fail with. The empty-tree
guard moved into the helper, where it belongs: `cargo tree` returning nothing
satisfies "no offenders" perfectly.

`tests/egress.rs` had two serve scripts whose first twenty lines were the
same. The setup half is `SERVE_PRELUDE` and the two assertion halves stay
verbatim literals — `SERVE_TAIL` and `SERVE_ASK_TAIL` — because the setup is
where nothing discriminates and the tails are where everything does. The
shared prelude also gained the plain test's loopback-address check for the ask
test, which had not had it. The two near-identical Rust wrappers became
`serve_in_netns`.

`scripts/sealed-probe.sh` built its one-file repository three times; it is
`fixture_repo`, which also carries the reason each check needs its own — a map
with every slot filled reports "nothing to enrich" and returns before a
provider is resolved.

### Ten tampers

Every guard touched was watched failing. The probe's section 0 was run with
the `network`-only binary as control (*lacks 'cli:claude' — the byte-probe is
vacuous*, which is the same invocation that used to reach 2b before failing)
and with the `agent-cli`-only binary as control (*lacks
'api.anthropic.com'*). Section 1 was run with the `agent-cli`-only binary as
the sealed subject and tripped on `claude` alone, the other two needles being
absent there. `fixture_repo` was stripped of its `mkdir` and the script died
on the `printf`, which is the proof it is load-bearing rather than decorative.

Both callers of `networking_crates_linked_by` were pointed at the default
tree in turn and each named all nine offenders under its own message. The
egress prelude's loopback check was made to demand `http://10.0.0.1` and
*both* serve tests failed with *serve bound a non-loopback address*, which is
what proves one shared prelude still guards two tests; `SERVE_TAIL`'s first
request was pointed at a route that 404s; the ask test's flags were emptied so
the POST landed on the 405 branch; and its control was pointed at
`/api/nope`, producing *the control failed … so the 502 above proves
nothing*. The new prompt test was tampered twice — a sixth field on a node,
and a `Repo root:` line beside the project — and caught each.

Test counts moved 176 → 177 default and 166 → 167 `agent-cli`, the new prompt
test in both; sealed stays 148, because `prompt` compiles only where a backend
that needs it does.
