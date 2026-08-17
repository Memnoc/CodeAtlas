---
system: CodeAtlas
owner: Memnoc (Matteo Stara)
last-reviewed: 2026-08-17
phase: shipping
verdict: ready
---

# Compliance: CodeAtlas

## Boundary

- Intended purpose: a local developer tool that scans a codebase with
  deterministic tree-sitter parsers, serves an interactive map on loopback
  only, and optionally buys AI prose (enrich) or answers questions (ask)
  through Anthropic's Claude using the operator's own credentials.
  Evidence: `README.md`, `docs/SECURITY.md`, `CONTEXT.md`.
- Users and affected people: software developers running it on their own
  machines; recipients of operator-exported share artifacts. No output
  influences a decision about a person.
- Distribution and jurisdictions: public GitHub source
  (github.com/Memnoc/CodeAtlas, MIT), worldwide by default; first binary
  release (GitHub Releases) imminent but not yet cut — no tags, no
  releases at review time. Maker jurisdiction: `unknown — owner: Memnoc`
  (likely UK; the EU nexus exists regardless via worldwide distribution).
- Builder/operator legal roles: maker distributes free open-source
  software and operates nothing; the operator supplies their own Anthropic
  credentials and runs everything locally. Anthropic is the upstream model
  provider. Whether the with-key build makes the maker an EU AI Act
  "provider" is CQ1 below.
- Upstream providers and models: Anthropic Claude via the operator's API
  key or logged-in CLI; the `fake:` test provider is compile-gated out of
  release builds (`test-provider` feature). Evidence:
  `crates/codeatlas/src/enrich/claude.rs`, `docs/SECURITY.md` guarantee 2.
- Training or fine-tuning: none. No model ships in any artifact.
- Data categories, sources, destinations, retention, deletion: source code
  stays local; ask sends bounded map-derived context, never file contents
  (`crates/codeatlas/src/enrich/ask.rs` bounds, six limits listed in
  `docs/SECURITY.md`); no telemetry, no accounts, no collection by the
  maker; annotation store lives on the operator's disk; ANTHROPIC_API_KEY
  is stripped from spawned children (`crates/codeatlas/src/enrich/agent_cli.rs`).
- Outputs and decisions influenced: code-map visuals (deterministic) and
  AI-generated prose (enrichment summaries, ask answers) badged in the
  dashboard where rendered; the annotation store carries one
  machine-readable `ProducedBy` record naming provider, model and UTC date
  of the last run that wrote it (`crates/codeatlas/src/enrich.rs`); share
  artifacts redact all enriched prose. No decisions about people.
- Behaviour configuration: prompts and bounds live in the repository,
  version-addressed by git; no runtime-mutable AI configuration.
- Release state and relevant dates: audit pinned at commit `cb7239d`
  (2026-08-16). Release workflow (`.github/workflows/release.yml`) present;
  its proving dry run was in flight at review time. `v0.1.0` is gated by
  `/harden` and the fresh-machine walk (ADR-0014).

## Trigger triage

| Trigger | Result | Evidence | Disposition |
|---------|--------|----------|-------------|
| AI inference / upstream AI service | yes (optional, operator-keyed) | ask/enrich via Claude; sealed build compiles egress out | EU AI Act pack |
| AI-assisted authorship of the software itself | yes | README/release-notes provenance paragraph | recorded as disclosure, no pack |
| Personal, sensitive, biometric, children's data | no | no telemetry/accounts; maker processes nothing; `docs/SECURITY.md` | GDPR pack (role check only) |
| Output filtering/scoring/deciding about persons | no | outputs are code maps and code prose | inapplicable |
| Human interaction presented through AI / generated text | yes | ask is labelled Claude; enrichment prose badged; store self-discloses | EU AI Act Art 50 analysis |
| Biometrics, emotion inference, surveillance, social scoring | no | no such capability in repo | inapplicable |
| Regulated product, profession, sector | no | developer tooling | inapplicable |
| Foreseeable material harm / misuse | low | harm review below | recorded below |
| Jurisdictional nexus | yes | worldwide public GitHub distribution | EU/UK/US packs |

## Distribution inventory

| Artifact or service | Revision/version | Public/deployed/supported | Evidence | Coverage |
|---------------------|------------------|---------------------------|----------|----------|
| GitHub source repository (main) | `cb7239d` | public | `git remote -v`; repo public | reviewed |
| Committed annotation store (AI prose, self-disclosing) | in-tree at `cb7239d` | public | `.codeatlas/annotations.json`, store v2 `ProducedBy` record | reviewed |
| Dashboard bundle | embedded via `build.rs` | ships inside binaries | `crates/codeatlas/build.rs` | reviewed |
| GitHub Release v0.1.0 (8 binaries, checksums, attestation) | not yet cut | planned, imminent | `.github/workflows/release.yml`; dry run in flight at review time | unverifiable at audit — re-check at shipping branch |
| Share artifacts | operator-produced | not distributed by maker | share redaction suite, route byte-scan (`crates/codeatlas/tests/share.rs`) | reviewed |
| Packages (crates.io, npm, containers), hosted services, demos | none | none | ADR-0014 defers crates.io, name unregistered | n/a |

## Foreseeable harm review

| Affected party | Capability, failure, or misuse | Reach and reversibility | Safeguard evidence | Disposition |
|----------------|--------------------------------|-------------------------|--------------------|-------------|
| Share recipients / code owners | share artifact leaking source or enriched prose | public once shared; irreversible | redaction suite; registry-wide route byte-scan; source structurally absent (wire chunk never embedded) | no material path identified |
| Local users on shared machines | open code widening loopback disclosure | local-only | route absent without `--open-code` (registry-gated); loopback bind hardcoded | no material path identified |
| Operators | credential exposure | account compromise; reversible by rotation | key never in bundle/logs; stripped from child env; no secret files in VCS | no material path identified |
| Download users | tampered or misattributed release binaries | material until noticed | SHA-256 checksums + GitHub build-provenance attestation in workflow; sealed probe against built artifacts | pending first release — verify at shipping |
| Readers | over-trusting AI prose | low; correctable | badging at render; store provenance; refuse-don't-fabricate ask posture | no material path identified |

## Audit remediation

Owner's standing directive (Memnoc, 2026-08-16): before anything publishes,
return to this record, rectify, and prepare all necessary documentation —
be over-zealous and over-document choices rather than leaving any gap,
guess, or assumption. Every row below must be closed or explicitly
dispositioned by Memnoc before the `v0.1.0` tag is cut.

| Finding | Affected artifact/release | Owner | Required remediation | Evidence to close | Status |
|---------|---------------------------|-------|----------------------|-------------------|--------|
| R-1: release evidence unproven at audit time (P-5) | v0.1.0 | Memnoc | run this skill's shipping branch after the dry run: close P-5 with the green run URL, re-run triage against release facts, append a dated re-verdict | 2026-08-17 shipping entry below + run URL in P-5 | **closed 2026-08-17** |
| R-2: transparency position implicit, not stated | v0.1.0 release notes / README | Memnoc | add a voluntary AI-transparency statement to the release documentation: the Art 50 posture (interaction labelled, machine-readable provenance, redaction-on-export), framed as disclosure of practice, not as a concession of applicability | signed off by Memnoc 2026-08-17 ("R2 is ok"); placed verbatim: `docs/RELEASE_NOTES_TEMPLATE.md` § Transparency and README § Design record; every claim maps to a named test | **closed 2026-08-17** |
| R-3: two research caveats rest on secondary sources | record integrity | Memnoc | re-verify Illinois text against ilga.gov and the Colorado AG filing against the docket when reachable | Colorado: **closed 2026-08-17** — docket-verified, and upgraded: a stipulated court order (X.AI LLC v. Weiser, 1:26-cv-01515, ECF 22 + 24) bars enforcement through the interregnum, quotes in the research note. Illinois: attempt exhausted 2026-08-17 (ILGA network unreachable from here, archives empty — full record in the note); same day, ilga.gov also unreachable from Memnoc's own browser, and a third network path drew ECONNREFUSED. **Dispositioned 2026-08-17 by Memnoc: accepted on secondary evidence with scope-class reasoning** — the statute class (employer duties over AI in employment decisions) cannot plausibly reach a free developer tool or its maker; re-verify opportunistically if ilga.gov becomes reachable | **closed 2026-08-17** (Illinois by disposition, Colorado by docket) |
| R-4: counsel questions have no recorded disposition | v0.1.0 | Memnoc | record an explicit disposition for each of CQ1–CQ5: consult counsel, or accept the documented risk position with rationale — silence is not a disposition | Uncertainty table: all five rows dispositioned 2026-08-17, each an accept-with-rationale, tripwires named on CQ2/CQ3 | **closed 2026-08-17** |
| R-5: decision-trail completeness sweep | record | Memnoc | sweep every compliance-relevant design choice (flag-gated open code, share redaction, provenance record shape, no-telemetry, sealed builds, key handling) for a citation to its ADR/SECURITY.md/test; add any missing citation to this record | the Design-choice citations table below | **closed 2026-08-17** |

### Transparency statement (R-2 — signed off by Memnoc 2026-08-17, placed)

Lives in the release-notes template (§ Transparency, beside "How this
software was built") and the README's Design record. The signed text:

> CodeAtlas's AI is strictly bring-your-own: `--ask` and enrichment call
> Anthropic's Claude with credentials you supply, and nothing else in the
> tool talks to a model — the sealed build cannot even be compiled to.
> Wherever AI-written prose appears it says so: the dashboard badges
> enriched text where it renders it, the annotation store carries a
> machine-readable record naming the provider, the model and the UTC date
> of the last run that wrote it, and `share` removes AI prose from the
> exported file entirely. Interaction with the model is always labelled
> as interaction with the model. This is stated as practice, verified by
> the tests `docs/SECURITY.md` names — not as a reading of where any
> law's lines fall — so a reader never has to guess which words a model
> wrote.

Every claim in the draft maps to shipped behaviour: badging
(`the dashboard badges enriched prose where it renders it`, CONTEXT.md
verbatim), the store record (`ProducedBy`, one record, last writing run —
the 3022fe6 correction's wording), share redaction (ADR-0006 allowlist,
share suite), sealed build (ADR-0006 feature gate), labelling (ask panel).

### Design-choice citations (R-5 sweep, 2026-08-17)

| Design choice | Decision record | Enforcement evidence |
|---------------|-----------------|----------------------|
| Open code is flag-gated, map-allowlisted, loopback-only | ADR-0013 | route-existence + allowlist + drift-gate tests named in `docs/SECURITY.md`; harden walk 2026-08-16 (spec Verification, stories 5–8) |
| Zero egress by default; sealed build makes exfiltration a compile error | ADR-0006 | `tests/egress.rs` (network-namespace suite), `tests/sealed.rs`, `scripts/sealed-probe.sh` run against release artifacts in-workflow |
| Share artifact redacts AI prose and carries no route strings | ADR-0006 (redaction allowlist), ADR-0011 (2 MiB ceiling) | share suite incl. registry-walk byte-scan and JSON-transparent probe; harden byte-scan of a real artifact |
| Two model paths only, operator-credentialed; key stripped from children | ADR-0004 (direct API), ADR-0008 (authenticated CLI) | `resolve_credentials`, `agent_cli.rs` env-strip test; SECURITY.md guarantee 2 |
| Ask is a serve route with bounded, client-carried input | ADR-0009, ADR-0012 | six bounds listed in SECURITY.md, each test-named; clamp + 400 wire tests |
| Annotation store is a committed artifact with one self-disclosing provenance record | ADR-0007; store-level record design (`enrich.rs` `ProducedBy` comment) | `the_store_records_what_produced_its_prose`; store round-trip byte-identical (harden 2026-08-16) |
| Store preserves sections it does not understand (old binaries can't drop purchased prose) | ADR-0014 consequence | flatten-capture unit tests incl. tamper naming the lost section |
| Releases are attested, sealed-beside-default, checksummed | ADR-0014 | `.github/workflows/release.yml`; green dry run 31963531524; attestation digest-verified against downloaded binary (harden) |
| Serve binds loopback only, no `--host` | SECURITY.md guarantee 1 | hardcoded `Ipv4Addr::LOCALHOST`; no flag exists to widen it |
| Map contract unversioned by releases (tag ≠ contract version) | ADR-0014 / spec "the map contract is untouched" | contract drift CI job |

## Legal obligations

| ID | Pack and primary source | Applies because | Required outcome | Owner | Evidence | Status |
|----|-------------------------|-----------------|------------------|-------|----------|--------|
| L-1 | EU AI Act Art 50(1), Reg. 2024/1689 (in force for Art 50 since 2026-08-02; Digital Omnibus Reg. 2026/1744 left Art 50 on schedule) | only if with-key build is an "AI system" the maker "places on the market" (CQ1, CQ2) | people interacting with the AI feature are informed | Memnoc | ask is labelled as asking Claude; dashboard badges; README/release notes disclose — satisfied on the facts even if applicable | ready |
| L-2 | EU AI Act Art 50(2) (same gates) | as L-1, for AI-generated text | machine-readable marking of synthetic content | Memnoc | store carries machine-readable `ProducedBy` (provider/model/UTC date); prose badged at render; share export strips it; whether marking must travel per-output is CQ3 | ready within coverage; CQ3 open |
| L-3 | EU AI Act Art 2(12), Recitals 102–104 | free open-source release | exemption applies outside Art 5 / high-risk / Art 50; Art 5 and Annex III demonstrably do not engage | Memnoc | research note §Pack 1 | ready |
| L-4 | GDPR Art 4(7)–(8); EDPB 07/2020 §76; C-40/17 | maker performs no processing operation | no controller/processor role attaches to distribution | Memnoc | no telemetry/accounts in repo; research note §Pack 2 | ready |
| L-5 | UK: no statute in force (bills.parliament.uk verified 2026-08-16) | maker likely UK-established | none | Memnoc | research note §Pack 3 | ready |
| L-6 | US federal + state scans (CO SB24-205/SB26-189, CA SB 53, SB 942, AB 2013, UT/TX/IL/CT/NY) | worldwide distribution | none attach on the statutes' own scope definitions; AB 2013 edge is CQ4 | Memnoc | research note §Pack 4 | ready |

## Northstar engineering policy

| ID | Trigger | Control | Owner | Evidence | Status |
|----|---------|---------|-------|----------|--------|
| P-1 | remote model service | operator-supplied key; never in bundles/logs; stripped from child env | Memnoc | egress tests, `agent_cli.rs` test | ready |
| P-2 | AI changes runtime prose | provider/model/date recorded; prompts version-addressed in git | Memnoc | store `ProducedBy`; repo history | ready |
| P-3 | humans interact with AI / receive generated text | identity clear at the point that matters | Memnoc | ask labelling, badges, provenance paragraph | ready |
| P-4 | AI failure path | tool degrades to full structural function with no key; never fabricates success | Memnoc | scan-first ordering, egress counter-test, sealed refusal string | ready |
| P-5 | release evidence | checksums + attestation + sealed probe inside workflow | Memnoc | green dry run github.com/Memnoc/CodeAtlas/actions/runs/31963531524 — 8 artifacts built, checksums re-verified on a fresh runner, attestation produced and verified for all 9 subjects, sealed probe green per target, publish leg inert | ready (closed 2026-08-17) |

## Uncertainty and decisions

| Question | Why material | Decision owner | Resolution or due condition | Release consequence |
|----------|--------------|----------------|-----------------------------|---------------------|
| CQ1: is with-key CodeAtlas an "AI system" / the maker its "provider"? (Art 3(1), Rec 97, Art 3(68); C(2025) 924 guidelines silent on BYO-key routing) | gates whether Art 50 attaches at all | Memnoc | **dispositioned 2026-08-17 — Memnoc accepts the documented position, no counsel engaged**: even on the most adverse reading, the attaching obligations (Art 50(1)/(2)) are substantially met by shipped, tested behaviour; revisit if Commission guidance speaks to BYO-key routing | none identified that blocks |
| CQ2: is a free MIT release "placing on the market"? (Art 3(9)–(10), Rec 103, Blue Guide §2.2) | second gate on Art 50 | Memnoc | **dispositioned 2026-08-17 — accepted with named tripwires**: monetisation, a hosted service, bundled commercial support, or business deployment each trigger a compliance re-round BEFORE the fact | as CQ1 |
| CQ3: does store-level machine-readable provenance satisfy Art 50(2) marking, or must marking travel with each output? | the one obligation with substance if gates resolve against | Memnoc | **dispositioned 2026-08-17 — accepted**: the only distributed AI text is the committed store, which self-discloses machine-readably; `share` strips AI prose so none travels unmarked; revisit if any future feature emits AI text into artifacts that leave the machine | none identified that blocks |
| CQ4: does CA AB 2013 ("designs, codes, produces" a GenAI system) reach an API wrapper that never trained? | duty would be factually unsatisfiable by the maker | Memnoc | **dispositioned 2026-08-17 — accepted**: the duty presupposes training that never happened; monitor only | low likelihood on text |
| CQ5: Colorado interregnum + "doing business in this state" for a UK individual | low | Memnoc | **dispositioned 2026-08-17 — accepted**: enforcement stayed by docket-verified court order through the interregnum (ECF 22/24), and the scope definitions never reached the maker | none |
| Research caveat: Illinois text verified only via secondary reproduction (ilga.gov unreachable; re-attempt exhausted 2026-08-17, record in the note). Colorado caveat resolved 2026-08-17: enforcement stay is a docket-verified court order, not merely a reported disclaimer | evidence hygiene | Memnoc | Illinois: re-verify when ilga.gov reachable | none |

## Research

- [2026-08-16 — AI legislation for CodeAtlas distribution (EU AI Act, GDPR, UK, US)](../research/2026-08-16-ai-legislation-for-codeatlas-distribution.md)

## Review history

### 2026-08-17 — shipping

- Boundary changes: none in capability or data flow. Since the audit at
  `cb7239d`: the release workflow's dry run executed green
  (run 31963531524 — 8 binaries, checksums, attestation, sealed probe,
  publish leg proven inert), `/harden` walked all 20 stories (19 pass;
  story 15, the fresh-machine walk, deliberately remains Memnoc's), and
  the audit record itself was committed publicly (`eeead9b`). Attestation
  hash metadata now exists in the public Sigstore log for unpublished dry
  artifacts — disclosed in the workflow header, no new trigger.
- Evidence exercised: shallow triage re-run against release facts — every
  trigger's answer unchanged from 2026-08-16; P-5 closed with the green
  run (checksums re-verified on a fresh runner in-run; locally,
  `sha256sum --check` OK and the sealed binary's digest resolves via the
  GitHub attestations API to this repo, `refs/heads/main`,
  `release.yml`).
- Accepted unverifiable evidence, approver, and rationale: one item —
  the Illinois primary text, accepted by Memnoc on secondary evidence
  with scope-class reasoning after the source refused every network
  path including Memnoc's own browser (2026-08-17); re-verify
  opportunistically.
- Verdict and reason: **ready** — remains the evidence-state verdict
  within recorded coverage. Same-day closure of the remediation gate:
  R-1/R-5 by evidence, R-2 signed and placed (`a042484`), R-3 by docket
  (Colorado) and disposition (Illinois), R-4 by Memnoc's recorded
  accept-with-rationale on all five counsel questions, tripwires named
  on CQ2/CQ3. **All five rows closed; the sole remaining gate on
  `v0.1.0` is the spec's story-15 fresh-machine walk.** Harden's
  Verification section (spec, `e2f45ef`) can cite this dated entry.

### 2026-08-16 — audit

- Boundary changes: first review; boundary established at commit `cb7239d`,
  pre-first-release (no tags, no releases).
- Evidence exercised: full Rust suite green in all three feature
  configurations and dashboard suite green as recorded in
  `.scratch/codeatlas-v3/` tickets 01–08 (same-day, this tree); loopback
  bind, egress posture, share redaction, ask bounds, provenance record and
  sealed gating all held by named committed tests cited in
  `docs/SECURITY.md`. Release-artifact evidence (checksums, attestation,
  sealed probe, smoke runs) is workflow-borne and was in flight at review
  time — documentary until the dry run completes.
- Accepted unverifiable evidence, approver, and rationale: none accepted;
  the release surface is deliberately left `unverifiable` pending the dry
  run and the shipping-branch re-check.
- Verdict and reason: **ready** — no release-blocking issue identified
  within the recorded coverage. This states the evidence position, not
  that harm is impossible and not a legal conclusion. The two EU threshold
  questions (CQ1, CQ2) and the marking-standard question (CQ3) are named
  for counsel, but on every reading examined the substantive transparency
  obligations are already substantially met by shipped behaviour
  (labelling, badging, machine-readable provenance, redaction-on-export).
  Condition attached: run this skill's **shipping branch** before cutting
  `v0.1.0` — it must confirm the dry run went green (P-5 closes) and that
  release facts introduced no new trigger.
