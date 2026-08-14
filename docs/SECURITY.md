# CodeAtlas security posture

This is the audit entry point (spec story 9, [ADR-0006]). Every claim below
names the code and the committed test that enforces it — the posture is
tested, not documented. CI (`.github/workflows/ci.yml`) runs all of it on
every push and pull request.

[ADR-0006]: adr/0006-zero-egress-enforced-by-compile-time-feature-gate.md
[ADR-0007]: adr/0007-the-annotation-store-is-a-committed-repository-artifact.md
[ADR-0008]: adr/0008-enrichment-through-an-authenticated-claude-cli-behind-its-own-feature.md
[ADR-0009]: adr/0009-codebase-questions-are-answered-by-the-serving-binary.md
[ADR-0012]: adr/0012-a-conversation-is-client-carried-bounded-input.md

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
`POST /api/ask`. It is not quite the program it was before [ADR-0009],
because it answers one route that decision added: `GET /api/capabilities`
(`serve::CAPABILITIES_ROUTE`), which on a plain `serve` says `{"ask":false}`.
The dashboard is the same embedded bytes either way, so which shape is
serving it has to be said at runtime. That route is the whole of the
difference — no provider, no credential, no egress path, one more loopback
GET whose body is a boolean about this process.

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

The capability route is held by a test of its own, because what it claims is
a fact about the server rather than about egress:
`the_capability_route_states_whether_questions_can_be_asked`
(`crates/codeatlas/tests/serve.rs`) reads it on both shapes and then holds
each answer to the route it describes — a plain `serve` must 405 the question
route it said it does not have, and a `serve --ask` must answer it. A
capability answer nothing checks against reality is the kind of fact that
drifts silently. `the_dashboard_asks_the_routes_this_binary_serves`
(`crates/codeatlas/tests/routes.rs`) pins both route strings across the
language border, so the page cannot come to ask for a route this server does
not serve — which would look to a reader exactly like a server that cannot
answer questions.

`serve` additionally binds a hardcoded `Ipv4Addr::LOCALHOST`
(`crates/codeatlas/src/serve.rs`); there is no `--host` flag, so the
listener cannot be pointed at a routable interface.

#### The served surface, in full

The route list below is the code's own dispatch table — `serve::REGISTRY`
(`crates/codeatlas/src/serve.rs`), the `const` slice `handle` walks — so a
route absent from it is not served at all, and this document can neither
omit a route the server answers nor keep naming one it no longer does:

- `GET /api/map` — the map JSON, read from disk per request
- `GET /api/diff` — the diff overlay, when `codeatlas diff` has written one;
  404 otherwise, which is how the dashboard knows to hide its toggle
- `GET /api/capabilities` — one boolean: whether this process was started
  with `--ask`
- `POST /api/ask` — registered only while `--ask` puts a backend behind it;
  without the flag the route does not exist rather than existing and
  refusing, which is guarantee 1's plain-`serve` claim above

Every other GET is answered from the embedded dashboard assets in process
memory, or 404s — the dashboard itself, every remaining path rather than a
route with one of its own. Any other method is a 405.

**Enforced by** two tests in `crates/codeatlas/tests/routes.rs`, one per
direction of drift.
`every_registered_route_is_named_in_the_security_document` derives the
route set from `serve::REGISTRY` and fails, naming the route and this
document, when one is not named here — so a route can no longer ship
undocumented, which this document did let happen three times during V1.
`every_route_the_security_document_names_is_still_registered` walks the
other way: it scans this whole document for `/api/`-prefixed path tokens
and fails, naming the token, when the registry no longer holds it — a
stale route description is the same false claim as a missing one. The
registry rather than a source scanner for the served surface, deliberately:
a scanner that recognises a spelling convention cannot fail for a route
spelled unexpectedly. (The document side has no such option — a document
can only be scanned — so its scan is the mechanical token rule the second
test documents.)

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

**Enforced by** `a_summary_slot_carries_exactly_the_documented_fields`,
`a_layer_description_slot_carries_exactly_the_documented_fields` and
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

Around that slice the message carries three further things and no others:
the **project name**, as the enrichment message does, the **reader's own
question**, which is the point of the request, and — only when the request
carried one — the **conversation so far**, the previous questions and
answers the client sent back ([ADR-0012]; the client already knew every word
of them). All are stated here because "per node, and nothing else" is a
claim about the nodes and would otherwise read as a claim about the whole
message (`crates/codeatlas/src/enrich/prompt.rs`, `ask_user_message`).

Five limits bound it, all in `crates/codeatlas/src/enrich/ask.rs`:

- **`ask::CONTEXT_NODES`** — the most nodes that may accompany a question.
  `select_context` scores every node and then truncates unconditionally, so a
  question engineered to match the whole repository sends exactly as much as
  one that matches nothing. Carried citations fill seats inside this bound,
  never on top of it — and a citation naming no real node in the map selects
  nothing, so a client cannot smuggle an invented node into the slice.
- **`ask::MAX_SUMMARY_CHARS`** — the longest summary one of those nodes may
  carry. Without it the slice would be bounded in nodes and unbounded in
  bytes, because a map from another producer may hold any string the schema
  allows.
- **`ask::MAX_QUESTION_CHARS`** — the longest question accepted. Refused
  rather than truncated: a truncated question is a different question,
  answered without saying so.
- **`ask::MAX_TURNS`** — the most previous turns a request may carry.
  Clamped oldest-first rather than refused: the history is the dashboard's
  bookkeeping, not the reader's typing ([ADR-0012]).
- **`ask::MAX_TURN_ANSWER_CHARS`** — the longest carried answer one turn may
  bring back, clamped like a summary; a carried question is clamped to
  `MAX_QUESTION_CHARS`. The server holds no conversation state: the turns
  arrive with the request, bounded here, and are forgotten with it.

**Enforced by**, in `crates/codeatlas/src/enrich/ask.rs`:
`a_context_entry_carries_the_documented_fields_and_no_contents` (the field
list above, asserted as a whole value rather than field by field),
`the_context_is_capped_however_large_the_map_is`,
`a_question_crafted_to_match_everything_still_gets_one_slice`,
`citations_alone_cannot_widen_the_slice_past_the_bound`,
`an_invented_carried_citation_selects_nothing`,
`one_enormous_summary_cannot_inflate_the_slice`,
`history_beyond_the_turn_bound_is_dropped_oldest_first`,
`carried_fields_are_clamped_rather_than_refused`,
`the_provider_never_sees_more_than_the_bound` (at the provider seam, on what
a backend was actually handed), and
`a_blank_or_oversized_question_is_refused_before_any_provider_is_asked`.
`a_question_carries_the_project_the_question_and_its_nodes_and_no_more`
(`crates/codeatlas/src/enrich/prompt.rs`) holds the paragraph above it — the
message *around* the nodes, asserted whole rather than by substring, because
a per-node claim cannot see a third thing added beside them — and
`carried_turns_ride_between_the_project_and_the_question_oldest_first`
(same file) holds the carried case's message whole the same way.
`a_question_reaches_a_spawned_cli_locked_down_and_correctly_framed` and
`carried_turns_reach_the_model_clamped_and_in_order`
(`crates/codeatlas/tests/serve.rs`) carry the same claims end to end, from
an HTTP request to the argv a real child received;
`two_conversations_interleaved_on_one_server_never_see_each_other` (same
file) holds the no-conversation-state sentence above on the wire.

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
  | `network` only | 3 | 1 | 23 |
  | `agent-cli` only | 3 | 0 | 0 |
  | default (both features) | 5 | 1 | 24 |

  **Only the sealed row is a claim. The other three rows are a reading, and
  no arithmetic relates them.** `grep -c` counts *lines*, and a "line" in a
  binary is whatever falls between two `0x0a` bytes — a boundary that depends
  on how the linker happened to lay the strings out. Two occurrences that
  share a line in one link land on two in another, so the counts move with
  the toolchain, with the build path, and with code that has nothing to do
  with either route.

  That is not a caution written in the abstract. Re-measured 2026-08-11 on
  ticket 30's crosscheck amendment — same machine, same toolchain, same
  build directory — `agent-cli` went from 2 to 3 and default `ureq` from 25
  to 24, moved by a `#[serde(flatten)]` in the annotation store and nothing
  else. An earlier version of this paragraph read the table for a shape, that
  the default row is the sum of the two above it. It was true of the `claude`
  column at the time and of no other column even then, and one refactor of an
  unrelated struct made it false there too. There is no shape. Do not add one
  back.

  `claude` is **not** an `agent-cli`-only string, and the table is its own
  disproof: the `network`-only build has no CLI backend compiled in at all
  and still contains three lines of it. Under `agent-cli` the string is
  `agent_cli::PROGRAM` and `SPEC` plus the help that names them; under
  `network` it is `DEFAULT_MODEL = "claude-opus-5"` and `SPEC = "claude"`
  (`crates/codeatlas/src/enrich/claude.rs`). That breadth is right for the sealed
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

### 5. What a scan writes into your repository, and which of it is published

A scan writes into `.codeatlas/` under the scanned root and nowhere else — not
into `.git`, not a dotfile at the root, not a lock file beside it. As of
[ADR-0007] it writes a nested `.codeatlas/.gitignore` there too, and that
file makes one artifact **committable by default**: the annotation store,
`.codeatlas/annotations.json`. Everything else CodeAtlas regenerates — the map
and the diff overlay — stays ignored.

This is a disclosure decision and belongs in an audit document, because it
sends LLM-written prose into git without anyone asking for it each time. What
the store holds is exactly what the map's enrichable slots hold: node
summaries, layer names and descriptions, domain-flow names and tour labels.
Two keyings, not one. A node summary is keyed by node id — which embeds a
repo-relative path — and carries a hash of that file's contents, so editing
the file expires the prose. A layer's name or description is keyed by the
layer's own id (the two ride the same membership hash), a domain-flow label
by the flow's, and a tour label by the tour stop's node id; each of those
carries an `inputs_hash` over the *derivation inputs* the label was bought for
(membership, step chains), which is what expires a name for a shape that has
changed. Neither keying holds file contents, for the reason guarantee 2 gives
— the model is never sent any, so there are none for it to paraphrase.

**It is the same policy as section 4's redaction, not the opposite one. The
line is the trust boundary, not the prose.** A share artifact goes to a
recipient chosen at send time who does not hold the source, and whose onward
reading its sender cannot audit, so its prose is redacted. A committed store
reaches only people who already hold the code it describes, so it discloses
nothing they could not read for themselves. Neither rule weakens the other,
and `share` still redacts an enriched map exactly as before.

Two properties keep this from being a surprise:

- **A scan never overwrites the ignore file.** It writes it when it is
  missing, and an edited one stands — including one edited to un-publish the
  store, which is the supported way to keep prose out of git.
- **The store says what produced it**: provider, model and UTC date, so prose
  arriving in a pull request can be read as generated rather than written.

**Enforced by** `crates/codeatlas/tests/publish.rs`, which for the
classification claims asks `git check-ignore` rather than reading the file's
text — those claims are about what git does:

- `a_scan_writes_nothing_outside_the_directory_it_owns` — the "and nowhere
  else" above, which is otherwise a claim about a reading of the code. It
  fingerprints every path under the root except `.codeatlas/` — length,
  content hash and modification time, `.git` included — scans, and
  fingerprints again; a created, removed or rewritten path fails. Its control
  is that the run demonstrably wrote *something*, so an unchanged tree is
  evidence of restraint and not of a scan that never ran
- `the_ignore_file_publishes_the_store_and_ignores_the_regenerated_map` — the
  map, and anything else in the directory, ignored; the store and the ignore
  file, not
- `this_repositorys_own_annotation_store_is_publishable` — the same question
  asked of the real repository root rather than a fixture, because the only
  rule that has ever broken this mechanism was an outer one, and no fixture
  has an outer rule (see the limitation on parent exclusions below)
- `an_edited_ignore_file_is_never_clobbered_and_its_decision_stands` — and the
  edit is what git then obeys, which is the half that matters
- `a_directory_where_the_ignore_file_belongs_neither_fails_nor_is_replaced` —
  "never overwrite what somebody else put there" holds for things that are not
  files, and a scan that met one must not abort
- `a_clone_gets_the_prose_with_no_credential_and_no_provider` — the story end
  to end through a real local clone: what travels is what the ignore file
  allowed, and a plain `scan` with no backend selected re-attaches the prose
- `the_store_records_what_produced_its_prose`, plus
  `the_written_store_records_the_provider_the_model_and_the_date` and
  `a_backend_with_no_model_records_none_rather_than_a_guess`
  (`crates/codeatlas/src/enrich.rs`) at the provider seam

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
  `claude::SPEC` spec constant beside it, which is `network`'s three lines in
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
- **A parent exclusion silently un-publishes the store.** Git will not let a
  nested ignore file re-include anything under a directory an outer
  `.gitignore` excluded outright, so a repository whose own rules say
  `.codeatlas/` gets no annotation store in git no matter what section 5's
  nested file says. Narrowing the outer rule to `**/.codeatlas/*` — this
  repository's own rule, and what the README tells a reader to write —
  restores it: the contents are ignored, the directory is reachable, and the
  `**/` keeps the rule applying below the root, where a scan run from a
  subdirectory leaves a `.codeatlas/` too. The failure is in the safe
  direction — prose stays unpublished rather than being published by surprise
  — and it is documented in the README rather than detected, because
  CodeAtlas does not read the repository's own ignore rules to second-guess
  them. What is guarded is this repository's own case:
  `this_repositorys_own_annotation_store_is_publishable`
  (`crates/codeatlas/tests/publish.rs`) asks `git check-ignore` about the real
  root, so re-tightening the rule fails the suite instead of silently
  un-publishing the store.
- **Netns skip conditions.** The egress tests need unprivileged user
  namespaces (`unshare -r -n`). Where that is unavailable — common inside
  containers and sandboxes — they SKIP with an explicit message rather than
  fail, so a local `cargo test` run in such an environment has not exercised
  them. CI on `ubuntu-latest`, where user namespaces work, is the enforcing
  environment; that runner assumption is stated in
  `crates/codeatlas/tests/egress.rs`.
- **The netns tests prove no TCP egress, not the absence of a DNS channel.**
  `unshare -r -n` removes every route off the host, so a command that
  succeeds inside the namespace has proven it needs no network — but a
  binary that fired resolver queries and shrugged off their failure would
  pass there exactly as a clean one does, because nothing in the suite
  watches the resolver. The complementary guarantee is the sealed
  dependency-tree probe (guarantee 3,
  `sealed_dependency_tree_links_no_networking_crates`): evidence about what
  code exists in the sealed build — no networking crates at all — rather
  than about what one run inside a namespace happened to do.
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
  may connect to the dashboard port, and a determined one can hammer the
  server. Accepted connections carry a 10-second read timeout
  (`accepted_connections_carry_a_read_timeout`,
  `crates/codeatlas/src/serve.rs`), so a connection that goes silent on a
  read errors out rather than blocking on it forever. That timeout is per
  read, and a per-read timeout bounds no request — any number of reads that
  each beat it add up to no limit at all — so the whole request read is
  bounded on its own terms, in total time, line length and line count:
  `REQUEST_DEADLINE`, twenty seconds across the entire read (request line,
  header block and, on the one route that reads one, the body);
  `MAX_HEADER_LINE`, 8 KiB, on any one head line; and `MAX_HEADER_LINES`,
  64, on the count — all in `crates/codeatlas/src/serve.rs`, beside the
  per-read timeout they complete. Three bounds, three separate refusals, so
  the reader of one knows which tripped: the deadline draws a 408 naming
  its twenty seconds; each cap draws a 431 naming its own number. Half-open
  requests cannot park threads forever — the sentence this document once
  claimed, then moved down here to the limitations when it was found false
  (V1 ticket 38), and now claims again with the code and the tests that
  make it true. **Enforced by**, all in `crates/codeatlas/tests/serve.rs`:
  `a_client_that_trickles_header_lines_is_dropped_at_the_request_deadline`,
  which trickles header lines inside the per-read timeout at the real
  binary, requires the 408 within a stated margin of the deadline (measured
  2026-08-14: refused at 20.0 s against the 20-second deadline, the
  trickler's write dead at 21.0 s), and counts kernel state rather than
  green assertions — the child's own `/proc` thread count shows a handler
  thread parked while the trickle runs and released after the refusal, and
  a fresh request answers promptly after the drop;
  `an_over_long_header_line_ends_the_request_instead_of_growing_a_buffer`
  and `a_header_block_of_too_many_lines_is_told_to_stop`, each of which
  also sends its bound megabytes of hostility and requires the server to
  stop reading — observed as the client's own write failing, kernel state
  again — and to keep serving afterwards. A handler thread
  also outlives its own response, by at most `DRAIN_DEADLINE` — one second:
  every response is followed by a half-close and a drain of whatever the
  client is still sending, because closing on unread bytes resets the
  connection and costs the client the response it was owed. That drain is
  bounded three ways, all in `crates/codeatlas/src/serve.rs`:
  `LINGER_TIMEOUT`, 500 ms, on a client that goes quiet mid-send;
  `MAX_DRAIN`, one mebibyte, on how much is read out; and `DRAIN_DEADLINE`
  across the loop as a whole. The last is what makes the other two add up to
  a bound — a per-read timeout bounds no loop, since any number of reads that
  each beat it are still any number of reads, and a client dribbling a byte
  at a time reaches neither of the first two for days.
  `a_client_that_keeps_sending_is_hung_up_on_rather_than_drained_forever`
  (`crates/codeatlas/tests/serve.rs`) holds the real binary to it by
  dribbling at a server that has already answered. So the drain is a second
  finite hold on a thread rather than a second way to keep one. Nothing is
  retained from it: the bytes pass through a stack buffer and are dropped,
  which is what keeps a route that reads no body from allocating one
  (`a_plain_serve_still_ignores_a_body_it_was_never_going_to_read`, which
  asks a plain `serve` for the map behind a declared 200 KB body and requires
  the map to come back at once). That the responses themselves arrive,
  including refusals decided while a body is still in flight, is asserted
  under repetition by `a_refused_method_reaches_the_client_that_asked_for_it`
  and `the_question_routes_refusals_reach_the_client_too` — with the one
  exception two bullets down. This affects availability of the local
  dashboard only, never confidentiality. What a connection can be told is the
  whole of this list:
  the map and the diff overlay, read from disk; the embedded dashboard
  assets, from process memory; and — since [ADR-0009] — one boolean saying
  whether this process was started with `--ask` (`CAPABILITIES_ROUTE`). That
  last is the only thing the server discloses about its own configuration
  rather than about the repository, and it is a fact a local process could
  establish anyway by asking a question and seeing what comes back.
- **Nothing caps how many connections hold threads at once.** `serve` is
  thread-per-connection and no thread cap exists. What the request-read
  bounds in the bullet above changed is each thread's tenure — one
  connection can no longer park one thread forever, only occupy it for the
  bounded request read plus the bounded drain — not how many such threads a
  client that opens connection after connection can hold at once. A
  concurrent-connection cap is a larger change to the hand-rolled shape
  that must be argued on its own; deferred V1 ticket 38 said so when it
  diagnosed the unbounded read, and the V2 spec's Out of Scope carries the
  decision. Reachable only from loopback, by someone already running code
  on the machine, and it affects availability of the local dashboard only,
  never confidentiality.
- **A request body past `MAX_DRAIN` costs its sender the refusal.** The drain
  stops at one mebibyte, so a client that declares and sends more than that
  is still sending when the server gives up reading and closes — which resets
  the connection, the very thing the drain exists to avoid, back again for
  the requests that overrun it. What that client sees is its own write
  failing partway rather than the 413 naming the cap. The response was
  written and flushed before the drain began, so on Linux it is usually still
  in the client's receive buffer to be read afterwards; that is the kernel's
  ordering of a FIN and an RST rather than anything this server promises, and
  a client which treats a failed write as fatal, as `curl` does, never looks.
  Pinned rather than fixed, by
  `a_body_far_past_the_drain_bound_costs_the_client_its_refusal`
  (`crates/codeatlas/tests/serve.rs`), so raising or losing the bound is a
  test failure rather than a discovery. Raising it would only move the line:
  a server that drains whatever it is sent has no bound at all, which is the
  trade the two numbers make.
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
