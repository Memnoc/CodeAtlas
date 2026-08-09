# Baseline study: RepoAtlas & Understand Anything

> Research for CodeAtlas — what the two baseline codebase-visualization tools actually do, how their pipelines work, and where the cost lives, studied from primary sources (local checkout and GitHub). Date: 2026-08-07.

## TL;DR

- **The capability set worth keeping**: a typed knowledge graph (`knowledge-graph.json`: 13 node types, 26 weighted edge types, layers, tour) produced from a codebase; an interactive React Flow dashboard; and cheap graph-consumer commands (chat, diff impact, explain, onboard, domain flows) that grep the graph instead of re-analyzing the repo. Share = a redacted single-file HTML artifact (RepoAtlas) or a zero-LLM viewer over a committed graph (upstream).
- **Architecture in one breath**: an agent-orchestrated 7-phase pipeline (scan → batch → per-batch analyze → assemble-review → architecture → tour → review → save) where deterministic scripts (tree-sitter extraction, import resolution, Louvain batching, Python merge/normalize) do the structure and LLM subagents add prose — all handoffs via JSON files in `.ua/intermediate/`.
- **The structure is already deterministic**: file/function/class nodes, imports/calls/contains edges, line ranges, and even domain flows all come from tree-sitter + import resolution scripts. The LLM contributes only summaries, tags, layer names, and the tour.
- **Where the slowness lives**: (a) per-batch file-analyzer agents — 4 + ceil(files/batch) subagent dispatches per enriched run, 5 concurrent; (b) two single-agent passes (architecture, tour) that re-inject the *entire* graph into one prompt, re-run in full even on incremental updates; (c) the orchestrator narrating deterministic phases turn-by-turn — RepoAtlas measured ~25 minutes of narrated run for ~2.3s of actual script work on a 458-file repo; (d) losing the scan inventory costs a quantified ~157k tokens / ~158s re-dispatch.
- **RepoAtlas's fork thesis is the headline finding**: inverting the default to deterministic-only (`--enrich` opt-in) turns a multi-minute, token-billing run into a ~2-second local one that still yields the complete structural map — LLM prose degrades gracefully to mechanical summaries and directory-derived layers.
- A large fraction of both codebases is not the product: 14-17 install platforms, 7 README translations, Figma analysis, wiki/knowledge-base analysis, marketing homepage, theme engines. RepoAtlas already cut most of this and documented why in 9 ADRs — a ready-made scope map for CodeAtlas.
- A second systemic cost is **LLM-output fragility**: both projects spend hundreds of prompt lines and a 54 KB Python merge script policing agent output (ID prefixes, batch filename regexes, envelope unwrapping, 1:1 import-edge counts). A rebuild that generates structure in code and asks the LLM only for prose eliminates most of that machinery.
- **Biggest opportunities for a compact rebuild**: deterministic-first as the only default; a real orchestrator program instead of a 1,100-line prompt; enrichment scoped per-node (provenance) rather than per-run; scoped/summarized inputs for layer+tour passes instead of whole-graph prompts; one platform; keep the single-file share artifact.

## Understand Anything (upstream)

Repo: https://github.com/Egonex-AI/Understand-Anything — MIT, by Yuxiang Lin (Lum1104) / Infinite Universe, Inc. Created 2026-03-15, last pushed 2026-07-30, 763 commits on `main`, ~77.8k stars / 6.5k forks, primary language TypeScript, homepage https://understand-anything.com/ (GitHub API, 2026-08-07). Latest GitHub release v2.9.0 "Figma design graphs + .ua data directory" (2026-07-10); source is ahead at version 2.9.4 in `understand-anything-plugin/.claude-plugin/plugin.json` with no matching release.

### Capabilities

Nine skills under `understand-anything-plugin/skills/` (directory listing via GitHub contents API):

| Command | What it produces / how invoked |
|---|---|
| `/understand` | Main analysis → `.ua/knowledge-graph.json` (nodes/edges/layers/tour), auto-launches dashboard. Flags: `--full`, `--auto-update`, `--no-auto-update`, `--review`, `--language <lang>`, `--exclude <patterns>`, optional path to scope. 858-line SKILL.md. |
| `/understand-dashboard` | Interactive web dashboard. Fast path: `npx` a self-contained viewer tarball from the GitHub release pinned to the plugin version; fallback: pnpm + Vite dev server with `GRAPH_DIR`. Prints tokenized `http://127.0.0.1:<port>?token=<TOKEN>` URL. |
| `/understand-chat` | Q&A over the graph: grep `knowledge-graph.json` for matching nodes, walk 1-hop edges, answer from the subgraph; graph-freshness check against `git diff`. |
| `/understand-diff` | Impact analysis of changes/PR: changed nodes → 1-hop affected components → layers → risk; writes `diff-overlay.json` for the dashboard. |
| `/understand-explain` | Deep-dive on a file/function: node + edges + layer, then reads real source. |
| `/understand-onboard` | Markdown onboarding guide (overview, layers, key concepts, tour, file map, complexity hotspots); offers to save `docs/ONBOARDING.md`. |
| `/understand-domain` | Business-domain graph (`domain-graph.json`: domains → flows → steps); derives from the knowledge graph or does a lightweight scan (`extract-domain-context.py`) + one domain-analyzer agent. |
| `/understand-figma` | Figma design knowledge graph via the Figma REST API (`FIGMA_TOKEN`), `kind:"design"`; `figma-scan.mjs` + `figma-merge.mjs`, batches of ~15 nodes, up to 5 concurrent design-analyzer agents; explicitly "not fully offline". |
| `/understand-knowledge` | Analyzes a "Karpathy-pattern LLM wiki" (`[[wikilinks]]` + index.md): deterministic `parse-knowledge-base.py` + article-analyzer agents (10-15 articles/batch, up to 3 concurrent) extracting entities/claims; `kind:"knowledge"` force-directed graph. |

There is **no share/export skill** in the upstream tree — sharing is committing `.ua/` (README recommends git-lfs for graphs > 10 MB) plus the standalone viewer: `understand-anything-viewer` needs only Node >= 18, "no Claude Code, no LLM, no API key", serves read-only on 127.0.0.1 behind a one-time token (`packages/viewer/README.md`).

**Ten agent definitions** under `understand-anything-plugin/agents/`: `project-scanner`, `file-analyzer`, `assemble-reviewer`, `architecture-analyzer`, `tour-builder`, `graph-reviewer`, `domain-analyzer`, `article-analyzer`, `design-analyzer`, `knowledge-graph-guide`. Frontmatter carries only `name`/`description` — **no model pinning anywhere**; model choice is inherited from the host session (verified in all 10 files).

**Hooks** (`hooks/hooks.json`): a `PostToolUse` hook that regex-matches `git (commit|merge|cherry-pick|rebase)` and, when `autoUpdate: true` in `.ua/config.json`, injects an instruction to incrementally update the graph without asking; plus a `SessionStart` staleness check (meta.json hash ≠ HEAD).

**Other shipped surface**: 7 translated READMEs (`READMEs/`: zh-CN, zh-TW, ja, ko, es, tr, ru); `install.sh`/`install.ps1` supporting **14 installable platforms** (`gemini, codex, opencode, pi, openclaw, antigravity, vibe, vscode, hermes, cline, kimi, trae, nanobot, kiro`, README line 220) with a 17-row platform-compatibility table (README lines 256-274); an Astro marketing homepage (`homepage/`) and a demo mode (`vite.config.demo.ts`); `.claude-plugin`, `.copilot-plugin`, `.cursor-plugin` manifests; a deterministic large-repo benchmark CLI; 22 internal plan + 17 spec markdown files shipped in `docs/superpowers/`.

### Architecture & pipeline

The `/understand` pipeline is 7 phases plus pre-flight, orchestrated by the host agent reading `skills/understand/SKILL.md` (read in full at raw.githubusercontent.com):

| Phase | Kind | What runs | Artifact |
|---|---|---|---|
| 0 Pre-flight | shell | worktree redirect, plugin-root resolution, core build if missing, full/incremental/review-only decision, subdomain merge (`merge-subdomain-graphs.py`) | `config.json` |
| 0.5 Ignore | script + **human gate** | `generate-ignore.mjs`, then waits for user confirmation of the generated ignore file | `.ua/.understandignore` |
| 1 SCAN | **1 LLM agent** wrapping 2 scripts | `scan-project.mjs` + `extract-import-map.mjs` (tree-sitter import resolution, 13 languages); LLM writes only the name/description/frameworks narrative. Gate: >100 files → suggest scoping | `intermediate/scan-result.json` |
| 1.5 BATCH | deterministic | `compute-batches.mjs` — Louvain community detection over the import graph; constants in source: `MAX_COMMUNITY_SIZE=35`, `MIN_BATCH_SIZE=3`, `MAX_MERGE_TARGET=25`, `MAX_NEIGHBORS=50`, `IO_PARALLELISM=64`; count-based fallback | `intermediate/batches.json` (+ per-batch `batchImportData`, cross-batch `neighborMap`) |
| 2 ANALYZE | **N LLM agents, up to 5 concurrent** (README: "20-30 files per batch") | each runs `extract-structure.mjs` (tree-sitter, 10 languages, + config/docs/infra/data parsers) then adds summaries/tags/complexity/semantic edges; output split when nodes > 60 or edges > 120; strict `batch-<i>[-part-<k>].json` naming (merge regex silently drops anything else). Then `merge-batch-graphs.py` (53.8 KB): ID normalization, dedupe, dangling-edge drop, 2-pass `tested_by` canonicalization | `batch-*.json` → `assembled-graph.json` |
| 3 ASSEMBLE REVIEW | **1 LLM agent** | recovers dropped nodes, remaps unknown types/complexity, adds missing cross-batch import edges backed by `$IMPORT_MAP` | `assemble-review.json` |
| 4 ARCHITECTURE | **1 LLM agent** | receives **ALL file-level nodes + ALL edges in one prompt**, plus injected per-language context (24 `languages/` files), framework addenda (10 `frameworks/` files), locale guidance (6 `locales/` files); identifies 3-10 layers; **always re-runs on the full node set, even incremental** | `layers.json` |
| 5 TOUR | **1 LLM agent** | receives all file-level nodes + layers + **ALL edges**; runs a topology script (fan-in/out, entry-point scoring, BFS), designs 5-15 pedagogical steps | `tour.json` |
| 6 REVIEW | deterministic by default | inline Node validation script embedded in SKILL.md (schema, referential integrity, layer coverage, orphans); `--review` → **1 LLM graph-reviewer** (approve/reject, up to 2 internal script retries); at most one automated fix pass, then "save the graph anyway" | `review.json` |
| 7 SAVE | deterministic | `build-fingerprints.mjs` (structural fingerprint baseline, must precede `meta.json` — issue #152), write graph + meta, cleanup preserving `scan-result.json` | `knowledge-graph.json`, `meta.json`, `fingerprints.json` |

Error policy: each failed subagent dispatch retried once, then the phase is skipped and partial results saved (SKILL.md, Error Handling).

**Schema**: `{version, project, nodes[], edges[], layers[], tour[]}`; 13 structural node types + 3 domain (`domain, flow, step`) = 16 in graph-reviewer; 26 structural edge types + 3 domain = 29; fixed per-type weights (`contains` 1.0 … default 0.5); `flow_step` weight encodes step order. Knowledge-wiki graphs add `article, entity, topic, claim, source` node types and `builds_on/contradicts/exemplifies/authored_by/cites` edges (`agents/article-analyzer.md`).

**Dashboard stack** (`packages/dashboard/package.json`): React 19 + Vite 6 + TypeScript 5.7; graph canvas `@xyflow/react` ^12 (React Flow); layouts elkjs ^0.9.3 + `@dagrejs/dagre` ^2 + d3-force ^3; graphology + Louvain communities; Zustand 5; Tailwind 4; react-markdown + remark-gfm; prism-react-renderer (no Monaco). No Express — the graph-serving/token-gate middleware lives in a 15.9 KB `vite.config.ts`, or in the standalone viewer.

### Performance profile

- **Agent count for a full enriched run** (the only mode): 1 scanner + ceil(files/~25) file-analyzers + 1 assemble-reviewer + 1 architecture + 1 tour = **4 + N_batches subagent dispatches** (graph-reviewer only with `--review`). A 250-file repo ≈ 10 batches ≈ 14 dispatches; each retried once on failure.
- **Cost rank**: (1) Phase 2 per-file LLM analysis — every file gets LLM summary/tags/complexity plus 1:1 import-edge enumeration ("MUST equal `batchImportData[filePath].length`. Not 90% of it." — `agents/file-analyzer.md`); (2) Phases 4-5, which each stuff the entire graph into a single prompt and are re-run in full on every incremental update; (3) Phase 1 re-dispatch, self-quantified at "~157k tokens / ~158s per incremental run" if `scan-result.json` is lost (SKILL.md Phase 7 comment).
- The README itself warns: initial `/understand` "can consume a significant number of tokens on large projects. We recommend running it on a token plan / subscription, or using a local model" (README line 130).
- **No unbounded review loop**: default Phase 6 is a free deterministic script; the LLM reviewer is opt-in with one fix attempt. `docs/superpowers/plans/2026-03-27-token-reduction-impl.md` shows token reduction was an explicit redesign, and small-batch pooling (`MIN_BATCH_SIZE=3` → ≤25-file misc batches) exists to "drastically cut orchestration".
- **Remaining redundancy**: assemble-reviewer overlaps the merge script and the Phase 6 validator; auto-update hooks can trigger graph work after every commit; the ignore-file gate and >100-file gate add interactive stalls.
- **Telemetry: none found** — no analytics code in README, SKILL.md files, or hooks; the only outbound calls are the Figma API (`understand-figma`) and the viewer-tarball download. (Note: RepoAtlas's ADR-0009 lists "telemetry" as upstream egress; that specific claim could not be confirmed against the upstream tree at HEAD — it may refer to an earlier or misremembered state. Demo mode and remote fetches were confirmed.)

### Essential vs. incidental

**Essential** — the deterministic extraction spine (`scan-project.mjs`, `extract-import-map.mjs` with 13-language import resolution, `extract-structure.mjs` tree-sitter extraction, Louvain `compute-batches.mjs`, `merge-batch-graphs.py`, `build-fingerprints.mjs`; "Same input → same output, every run" per README); the strict graph schema with fixed edge weights; the capped-parallelism batch analysis; the dashboard + zero-LLM viewer; fingerprint-based incremental updates; the cheap graph-consumer skills (chat/diff/explain/onboard).

**Incidental** — the 17-platform installer matrix with ~40 lines of plugin-root resolution boilerplate duplicated per skill; 7 translated READMEs + the 6-file locale system and language-detection/confirmation flow; `understand-figma` (a different product bolted onto the same dashboard, and the only network egress) and `understand-knowledge` (wiki tooling inside a codebase tool); the Astro marketing homepage and demo mode; 39 internal plan/spec markdown files shipped in the tree; the maintainer-facing benchmark subsystem. Borderline: the 24 language + 10 framework prompt-context files (they inflate one prompt) and the assemble-reviewer pass.

**Not verified**: contributor count; core `src/` file-by-file inventory (recursive tree API exceeded fetch limits; directory listings used instead); `merge-batch-graphs.py` internals (behavior taken from SKILL.md and assemble-reviewer descriptions plus its 53,826-byte size).

## RepoAtlas (fork)

Local checkout: `/home/memnoc/Code/RepoAtlas` (remote `git@github.com:Memnoc/RepoAtlas.git`).

### Fork delta

RepoAtlas is a **hard fork of Understand Anything** — the checkout retains full upstream history: 752 upstream commits (last one `2cda14e`, merge of upstream PR #598, 2026-07-25; principal authors Lum1104/Yuxiang Lin with 516 commits) followed by **55 fork commits** by Memnoc (git log). Upstream continued to 763 commits by 2026-07-30, so the fork point is ~11 commits behind upstream HEAD. ADR-0009 describes the copy as taken "at its 3.0.0"; note the `chore: release 3.0.0` commit (`2fe462a`) is itself a fork-authored rebrand commit, and upstream's own releases stop at v2.9.0/source 2.9.4 — so "3.0.0" is the version the imported tree carried at rebrand time, not an upstream GitHub release. The fork declares divergence complete at its own **v4.0.0** and does not track upstream (`docs/adr/0009-the-fork-is-complete-at-v4.md`).

The delta is recorded decision-by-decision in nine ADRs (`/home/memnoc/Code/RepoAtlas/docs/adr/`), summarized in ADR-0009's own table:

| Area | Upstream | RepoAtlas 4.0.x | ADR |
|---|---|---|---|
| Analysis default | Every run calls the model | **Tree-sitter + import resolution only; `--enrich` opt-in** | 0003 |
| Threat model | Implicit; protect the user | Containment — protect the *repo from the tool*; zero egress enforced by a static test suite | 0002 |
| Node origin | Unrecorded | `provenance: "structural" \| "llm"` per node, surfaced in UI as badge/count/filter | 0004 |
| Sharing | Commit `.ua/` + tokenized viewer | Redaction by allowlist (exhaustiveness-tested) + single-file HTML artifact | 0005 |
| Scope | Codebases, domains, Figma, knowledge bases | Codebases and domains only (Figma ~647 lines + the only network path removed; knowledge removed) | 0006 |
| Platforms | 14-17 installer platforms | Two (Claude Code, opencode); viewer repacked on demand | 0007 |
| Locales | 7 README translations + UI locale system | English only (~1,550 unreachable lines removed); `--language` still drives generated content | 0006 |
| Grammars | Vendored Dart/Swift wasm (4.5 MB) | All grammars from npm via `web-tree-sitter` | 0006 |
| Theming | 5 presets x 8 accents x 3 fonts (~120 combos) | Two Rosé Pine themes; 13 node types → 6 color families | 0008 |
| Audience | Product with ~78k-star installed base | One maintainer's personal tool | 0001 |

Fork-only additions visible in git history: the single-process structural driver `atlas-structural.mjs` (PR #36 — "run the structural path in one process instead of seven turns"), deterministic domain-graph derivation (PR #33), call-graph edge emission (PR #32), `/atlas-share` (PRs #27/#28/#46), markdown-link edges (PR #45), mechanical summaries + provenance (PR #22), dagre auto-layout and Rosé Pine dashboard rework (PRs #21, #24, #37-#42), and security fixes upstream may still lack (credential-file scanning, symlink-following in read endpoints, `c776f18`).

### Capabilities

Eight skills under `repoatlas-plugin/skills/` (each a SKILL.md prompt executed by the host agent):

| Command | What it produces | How |
|---|---|---|
| `/atlas` | `.ua/knowledge-graph.json` (+ `domain-graph.json`, `fingerprints.json`, `meta.json`) | Default: deterministic scripts only, zero egress. `--enrich`: the inherited multi-agent pipeline. Incremental by default; `--full`, `--exclude`, `--language`, `--review`, path scoping (`skills/atlas/SKILL.md:1-35`) |
| `/atlas-dashboard` | Local dashboard (Vite dev server preferred; prebuilt viewer tarball as fallback), tokenized URL | `skills/atlas-dashboard/SKILL.md` |
| `/atlas-chat <q>` | Answers about the codebase | Main-session grep over the graph JSON — no subagents, no full-file reads (`skills/atlas-chat/SKILL.md`) |
| `/atlas-explain <path>` | Deep-dive on one file/function/module | Grep target node + 1-hop edges, then read source (`skills/atlas-explain/SKILL.md`) |
| `/atlas-diff` | Change/impact analysis + `diff-overlay.json` rendered by the dashboard | git diff → grep changed nodes → 1-hop blast radius → risk assessment (`skills/atlas-diff/SKILL.md:55-85`) |
| `/atlas-onboard` | Onboarding guide from layers + tour + graph metadata | Grep-based, main session (`skills/atlas-onboard/SKILL.md`) |
| `/atlas-domain` | `domain-graph.json`: business domains → flows → steps | Derives from existing graph (cheap) or lightweight Python scan + one `domain-analyzer` agent; a mechanical version now also runs on every default `/atlas` (`skills/atlas-domain/SKILL.md`; `skills/atlas/SKILL.md:992-1009`) |
| `/atlas-share` | Single self-contained redacted HTML (~4 MB fixed, opens by double-click; no server, no token; discloses its own redaction in a notice chip) | `build-share-artifact.mjs` + core `redact` allowlist (`skills/atlas-share/SKILL.md`) |

Plus **auto-update hooks** (`repoatlas-plugin/hooks/hooks.json`, inherited from upstream): `PostToolUse` commit detection and `SessionStart` staleness detection trigger `hooks/auto-update-prompt.md`, which uses structural fingerprint diffing to "spend zero LLM tokens when changes are cosmetic".

Dashboard features (`packages/dashboard/src/components/`): React Flow canvas with layer clustering, dagre/ELK/d3-force layouts, Louvain communities (`GraphView.tsx`, 60 KB), domain flow view, prism-based code viewer, node info with provenance badges, Fuse.js fuzzy search, guided tour panel, node-to-node path finder, diff overlay toggle, file explorer, personas, export menu with scope control, freshness/staleness notices, mobile layout, token gate.

### Architecture & pipeline

**Monorepo** (`repoatlas-plugin/`, pnpm workspace; `README.md:80-91`):
- `packages/core` — TS analysis engine: Zod schema (`src/schema.ts`, 21 KB), types, tree-sitter via `web-tree-sitter` WASM + 12 npm grammars, Fuse.js search, persistence, fingerprints, staleness, change classification, redaction, mechanical summaries.
- `packages/dashboard` — React 19 + Vite 6 + Tailwind 4 + Zustand + `@xyflow/react` + dagre/elkjs/d3-force + graphology/Louvain (`package.json`); imports only core's browser-safe subpath exports.
- `packages/viewer` — standalone Node >= 18 read-only server, packed as a release tarball on demand (ADR-0007).
- `skills/` — 8 SKILL.md prompts + ~15 bundled `.mjs`/`.py` scripts under `skills/atlas/`; `agents/` — 8 agent definitions (upstream's 10 minus `design-analyzer` and `article-analyzer`, ADR-0006); `src/` — TS context-builders backing chat/diff/explain/onboard.

**`/atlas`, structural mode (default; ADR-0003)** — one process, zero agents, zero egress. `atlas-structural.mjs` runs the fixed sequence: `scan-project.mjs` → `extract-import-map.mjs` → `build-structural-graph.mjs` (tree-sitter parse of every file: file/function/class nodes, `contains`/`exports`/`imports`/`calls` edges, mechanical summaries like "TypeScript file, 214 lines: 3 functions") → `merge-batch-graphs.py` → deterministic `detectLayers` (directory-derived) → inline validation → save, then `build-domain-graph.mjs` (domains = top-level dirs, flows = call chains from uncalled roots — no model) and the fingerprint baseline (`skills/atlas/SKILL.md:334-386, 992-1044`). Measured on a 458-file repo: **~2.3s of scripts, versus ~25 minutes when the same phases were narrated turn-by-turn by the orchestrating agent** (`skills/atlas/SKILL.md:349-352`). Incremental by default (git diff unioned with working-tree mtimes, so uncommitted edits and non-git projects work), and preserves existing `llm`-provenance nodes rather than flattening them.

**`/atlas --enrich`** — the inherited upstream pipeline, made **resumable per node** (ADR-0004): it selects only files whose nodes are still `provenance: "structural"` and runs them through the same phases as upstream (1 project-scanner; `compute-batches.mjs` Louvain batching with count-based fallback of 12/batch; N file-analyzers up to 5 concurrent with 60-node/120-part output caps; `merge-batch-graphs.py`; 1 assemble-reviewer; 1 architecture-analyzer with 28 language + 10 framework prompt addenda; 1 tour-builder; deterministic Phase 6 validation, LLM graph-reviewer only with `--review`; save). Enriched nodes are stamped `provenance: "llm"`.

**Artifacts/schema**: `{version, project, nodes[], edges[], layers[], tour[]}`, 13 node types, 26 edge types with fixed weights (`skills/atlas/SKILL.md:1099-1138`), typed node IDs (`file:path`, `function:path:name`), Zod-validated on load. All stage handoff is files in `.ua/intermediate/`; enrichment progress is a polled file the dashboard renders as a progress bar (`report-progress.mjs`). For RepoAtlas's own ~460-file repo: `knowledge-graph.json` 996 KB, `domain-graph.json` 536 KB, `fingerprints.json` 532 KB (`/home/memnoc/Code/RepoAtlas/.ua/`).

### Performance profile

- **Structural default is effectively free**: ~2.3s of scripts; residual overhead is the orchestrator reading a 1,138-line SKILL.md and running one command. PR #36 exists precisely because narrating deterministic phases cost a model round-trip per phase (~25 min for 2.3s of work).
- **Enriched mode carries the inherited cost shape**: 4 + ceil(F/batch) agent dispatches (≈ 42-43 for a 458-file repo at ~12/batch fallback; fewer at upstream's 20-30/batch), Phase 2 dominating tokens, Phases 4-5 re-injecting the whole graph per prompt. The scan re-dispatch cost is quantified in the skill itself: ~157k tokens / ~158s (`skills/atlas/SKILL.md:1058-1061`).
- **Fragility taxes persist in enriched mode**: batch filename regex discipline ("anything else is silently dropped"), envelope unwrapping, legacy field renames, ID-prefix policing, 1:1 import-edge self-checks with a deterministic recovery pass (`agents/file-analyzer.md:286-296, 484-495`) — correctness re-imposed by scripts after every model step.
- **Retry policy**: one retry per failed dispatch, then skip with partial results; single-pass fix loop in review (`skills/atlas/SKILL.md:1088-1095`).
- Steady-state cost is proportional to the delta: incremental updates and per-node enrichment resume both narrow work to changed/still-structural files; the auto-update hook classifies cosmetic changes at zero token cost (`hooks/auto-update-prompt.md`).
- Deterministic-path scaling has its own benchmark harness (default concurrency 5, schema-validated JSON+MD reports; `docs/benchmarks/large-monorepo.md`) that deliberately excludes LLM cost.

### Essential vs. incidental

**Essential (delivers the map, fast):**
- Tree-sitter structural extraction + deterministic import/call-graph resolution — the entire node/edge skeleton, in seconds, with real names and line ranges (ADR-0003; `build-structural-graph.mjs`).
- The graph schema (typed IDs, typed weighted edges, layers) + Zod validation — the contract every consumer reads.
- The merge/normalize step (`merge-batch-graphs.py`) — dedup and dangling-edge dropping are needed even in pure structural mode.
- The dashboard graph view + search + node info, and the single-file share artifact — the consumption surfaces.
- Incremental machinery: fingerprints, staleness detection, per-node provenance.
- Directory-derived layers and the mechanical domain projection — coarse but instant; enrichment only relabels them ("a flow is called `main`, not 'Checkout'. The shape is real either way", `skills/atlas/SKILL.md:643-649`).

**Valuable but optional (the opt-in enrichment layer):** LLM summaries/tags, inferred layer names, the tour, domain flow naming. The teaching layer, not the map — ADR-0003: "These degrade; they do not break."

**Incidental / bloat — mostly already cut, with sizes from the ADRs:** Figma (~647 lines + only network egress), knowledge-base analysis, 12 extra installer platforms + consistency test, 7 locales (~1,550 unreachable lines), 4.5 MB vendored grammars, ~120-combo theme engine, per-release viewer repack ritual, demo mode. **Residual fork baggage**: legacy `.understand-anything/` directory support threaded through every skill (flagged as a prune candidate, `CONTEXT.md:38-40`); ~30-line plugin-root resolution boilerplate duplicated per SKILL.md; and the orchestration prompts themselves — 1,138 lines for `/atlas` — much of which is process-hardening prose a compiled orchestrator would not need.

## Implications for CodeAtlas

**Keep (proven core):**
1. **Deterministic-first as the only default.** RepoAtlas demonstrated the full structural map — nodes, edges, layers, domain flows — costs ~2 seconds and zero tokens. CodeAtlas should treat this not as a mode but as *the product*; LLM prose is an optional layer on top.
2. **The graph contract**: typed node IDs, a small closed set of node types, weighted typed edges, layers, per-node provenance. Both projects converged on nearly the same schema; it works and every downstream feature (diff overlay, chat, onboard, share) consumes it cheaply.
3. **Graph-consumer commands as grep, not pipelines.** Chat/diff/explain/onboard cost almost nothing because they read the artifact instead of the repo. This pattern generalizes.
4. **Incremental updates keyed on structural fingerprints** + per-node provenance for resumable enrichment — the difference between a tool you re-run casually and one you dread.
5. **The single-file redacted share artifact** (ADR-0005): allowlist redaction with an exhaustiveness test, no server, no token. It is the best sharing story either project has.

**Drop (documented bloat):** multi-platform installers (one host is enough — RepoAtlas's ADR-0007 reasoning applies doubly to a fresh build), UI translations, Figma/wiki analysis, theme engines, marketing site, demo mode, legacy data-directory compatibility (CodeAtlas has no installed base at all).

**Rethink (the real rebuild opportunities):**
1. **Replace the prompt-orchestrator with a program.** Both projects drive the pipeline by having an LLM read an 850-1,150-line SKILL.md and execute bash blocks — the source of the 25-minutes-for-2.3-seconds pathology, the per-skill plugin-root boilerplate, and most of the defensive prose. A small CLI that runs the deterministic pipeline and *optionally* dispatches enrichment calls inverts the ownership: code orchestrates, the model only writes prose.
2. **Ask the LLM only for what is genuinely non-mechanical** — summaries, layer names, tour narration, domain labels — and have it fill slots in an already-built graph rather than emit nodes/edges JSON. That deletes the 54 KB merge/normalize script's LLM-repair half, the ID-prefix policing, the batch-filename discipline, and the assemble-reviewer agent outright.
3. **Bound the whole-graph prompts.** Architecture and tour passes should receive summarized/sampled topology (the tour-builder already computes fan-in/out mechanically), not "ALL nodes + ALL edges", and should not re-run in full on incremental updates.
4. **Keep enrichment resumable and node-scoped** (provenance filter), so cost is always proportional to the delta.
5. **Containment as a testable property** (ADR-0002): zero egress on the default path, enforced by tests, is both a security posture and the strongest adoption argument for running on private code.

## Sources

**RepoAtlas (local checkout, `/home/memnoc/Code/RepoAtlas`):**
1. `README.md` (fork status, commands, layout, credits)
2. `CONTEXT.md` (naming, vocabulary, ADR index, prune candidates)
3. `docs/adr/0001` … `docs/adr/0009` (all nine fork-shaping ADRs)
4. `repoatlas-plugin/skills/atlas/SKILL.md` (1,138 lines — full pipeline, modes, schema, weights)
5. `repoatlas-plugin/skills/{atlas-chat,atlas-dashboard,atlas-diff,atlas-domain,atlas-explain,atlas-onboard,atlas-share}/SKILL.md`
6. `repoatlas-plugin/agents/{file-analyzer,graph-reviewer,assemble-reviewer,project-scanner,domain-analyzer}.md` (read); `{architecture-analyzer,tour-builder,knowledge-graph-guide}.md` (line counts/roles from SKILL.md dispatch sections)
7. `repoatlas-plugin/skills/atlas/compute-batches.mjs` (batching constants), `atlas-structural.mjs`, `build-domain-graph.mjs` (roles per SKILL.md)
8. `repoatlas-plugin/hooks/hooks.json`, `repoatlas-plugin/hooks/auto-update-prompt.md`
9. `repoatlas-plugin/packages/core/package.json`, `packages/core/src/` listing; `packages/dashboard/package.json`, `src/` + `src/components/` listing; `packages/viewer/`
10. `docs/benchmarks/large-monorepo.md`
11. `.ua/` artifact sizes (`knowledge-graph.json`, `domain-graph.json`, `fingerprints.json`)
12. git history: `git log --oneline`, `git remote -v`, author/commit counts around fork point `2cda14e`/`fd36745`

**Understand Anything (GitHub, fetched 2026-08-07):**
13. https://github.com/Egonex-AI/Understand-Anything and https://api.github.com/repos/Egonex-AI/Understand-Anything (metadata, stars, dates, commit count)
14. https://api.github.com/repos/Egonex-AI/Understand-Anything/releases (v1.2.0 → v2.9.0)
15. https://raw.githubusercontent.com/Egonex-AI/Understand-Anything/main/README.md (platform table, token warning, agent table, sharing guidance)
16. https://raw.githubusercontent.com/Egonex-AI/Understand-Anything/main/understand-anything-plugin/skills/understand/SKILL.md (858 lines, read in full)
17. Skills directory listing: https://api.github.com/repos/Egonex-AI/Understand-Anything/contents/understand-anything-plugin/skills — and the SKILL.md of each of the 9 skills (understand-dashboard, -chat, -diff, -explain, -onboard, -domain, -figma, -knowledge)
18. Agents directory listing: https://api.github.com/repos/Egonex-AI/Understand-Anything/contents/understand-anything-plugin/agents — all 10 agent .md files
19. https://raw.githubusercontent.com/Egonex-AI/Understand-Anything/main/understand-anything-plugin/skills/understand/compute-batches.mjs (batching constants)
20. https://raw.githubusercontent.com/Egonex-AI/Understand-Anything/main/understand-anything-plugin/packages/dashboard/package.json (dashboard stack)
21. https://raw.githubusercontent.com/Egonex-AI/Understand-Anything/main/understand-anything-plugin/packages/viewer/README.md (viewer mechanism)
22. https://raw.githubusercontent.com/Egonex-AI/Understand-Anything/main/understand-anything-plugin/.claude-plugin/plugin.json (source version 2.9.4)
23. https://raw.githubusercontent.com/Egonex-AI/Understand-Anything/main/understand-anything-plugin/hooks/hooks.json

**Unverified items are flagged inline** (upstream telemetry claim in ADR-0009; upstream core `src/` file-by-file inventory; `merge-batch-graphs.py` internals upstream; contributor count).
