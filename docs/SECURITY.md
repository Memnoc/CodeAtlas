# CodeAtlas security posture

This is the audit entry point (spec story 9, [ADR-0006]). Every claim below
names the code and the committed test that enforces it — the posture is
tested, not documented. CI (`.github/workflows/ci.yml`) runs all of it on
every push and pull request.

[ADR-0006]: adr/0006-zero-egress-enforced-by-compile-time-feature-gate.md
[ADR-0008]: adr/0008-enrichment-through-an-authenticated-claude-cli-behind-its-own-feature.md
[ADR-0009]: adr/0009-codebase-questions-are-answered-by-the-serving-binary.md

## The one sentence

> CodeAtlas has exactly two ways to reach a model — an HTTPS POST to
> `api.anthropic.com`, and spawning the already-authenticated `claude` CLI.
> Each sits behind its own Cargo feature; each is reachable only from
> `scan --enrich` and `serve --ask`. The sealed build has neither.

Everything below is that sentence in parts, each part with the test that
holds it. Three build configurations are therefore auditable rather than two
— both features, neither, and the CLI without the HTTP client — and CI
compiles and runs all three.

| Build | HTTP client | `claude` CLI | Egress-capable commands |
| --- | --- | --- | --- |
| default (`network`, `agent-cli`) | yes | yes | `scan --enrich`, `serve --ask` |
| `--no-default-features --features agent-cli` | no | yes | `scan --enrich`, `serve --ask` |
| sealed (`--no-default-features`) | no | no | none |

## The guarantees

### 1. The default path opens no non-loopback sockets

`codeatlas scan`, `diff`, `share`, and `serve` never touch the network. Two
flags are the exceptions, and they are named rather than implied:
`scan --enrich` and `serve --ask`, which are guarantee 2's subject. Plain
`serve` — the flag absent — holds no provider at all and does not route
`POST /api/ask`, so it is the same program it was before [ADR-0009] rather
than a similar one.

**Enforced by** `crates/codeatlas/tests/egress.rs`: each command runs inside
a fresh Linux network namespace (`unshare -r -n`) whose only interface is
loopback, so no route off the host exists — a command that succeeds there has
proven it needs no egress, including short-lived sockets that a
`/proc/net` snapshot would miss.

- `scan_succeeds_with_no_network_beyond_loopback`
- `diff_succeeds_with_no_network_beyond_loopback`
- `share_succeeds_with_no_network_beyond_loopback`
- `serve_binds_loopback_and_answers_with_no_network_beyond_loopback` — also
  asserts the printed URL is `http://127.0.0.1:…` and that `GET /` and
  `GET /api/map` answer correctly over 127.0.0.1 *inside* the namespace

The two counter-tests are what pin the surface, by requiring the other two
paths to fail in the same conditions:

- `enrich_needs_egress_and_degrades_cleanly_without_it` — `--enrich` against
  the real provider inside the namespace must FAIL (no route out), and leave
  an intact structural map
- `serve_ask_needs_egress_and_says_so_without_taking_the_server_down` —
  `POST /api/ask` inside the namespace must answer 502, while the same
  server answers `GET /api/map` 200 immediately afterwards. That second
  request is a live control, not a courtesy: a server that never started
  would produce a 502 just as readily

`serve` additionally binds a hardcoded `Ipv4Addr::LOCALHOST`
(`crates/codeatlas/src/serve.rs`); there is no `--host` flag, so the
listener cannot be pointed at a routable interface.

**CI:** the `rust` job (default features) and both legs of the
`feature-configuration` matrix — sealed, and `agent-cli` without `network` —
run this suite on `ubuntu-latest`. The counter-tests need the API backend, so
they compile only under `network`.

### 2. There are exactly two ways to reach a model, each behind its own feature

**The HTTPS route.** All *HTTP* code is the Claude API provider
(`crates/codeatlas/src/enrich/claude.rs`). The URL is a hardcoded constant
(`API_URL`); no env var, flag, or config can redirect it. The HTTP agent
refuses to follow redirects and ignores proxy environment variables, so the
transport cannot be steered elsewhere either.

**The subprocess route.** The second way to reach a model links no HTTP
client at all: it spawns the reader's already-authenticated `claude` CLI
([ADR-0008], `crates/codeatlas/src/enrich/agent_cli.rs`), so CodeAtlas never
handles a credential. It is the only program a *shipped* binary will run for
this purpose — `agent_cli::PROGRAM`, not configurable there, and a `cli:` spec
naming anything else is refused by name. One spec does run an arbitrary
program, and it is named here rather than left for a reader to find:
`cli-exec:<path>` (`crates/codeatlas/src/enrich.rs`) spawns whatever it is
pointed at, and compiles only under the `test-provider` feature — seam 3's
injection point, so the spawn can be asserted against a stand-in executable
without spending the reader's subscription. `test-provider` is not in the
default feature set and is reachable only through the crate's own
dev-dependency on itself, so no released build carries it.

The child is a completion and not an agent: no tools, no MCP servers, no
hooks, a fresh empty working directory outside the repository, and an
allowlisted environment (`PATH`, `HOME`, `XDG_*`) that deliberately excludes
`ANTHROPIC_API_KEY`.

This is the clause [ADR-0006] had to correct on 2026-08-11. Its Decision once
named the enrichment provider as the only network code, which was a true
sentence about one route and a false one about two.

**Enforced by**

- the `network` Cargo feature gate (`crates/codeatlas/Cargo.toml`): the API
  provider and its HTTP client (`ureq`) compile only with `network`
- the `agent-cli` Cargo feature gate, deliberately separate: a subprocess
  links no HTTP client, so folding it into `network` would make that
  feature's name false and would hide the third configuration
- `the_agent_can_neither_follow_redirects_nor_use_an_env_proxy`
  (unit test in `crates/codeatlas/src/enrich/claude.rs`): pins
  `max_redirects(0)` and `proxy(None)` on the one agent `post` uses
- `the_child_gets_no_tools_no_mcp_servers_and_no_extra_directory`,
  `the_api_key_is_never_handed_to_the_child`,
  `the_scratch_directory_is_private_and_temporary`,
  `a_question_is_locked_down_exactly_as_an_enrichment_call_is`
  (unit tests in `crates/codeatlas/src/enrich/agent_cli.rs`), plus the
  spawned-executable tests in `crates/codeatlas/tests/enrich.rs`, which
  assert the same properties on a real child process rather than on an argv
  the test built itself
- the two netns counter-tests in guarantee 1

#### What a model receives on the enrichment path

Bounded by construction (`crates/codeatlas/src/enrich/prompt.rs`,
`user_message`): node ids/kinds/names/paths and mechanical summaries, layer
directories, flow step names, tour stop topology, the project name — never
file contents, never edges, never member lists. At most
`enrich::BATCH_SIZE` slots per request, so a prompt cannot grow with the
repository.

**Enforced by** `a_summary_slot_carries_exactly_the_documented_fields` and
`the_message_carries_the_project_and_its_slots_and_no_more`
(`crates/codeatlas/src/enrich/prompt.rs`), and
`the_prompt_carries_the_slots_and_nothing_from_the_repository`
(`crates/codeatlas/src/enrich/agent_cli.rs`). Both transports build the same
prompt, so this has one place to be true rather than one per backend.

#### What a model receives on the question path

`serve --ask` answers from a bounded slice of the map alone ([ADR-0009]). Per
node it sends the node's **id, kind, name, repo-relative path, and the
summary the map already holds** — mechanical or enriched — and nothing else.
Never file contents: the selection is a projection of the graph, and the
module that performs it opens no file and takes no repository root.

Around that slice the message carries two further things and no others: the
**project name**, as the enrichment message does, and the **reader's own
question**, which is the point of the request. Both are stated here because
"per node, and nothing else" is a claim about the nodes and would otherwise
read as a claim about the whole message
(`crates/codeatlas/src/enrich/prompt.rs`, `ask_user_message`).

Three limits bound it, all in `crates/codeatlas/src/enrich/ask.rs`:

- **`ask::CONTEXT_NODES`** — the most nodes that may accompany a question.
  `select_context` scores every node and then truncates unconditionally, so a
  question engineered to match the whole repository sends exactly as much as
  one that matches nothing.
- **`ask::MAX_SUMMARY_CHARS`** — the longest summary one of those nodes may
  carry. Without it the slice would be bounded in nodes and unbounded in
  bytes, because a map from another producer may hold any string the schema
  allows.
- **`ask::MAX_QUESTION_CHARS`** — the longest question accepted. Refused
  rather than truncated: a truncated question is a different question,
  answered without saying so.

**Enforced by**, in `crates/codeatlas/src/enrich/ask.rs`:
`a_context_entry_carries_the_documented_fields_and_no_contents` (the field
list above, asserted as a whole value rather than field by field),
`the_context_is_capped_however_large_the_map_is`,
`a_question_crafted_to_match_everything_still_gets_one_slice`,
`one_enormous_summary_cannot_inflate_the_slice`,
`the_provider_never_sees_more_than_the_bound` (at the provider seam, on what
a backend was actually handed), and
`a_blank_or_oversized_question_is_refused_before_any_provider_is_asked`.
`a_question_carries_the_project_the_question_and_its_nodes_and_no_more`
(`crates/codeatlas/src/enrich/prompt.rs`) holds the paragraph above it — the
message *around* the nodes, asserted whole rather than by substring, because
a per-node claim cannot see a third thing added beside them.
`a_question_reaches_a_spawned_cli_locked_down_and_correctly_framed`
(`crates/codeatlas/tests/serve.rs`) carries the same claim end to end, from
an HTTP request to the argv a real child received.

### 3. The sealed build has neither route — reaching a model is a compile error

`cargo build --no-default-features` produces a binary in which egress is not
a forbidden action but an impossible one ([ADR-0006]). Every command works in
that build; `scan --enrich` and `serve --ask` fail with a message that says
the build has no enrichment backend at all, and `--enrich` still writes the
structural map.

The two routes need differently-shaped proofs, and that is the whole
difficulty of this section. A dependency tree can see an HTTP client; it
cannot see a subprocess, because a subprocess adds no dependency. A tree
probe alone would therefore read exactly the same whether or not the CLI
backend were compiled in — a guard that cannot fail.

**The HTTP route is absent — enforced by**

- `crates/codeatlas/tests/sealed.rs` —
  `sealed_dependency_tree_links_no_networking_crates` shells out to
  `cargo tree -e normal --no-default-features --locked --offline` and asserts
  none of ureq, rustls, webpki(-roots), tokio, hyper, reqwest, native-tls, or
  openssl (nor any crate in their families) is linked;
  `the_agent_cli_configuration_links_no_networking_crates_either` asserts the
  same of `--no-default-features --features agent-cli`, which is what makes
  [ADR-0008]'s *HTTP client absent, approved CLI permitted* posture a
  guarantee rather than an inference from the feature adding no dependency;
  `default_dependency_tree_contains_the_http_client_control` proves the
  probe is live by asserting the default tree DOES contain `ureq`.
  The `-e normal` is load-bearing — see "Reproducing the tree probe by hand"
  below before reading anything into a raw `cargo tree`
- `the_claude_provider_does_not_exist_in_sealed_builds`
  (`crates/codeatlas/tests/enrich.rs`, compiled only without `network`):
  the provider cannot even be selected by name in sealed test builds

**The subprocess route is absent — enforced by**

- `scripts/sealed-probe.sh` — byte-scans the genuinely sealed binary for the
  `claude` program string and finds none, with the default binary as a live
  control. Counted with `grep -c -a` on release builds of all four
  configurations:

  | build | `claude` | `api.anthropic.com` | `ureq` |
  | --- | --- | --- | --- |
  | sealed (`--no-default-features`) | 0 | 0 | 0 |
  | `network` only | 4 | 1 | 22 |
  | `agent-cli` only | 2 | 0 | 0 |
  | default (both features) | 6 | 1 | 25 |

  `claude` is **not** an `agent-cli`-only string, and the table is its own
  disproof: 6 is 4 + 2. Under `agent-cli` it is `agent_cli::PROGRAM` and
  `SPEC` plus the help that names them; under `network` it is
  `DEFAULT_MODEL = "claude-opus-5"`
  (`crates/codeatlas/src/enrich/claude.rs`) and the `"claude"` spec literals
  (`crates/codeatlas/src/enrich.rs`). That breadth is right for the sealed
  *subject*, which must contain neither route — but it is useless in the
  *control*, so the control asks for `cli:claude` (`agent_cli::SPEC`), which
  only `agent-cli` emits and which contains `claude`: one check proving the
  scan reads real data and the backend it scans for is present
- the same script's behavioural half: the sealed binary answers
  `--provider cli:claude` with `unknown enrichment provider`, meaning it does
  not know the spec at all rather than declining to run it. Its control is
  indirect on purpose — asking the *default* binary for `cli:claude` would
  spawn the reader's real CLI and spend their subscription, so the control
  asks both binaries for a `cli:` spec no build accepts, and requires the
  default binary to refuse it by naming `cli:claude`
- `the_cli_backend_does_not_exist_without_the_agent_cli_feature`
  (`crates/codeatlas/tests/enrich.rs`, compiled only without `agent-cli`) and
  its live control
  `the_cli_backend_is_selectable_exactly_where_it_is_compiled_in`
  (`crates/codeatlas/src/enrich.rs`), which asserts the same spec DOES
  resolve wherever the feature is on. Without the control the refusal would
  also pass in a build that had merely lost the ability to select anything

`scripts/sealed-probe.sh` also scans for `api.anthropic.com` and `ureq`, and
runs sealed `scan --enrich` and `serve --ask` for their refusal messages. It
lives in CI rather than `cargo test` because every test build carries the
`test-provider` feature via the self dev-dependency — no locally built test
binary is the sealed artifact — and building one inside a test would double
local test time.

### 4. The share artifact makes zero external requests and redacts by allowlist

`codeatlas share` emits one self-contained HTML file. Every property path of
every map-contract type is classified in `FIELD_CLASSIFICATIONS`
(`crates/codeatlas/src/share.rs`) — the one table to review. Unclassified
fields are dropped (deny by default); LLM-derived prose is replaced with
`[redacted]`; unreadable provenance fails closed.

**Enforced by** `crates/codeatlas/tests/share.rs`:

- `every_schema_field_is_classified_and_no_classification_is_stale` — the
  schema-derived exhaustiveness gate: it walks the generated map contract
  schema, so adding a field to the map types without classifying it fails
  `cargo test`
- `redaction_denies_by_default`, `redaction_replaces_llm_prose_and_keeps_mechanical_prose`
- `share_artifact_references_no_external_host` — byte-scan of the artifact
  for external URL shapes
- `share_refuses_a_map_that_does_not_conform_to_the_contract` — the map is
  deserialized into the typed contract before anything ships; a
  non-conforming map (a string where an object belongs, an unknown enum
  value) aborts the share instead of passing through redaction unrecognized
- `dashboard/tests/zero-egress.test.ts` — the dashboard production build
  (the same assets `share` inlines and `serve` embeds) contains no external
  fetch targets

The artifact also self-discloses which fields were redacted and how often.

## CI jobs (`.github/workflows/ci.yml`)

| Job | What it gates |
| --- | --- |
| `rust` | fmt, clippy, build, full test suite with default features — includes the egress netns suite, the redaction exhaustiveness gate, and the dependency-tree probes |
| `feature-configuration`, sealed leg | clippy, build, and the full test suite `--no-default-features`, then `scripts/sealed-probe.sh` against the genuinely sealed binary (dev-dep-free) with the default binary as probe control |
| `feature-configuration`, `agent-cli` leg | clippy, build, and the full test suite `--no-default-features --features agent-cli` — the third configuration, and the one [ADR-0008] exists to make expressible. Untested it is a claim rather than a guarantee, and two configurations could not tell a broken feature combination from an unused one. This leg runs no byte probe; see the limitations below |
| `contract` | map contract drift: regenerates the JSON Schema and TypeScript types and fails on any diff against the committed artifacts |
| `dashboard` | dashboard tests (including the zero-egress scan of the production build) and production build |

## Honest limitations

- **Byte-scan limits.** `scripts/sealed-probe.sh` greps binaries for
  `api.anthropic.com`, `ureq` and `claude`. Absence of a string is weaker
  evidence than the dependency-tree probe (which is why both exist); and
  `ureq` is four bytes of ASCII that could in principle occur by chance in
  unrelated binary data, which would fail the probe spuriously, not silently
  pass it. `claude` is six bytes with the same caveat — and unlike the other
  two it has no dependency-tree probe standing behind it, because a
  subprocess adds no dependency, so for that route the byte scan and the
  behavioural refusal are the whole of the evidence.
- **The `claude` byte scan is broader than the CLI backend.** The default
  binary contains `claude` for several reasons — the CLI backend's program
  name and spec, and the API backend's default model `claude-opus-5` with the
  `"claude"` spec literals beside it, which is `network`'s four occurrences in
  the table above. That breadth is correct for the sealed subject and wrong
  for a control, so the control does not use it: section 0 of
  `scripts/sealed-probe.sh` requires the control binary to contain
  `cli:claude`, which only `agent-cli` emits. Section 2b is the second,
  differently-shaped control on the same question — the control binary must
  refuse `--provider cli:nonsense` *by naming* `cli:claude`, which only a
  build holding the backend can do. Either one fails if `agent-cli` is
  dropped from the default feature set.
- **No CI job byte-scans the `agent-cli`-without-`network` binary.** The
  `agent-cli` leg of the `feature-configuration` matrix runs clippy, build and
  test in that configuration and nothing else; `scripts/sealed-probe.sh` is
  invoked only by the sealed leg, and only ever against the sealed binary
  with the *default* binary as its control — the control half asks for
  `api.anthropic.com` and `ureq`, which the third configuration is defined by
  not having. So the byte-level evidence for that configuration is the
  measurement in the table above, taken by hand and recorded, not a committed
  check. What CI does enforce there is the dependency tree
  (`the_agent_cli_configuration_links_no_networking_crates_either`, which
  runs in every configuration) and the whole test suite.
- **Netns skip conditions.** The egress tests need unprivileged user
  namespaces (`unshare -r -n`). Where that is unavailable — common inside
  containers and sandboxes — they SKIP with an explicit message rather than
  fail, so a local `cargo test` run in such an environment has not exercised
  them. CI on `ubuntu-latest`, where user namespaces work, is the enforcing
  environment; that runner assumption is stated in
  `crates/codeatlas/tests/egress.rs`.
- **Subprocesses resolved via `PATH`.** Two programs are run by name, so a
  hostile executable earlier on the user's `PATH` runs with the user's
  privileges — the standard trust model of any CLI that shells out, noted
  here because both sit on a credential path. `ant auth print-credentials
  --access-token` (`resolve_credentials` in
  `crates/codeatlas/src/enrich/claude.rs`) receives no code and no map data.
  `claude` (`crates/codeatlas/src/enrich/agent_cli.rs`, under `cli:claude`)
  does receive map data — it is the model call — bounded exactly as
  guarantee 2 states, and it is spawned only when that provider is selected.
- **`serve` DoS surface is loopback-local.** Any process on the same host
  may connect to the dashboard port. Accepted connections carry a 10-second
  read timeout (`accepted_connections_carry_a_read_timeout`,
  `crates/codeatlas/src/serve.rs`), so half-open requests cannot park
  threads forever, but a determined local process can still hammer the
  server. This affects availability of the local dashboard only, never
  confidentiality: the server reads just the map and overlay from disk and
  serves embedded assets.
- **Reproducing the tree probe by hand.** `cargo tree` shows
  dev-dependencies by default, and the test-only `jsonschema` crate pulls in
  `reqwest` and `tokio`. So the obvious hand-check —

  ```
  cargo tree -p codeatlas --no-default-features | grep -iE 'reqwest|tokio|rustls'
  ```

  — returns dozens of hits and looks alarming. Those crates are linked into
  *test* binaries only; none is a dependency of the shipped binary. Pass
  `-e normal` to see the shipping tree, which is what
  `sealed_dependency_tree_links_no_networking_crates` asserts against:

  ```
  cargo tree -p codeatlas -e normal --no-default-features   # 0 networking crates
  cargo tree -p codeatlas -e normal                         # ureq + rustls, as expected
  ```

  Two independent checks confirm the shipped artifact rather than the
  resolver's view of it: the sealed binary contains no `api.anthropic.com`
  bytes (`scripts/sealed-probe.sh`), and it dynamically links only `libc`
  and `libgcc_s` (`ldd`).
- **Trust boundary of the probes.** `cargo tree` reads `Cargo.lock`
  (`--locked --offline`, so it can neither rewrite the lockfile nor touch
  the network); a compromised toolchain is out of scope, as for any build.
