# Ticket 31 — enrich without ever handling a credential

**Status:** done
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

- [x] A new `agent-cli` Cargo feature, in the default set and **separate from
      `network`**. A subprocess contains no HTTP client, and filing it under
      `network` would make that feature's name false. All three
      configurations build, test and lint clean.
- [x] `--provider cli:claude` fills the same typed slots as the API provider,
      through the same provider trait, from a schema-constrained completion.
      Both backends now render the prompt and the schema from one shared
      module — see Notes.
- [x] The child process is a completion, not an agent: no tools, no MCP
      servers, and a working directory outside the repository — plus
      `--safe-mode`, which the criterion did not ask for and should have.
      Every one is the CLI's own documented mechanism rather than an
      approximation; see Notes.
- [x] The child's environment is an explicit allowlist, and
      `ANTHROPIC_API_KEY` is **not** in it — `cli:` must unambiguously mean
      the CLI's own credential rather than silently billing the API through a
      subprocess. Asserted against a real spawn, with the key set in the
      parent.
- [x] Every failure mode leaves a complete, schema-valid structural map
      (story 14): the program is not installed, it is installed but not
      logged in, it exits non-zero, or its output does not parse. Six shapes
      of disappointment at the unit level, four end to end.
- [x] `cli:` with any program other than `claude` is rejected, by name and
      with the reason, rather than falling into the generic unknown-spec
      message.
- [x] Tested at **seam 3** against a fake executable that echoes canned JSON:
      assertions cover the argv it was invoked with, the environment it did
      and did not receive, the working directory, and what the provider made
      of what came back.
- [x] The fake-executable injection point compiles only under
      `test-provider`, exactly as the `fake:` and `fail` backends already do,
      so no shipped binary gains a way to run an arbitrary program.
- [x] No test spawns the real `claude`. No test performs network I/O.

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

> Both of those held up. The flag survey did not: it missed `--tools` and
> `--safe-mode`, the two flags the lockdown actually needed, and the reading
> of `--disallowed-tools` missed that it is variadic and would have eaten the
> prompt. A help page skimmed for the flags you expect to find is not the same
> as a help page read.

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

## What the work found

**The response envelope was confirmed without spending anything.** The ticket
asked for it to be checked rather than guessed, and the obvious way — one live
`claude -p` call — spends the user's subscription. The CLI ships as a single
ELF with its JavaScript bundle inside, so `strings` answered it for free:
a completed run prints `{"type":"result","subtype":"success","is_error":false,
"structured_output":{…},…}`, and the bundle's own reader is
`h?.subtype==="success" ? h.structured_output : void 0`. The parser requires
all three of those fields to agree before it will believe an answer.

**The lockdown was rebuilt after `/crosscheck` found the right flags.** The
first attempt reasoned that no deny-everything rule existed — permission rules
are `Tool(content)` with `*` as a *content* wildcard — and fell back to an
enumerated `--disallowed-tools` list with the empty working directory as the
real guarantee. Two flags in the CLI's own help make that unnecessary and were
missed:

- **`--tools=`** — documented as *"Use `""` to disable all tools"*. This is
  the guarantee, and an enumerated deny-list is strictly worse: it silently
  stops covering a tool the day a new one is added.
- **`--safe-mode`** — disables CLAUDE.md, skills, plugins, **hooks**, MCP
  servers and custom agents, while stating that authentication is unaffected.
  Hooks are the one that matters: they run shell commands, and `HOME` is on
  the environment allowlist, so without this the reader's own hooks would
  fire on every enrichment call. That was a real hole, not a tidiness point.

**`tempfile` stayed a dev-dependency.** The obvious way to get a scratch
directory would add a crate to the shipped tree, and `agent-cli` must not
widen the dependency surface a security review reads (ADR-0006) — the sealed
build's whole argument is that the tree is short enough to check. A 30-line
`ScratchDir` claims a name with `create_dir`, which fails rather than
succeeds on an existing path, so it is race-free rather than
check-then-create.

**The prompt moved out of the API backend.** `SYSTEM_PROMPT`, the slot
payloads, the answers schema and the answer parsing are now
`enrich::prompt`, shared by both backends. `docs/SECURITY.md` states exactly
what a model receives; two copies of that would be two places for it to drift,
and the CLI backend would have been the copy nobody re-read.

**The third build configuration earned its place on its first run.** Ticket
29's test asserted that the offered provider list named `claude` exactly when
the API backend was compiled in — true while `claude` was the only spec
containing that word, and wrong the moment `cli:claude` existed.
`--no-default-features --features agent-cli` failed immediately. The
assertion now parses the rendered list back out and compares it to
`recognised_specs()` exactly.

**Twelve mutations, all killed** — the API key back on the allowlist, the
environment not cleared, the child run in the repository, `--add-dir` added,
tools un-denied, MCP config not strict, a non-zero exit accepted, an error
envelope accepted, a non-result message accepted, stderr transcribed whole,
`cli:` accepting any program, and the model pinned like the API backend's.

Three of those survived the first pass and needed better tests. Two were
redundant *for the inputs chosen*: every error-envelope case also happened to
lack `structured_output`, so removing the envelope checks changed only which
error was raised. They are now tested with envelopes that carry a usable
answer alongside the thing that makes them untrustworthy. The third —
pinning a default model — was never tested at all, because the assertion
looked at `build_args` while the mutation lived in `CliProvider::new`.

Six more went in after `/crosscheck`: tools re-enabled, safe-mode dropped, the
`--` fence removed, flags returned to space-separated form, the scratch
directory made world-readable, and the scratch directory never removed. The
last two needed a test written for them; the first four were caught by tests
added in the same pass.

### The defect that mattered most: the prompt would never have arrived

`/crosscheck` read the CLI's bundled argument parser and found that several of
its options — including `--disallowed-tools` and `--mcp-config` — are
**variadic**. A variadic option in space-separated form consumes following
arguments until the next option, so the prompt, passed as a trailing
positional, would have been swallowed as one more denied tool. The model would
have been asked nothing, and this backend would have had no way to tell.

**No fake-executable test could ever have caught it**: a stand-in shell script
has no argument parser to be confused. What is checkable is the shape, so
every flag is now `--flag=value`, `--` fences the prompt off, and two tests
assert that shape — one on the constructed argv, one on the argv a real spawn
actually received.

### Everything else `/crosscheck` found, and what changed

**Three messages lied in the `agent-cli`-without-`network` build** — the exact
fault ticket 29 forbade and this ticket's own notes warned against, reproduced
anyway. `default_provider` said the build had been compiled without the
`network` feature and refused to enrich, when `cli:claude` was present and
working; `model_help` said `--model` had nothing to modify while the CLI
backend honoured it; and the `cli:` refusal produced a garbled sentence. All
three now derive from what is compiled in. `default_provider` also gained the
missing fourth arm: a binary with exactly one backend defaults to it.

**The sealed binary carried `cli:claude` in its bytes**, which would have
failed ticket 32's byte probe before that ticket was even written. Two causes:
the `cli:` refusal arm was ungated, and `MODEL_AWARE_SPECS` was an ungated
`const`. Both are feature-gated now, and a genuinely sealed build was
rebuilt and grepped to confirm — with the default build as a live control.

**`scripts/sealed-probe.sh` broke and was repaired here rather than left for
ticket 32.** It asserted the refusal message contained "compiled without" and
"network", which the corrected message no longer says — and should not, since
since ADR-0008 there are two features a build can lack. It now asserts the
message names no enrichment backend and cites ADR-0006.

**The scratch directory was world-readable.** `fs::create_dir` yields 0755 in
a world-writable system temp directory. Now 0700 via `DirBuilder::mode`, with
a test for the mode and for removal on drop.

Also fixed: `first_line` truncated by lines but not by characters, so a single
unbroken 10,000-character diagnostic would still have been transcribed whole;
the duplicated "did not match the requested schema" wording became
`prompt::parse_answers`, shared by both backends; and `prompt.rs` gained the
bounded-prompt tests, which had stayed behind in the network-gated module and
so did not run in an `agent-cli`-only build.

**Not done here, and deliberately:** `README.md`, `docs/SECURITY.md` and CI's
third configuration. Those are ticket 32, which is blocked on this ticket and
34 precisely so the one sentence describing the egress surface is written
once. `docs/SECURITY.md` §2 is currently false in a default build — it says
`--enrich`'s only possible destination is `api.anthropic.com` — and ticket 32
is where that is corrected.
