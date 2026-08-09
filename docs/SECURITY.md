# CodeAtlas security posture

This is the audit entry point (spec story 9, [ADR-0006]). Every claim below
names the code and the committed test that enforces it — the posture is
tested, not documented. CI (`.github/workflows/ci.yml`) runs all of it on
every push and pull request.

[ADR-0006]: adr/0006-zero-egress-enforced-by-compile-time-feature-gate.md

## The guarantees

### 1. The default path opens no non-loopback sockets

`codeatlas scan`, `diff`, `share`, and `serve` — everything except
`scan --enrich` — never touch the network.

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
- `enrich_is_the_only_path_that_needs_egress_and_it_degrades_cleanly` — the
  counter-test: `--enrich` against the real provider inside the namespace
  must FAIL (no route out), pinning the egress surface to exactly `--enrich`

`serve` additionally binds a hardcoded `Ipv4Addr::LOCALHOST`
(`crates/codeatlas/src/serve.rs`); there is no `--host` flag, so the
listener cannot be pointed at a routable interface.

**CI:** the `rust` job (default features) and the `sealed` job
(`--no-default-features`) both run this suite on `ubuntu-latest`.

### 2. `--enrich` is the only egress-capable command, and its only possible destination is `api.anthropic.com`

All network code is the Claude provider
(`crates/codeatlas/src/enrich/claude.rs`), reachable only via
`scan --enrich`. The URL is a hardcoded constant (`API_URL`); no env var,
flag, or config can redirect it. The HTTP agent refuses to follow redirects
and ignores proxy environment variables, so the transport cannot be steered
elsewhere either.

**Enforced by**

- the `network` Cargo feature gate (`crates/codeatlas/Cargo.toml`): the
  provider and its HTTP client (`ureq`) compile only with `network`
- `the_agent_can_neither_follow_redirects_nor_use_an_env_proxy`
  (unit test in `crates/codeatlas/src/enrich/claude.rs`): pins
  `max_redirects(0)` and `proxy(None)` on the one agent `post` uses
- the netns counter-test in guarantee 1

What Claude receives is bounded by construction (`build_request_body`):
node ids/kinds/names/paths and mechanical summaries, layer directories,
flow step names, tour stop topology, the project name — never file
contents, never edges, never member lists. Only Claude ever sees anything,
and only under `--enrich`.

### 3. The sealed build contains no networking code — sending data is a compile error

`cargo build --no-default-features` produces a binary in which egress is not
a forbidden action but an impossible one (ADR-0006). Every command works in
that build; `scan --enrich` fails with a clear "compiled without the
`network` feature" message and still writes the structural map.

**Enforced by**

- `crates/codeatlas/tests/sealed.rs` —
  `sealed_dependency_tree_links_no_networking_crates` shells out to
  `cargo tree -e normal --no-default-features --locked --offline` and asserts
  none of ureq, rustls, webpki(-roots), tokio, hyper, reqwest, native-tls, or
  openssl (nor any crate in their families) is linked;
  `default_dependency_tree_contains_the_http_client_control` proves the
  probe is live by asserting the default tree DOES contain `ureq`.
  The `-e normal` is load-bearing — see "Reproducing the tree probe by hand"
  below before reading anything into a raw `cargo tree`
- `scripts/sealed-probe.sh` (CI `sealed` job) — byte-scans the genuinely
  sealed binary for `api.anthropic.com` and `ureq` (absent), with the
  default binary as a live control (present), and runs sealed
  `scan --enrich` asserting the refusal message and the surviving map.
  This probe lives in CI rather than `cargo test` because every test build
  carries the `test-provider` feature via the self dev-dependency — no
  locally built test binary is the sealed artifact — and building one
  inside a test would double local test time
- `the_claude_provider_does_not_exist_in_sealed_builds`
  (`crates/codeatlas/tests/enrich.rs`, compiled only without `network`):
  the provider cannot even be selected by name in sealed test builds

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
| `sealed` | clippy, build, and the full test suite `--no-default-features`, then `scripts/sealed-probe.sh` against the genuinely sealed binary (dev-dep-free) with the default binary as probe control |
| `contract` | map contract drift: regenerates the JSON Schema and TypeScript types and fails on any diff against the committed artifacts |
| `dashboard` | dashboard tests (including the zero-egress scan of the production build) and production build |

## Honest limitations

- **Byte-scan limits.** `scripts/sealed-probe.sh` greps binaries for
  `api.anthropic.com` and `ureq`. Absence of a string is weaker evidence
  than the dependency-tree probe (which is why both exist); and `ureq` is
  four bytes of ASCII that could in principle occur by chance in unrelated
  binary data, which would fail the probe spuriously, not silently pass it.
- **Netns skip conditions.** The egress tests need unprivileged user
  namespaces (`unshare -r -n`). Where that is unavailable — common inside
  containers and sandboxes — they SKIP with an explicit message rather than
  fail, so a local `cargo test` run in such an environment has not exercised
  them. CI on `ubuntu-latest`, where user namespaces work, is the enforcing
  environment; that runner assumption is stated in
  `crates/codeatlas/tests/egress.rs`.
- **`ant` subprocess.** OAuth credential resolution runs
  `ant auth print-credentials --access-token` resolved via `PATH`
  (`resolve_credentials` in `crates/codeatlas/src/enrich/claude.rs`). A
  hostile executable named `ant` earlier on the user's `PATH` runs with the
  user's privileges — the standard trust model of any CLI that shells out,
  noted because this is the credential path. It receives no code or map
  data.
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
