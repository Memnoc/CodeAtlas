# 08 — The release documents

**Spec:** `docs/specs/2026-08-16-codeatlas-v3.md` — a fresh session reads
this first, then ADR-0014's consequences.

**What to build:** The words a stranger meets first become true and
download-shaped: the README leads with download-and-run, and the
release-notes template says what needs no key, what needs Claude, and how
this software was built — so every claim survives someone acting on it.

**Blocked by:** None — can start immediately.

**Status:** done

- [x] README Quick start leads with download-and-run for the four targets
      (pointing at the releases page pattern); build-from-source demoted
      to second and still correct
- [x] The release-notes template carries the provenance paragraph — built
      AI-assisted under the Northstar pipeline; annotation-store prose
      self-discloses provider, model and date — and the no-key sentence
      for scan/serve/diff/share
- [x] No new promised numbers anywhere: counts stay measured-or-absent,
      the house rule
- [x] Every claim in the new prose cross-checked against the shipped
      behaviour it describes

**Where the template lives:** `docs/RELEASE_NOTES_TEMPLATE.md` — standing
documents live in `docs/` beside `SECURITY.md`, and ticket 09's workflow
fills its `{{slots}}`; every value the workflow measures is a slot, so no
number lives in the file at all.

**How the no-release-yet honesty was kept:** every README sentence about
releases quantifies over "every release on the releases page" — vacuously
true today (the page exists and is empty), true of each release the
moment ticket 09's workflow cuts one, and enforced by that workflow's own
gates before anything publishes. The download commands carry `<tag>` and
`<target>` placeholders and the GitHub asset-URL *pattern*; no version is
named anywhere, no link targets a release that does not exist.

**The claims cross-check** (each sentence → what backed it, this tree):

- scan/serve/diff/share need no key, no account, no non-loopback socket →
  `crates/codeatlas/tests/egress.rs` (`*_with_no_network_beyond_loopback`
  netns tests); serve binds hardcoded `Ipv4Addr::LOCALHOST`, no `--host`
  (`crates/codeatlas/src/serve.rs`, `src/lib.rs`)
- dashboard compiled into the binary, one file is the whole install →
  `crates/codeatlas/build.rs` (embedded asset table); SECURITY.md's
  "embedded dashboard assets in process memory"
- default port and printed URL `http://127.0.0.1:4173/` →
  `default_value_t = 4173` (`src/lib.rs`);
  `serve_binds_loopback_and_answers_…` asserts the printed URL shape
- `scan .` writes `.codeatlas/knowledge-graph.json` → `scan::OUTPUT_DIR`
  and `save` (`src/scan.rs`)
- exactly two flags reach a model; both need Claude → SECURITY.md
  guarantee 2; `--provider claude` credentials `ANTHROPIC_API_KEY` then
  `ant auth login` profile (`resolve_credentials`,
  `src/enrich/claude.rs`); `cli:claude` spawns the logged-in CLI with the
  API key stripped (`src/enrich/agent_cli.rs`)
- a model never receives file contents → prompt/ask bound tests
  (`a_context_entry_carries_the_documented_fields_and_no_contents`,
  `src/enrich/ask.rs`; the prompt-field tests in `src/enrich/prompt.rs`)
- no provider still yields the complete structural map; enrichment
  relabels, never creates →
  `enrich_needs_egress_and_degrades_cleanly_without_it` (egress.rs) and
  the scan-first ordering in `src/lib.rs`
- sealed build: neither path to a model, every no-key command works,
  `--enrich`/`--ask` refuse naming the build → `tests/sealed.rs`
  dep-tree probes, `scripts/sealed-probe.sh` byte scan + behavioural
  half, refusal string "this build has no enrichment backend at all"
  (`src/enrich.rs`)
- `serve --open-code` widens what the browser is told, never what leaves
  the host; loopback-local → SECURITY.md guarantee 1's open-code
  paragraph; `the_source_route_exists_exactly_when_open_code_was_given`,
  `only_file_nodes_in_the_map_resolve_and_disk_existence_buys_nothing`
  (tests/serve.rs)
- annotation store self-discloses provider, model, UTC date →
  `the_store_records_what_produced_its_prose` (tests/publish.rs),
  `the_written_store_records_the_provider_the_model_and_the_date`
  (src/enrich.rs)
- dashboard badges enriched prose where it renders it; `share` redacts
  it → CONTEXT.md's Provenance wording, deliberately reused verbatim
  (layer names on cards render unbadged, so "wherever" was avoided);
  `redaction_replaces_llm_prose_and_keeps_mechanical_prose`
  (tests/share.rs)
- build-from-source needs Rust edition 2024 and Node 24 →
  `crates/codeatlas/Cargo.toml` (`edition = "2024"`), `ci.yml`
  (`node-version: 24`); same sentence the Development section already
  carried
- built AI-assisted under the Northstar pipeline, decisions human,
  recorded as ADRs → `CLAUDE.md`'s pipeline, `docs/adr/` with
  `proposed-by`/`approved-by` front matter

**The digit grep:** every added line grepped for `[0-9]`. Hits, all
justified: `x86_64`/`aarch64`/`arm64` (target names, ADR-0014's own
enumeration), `SHA-256` (algorithm name), `127.0.0.1:4173` (hardcoded
bind address and default port, verified above, and already in the old
Quick start), `edition 2024`/`Node 24` (verified toolchain facts already
in Development). The template contains zero digits; no count, size, or
version number was written anywhere — the targets are enumerated by
name, never counted, so a fifth target changes a list and falsifies no
number.

**Suites (regression backstop, docs-only change):** full Rust suite,
default configuration — every binary 0 failed (serve's 53 included);
dashboard — 20 files, 307 passed, 0 failed.
