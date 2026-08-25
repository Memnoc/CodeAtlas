# CodeAtlas V4 agenda — harvested from the V3 ship

> Produced by `/next` on 2026-08-25. Triage decisions in this doc are
> **Memnoc's**, made in the harvest session: the recommended dispositions
> were accepted with one reversal (the local-model provider, unparked by
> name — "would be cool to offer support for Qwen or some of the open
> source models") and one addition (an appetite discussion on the
> long-standing parks, listed under Open questions). This is an agenda for
> `/adr-with-docs`, not a commitment: nothing here is a spec, a glossary
> entry, or a decision until the interview makes it one.
>
> **Timing, Memnoc's call 2026-08-25: V4 waits.** Nobody but Memnoc has
> run the tool; Memnoc is starting to use it properly and collect
> feedback first. This agenda sits ready until the lap is called — and
> whatever that use teaches joins it as user-supplied material.

## Where V3 landed

Shipped 2026-08-18 as `v0.1.0`, a public GitHub Release — run
32114387855 green, nine attested assets (default + sealed × four
targets, plus checksums). The spec's `## Verification` records all 20
stories **pass**: 19 watched by the harden session against release
binaries, story 15 walked by Memnoc on `craftinginterpreters` (754
files, 1,047 nodes) and recorded in the addendum with its full friction
log. The build run tripped once (ticket 08's per-piece provenance claim,
fixed `3022fe6`) and deferred ~35 crosscheck judgement calls, harvested
below. The Compliance Round's gate closed before the tag (`5e9f692`);
its tripwires stand (see Standing items). As of 2026-08-25: zero release
downloads, zero issues — the release exists but has not been announced.

Context the interview should hold (user-supplied, 2026-08-25): Memnoc's
own use is only beginning; there is no monetisation, hosted-service, or
business-deployment intent — CodeAtlas stays a tool people download from
GitHub and run. The compliance tripwires therefore stay dormant.

## V4 candidates

**1. Make open code discoverable** — *the one candidate born from a
human's hands on the tool.*
Source: V3 spec, Verification addendum (story-15 walk), friction item 3:
with plain `serve`, Memnoc looked for open code and nothing hinted it
exists behind a flag — "exactly ADR-0013's absent-not-advertised design,
recorded here as a discoverability datum for `/next` to weigh." The
interview must weigh a hint (in the dashboard, in `serve`'s startup
line, in `--help` prominence) against ADR-0013's deliberate posture that
the route is *absent*, not advertised, when the flag is off. That
tension is the design question; nothing here prejudges it.

**2. An open-source-model provider (Qwen named)** — *unparked by Memnoc
2026-08-25, user-supplied.*
Lineage: parked in the V1-next harvest; re-parked in V2-next with the
note that distribution could reopen it; V3 spec Out of Scope as "parked,
demand-driven". The demand that arrived is Memnoc's own wish, not a
download — recorded as such. ADR-0004's provider trait is the seam that
kept this door open. Four blockers the interview must resolve, each
pinned by a test, an ADR, or a signed record — the same species as open
code's three blockers in the last harvest, none wavable:

- **The pinned two-ways sentence.** README and `docs/SECURITY.md` state,
  verbatim and test-pinned (`crates/codeatlas/tests/routes.rs`, the
  story-9 drift gate): "CodeAtlas has exactly two ways to reach a model —
  an HTTPS POST to `api.anthropic.com`, and spawning the
  already-authenticated `claude` CLI." A third provider falsifies it.
  The sentence, the test, and V1 story 9's intent must be redrafted
  together, knowingly — the gate exists precisely so this cannot happen
  by accident.
- **The feature-gate ceremony.** ADR-0006 (zero egress compile-gated)
  and ADR-0008 (each backend behind its own Cargo feature) set the
  pattern: a new provider gets its own feature, and the sealed build
  must remain compilable to neither.
- **The transparency statement.** The signed R-2 statement (release-notes
  template `## Transparency` + README Design record) says the tool calls
  "Anthropic's Claude … and nothing else in the tool talks to a model."
  That goes false; the statement needs re-issue and the compliance
  record a dated delta — the over-document directive applies.
- **Local versus hosted is the fork with compliance texture.** A truly
  local backend (an on-machine runtime serving Qwen-class weights) keeps
  AI text on the machine — the tripwire family untouched. An
  OpenAI-compatible *hosted* endpoint is a new egress destination and a
  different posture. The interview decides which of these "open-source
  model support" means before anything else about it.

**3. Pin the wire chunk's HTTP reachability** — source: ticket 04's
crosscheck (run receipt 2026-08-16; ranked first in the run summary's
polish list). The share artifact's innocence rests on all route-speaking
code living in a dynamically-imported `wire.ts` chunk the inliner never
embeds — but nothing tests that the chunk is actually *servable* over
HTTP from the embedded dashboard. A future `build.rs` change to "embed
only referenced assets" would break live ask and open code with no red
test. One test closes it.

**4. Guard-hygiene sweep** — one small ticket; sources are the V3
crosscheck receipts (recorded here in full because `.scratch/` is
disposable):

- Ticket 03: the highlight class-contract drift guard fires in one
  direction only; two of its guards lack a recorded tamper-proof.
- Ticket 01: SECURITY.md cites the private `MAX_SOURCE_BYTES` constant
  as if public; the clap help text hardcodes `/api/source` beside the
  `SOURCE_ROUTE` constant; `from_utf8_lossy` on invalid-UTF-8 under-cap
  source is undisclosed (truncation is disclosed, lossiness is not).
- Ticket 09: the release workflow hard-codes the artifact count of 8 in
  two places; the attestation job is not `needs: gates` (disclosed
  in-file); a duplicated setup stanza.

## Parked — real, not next

- **Feeding opened source into ask's slice** — V3 spec Out of Scope;
  ADR-0013 names it a different decision with its own bounds story. The
  natural deepening of open code; parked until use argues for it. The
  readiest to move up if the appetite discussion pulls it.
- **crates.io name** — deferred by ADR-0014 to its own decision; the
  name was checked free 2026-08-16 and is still unregistered. Parks
  *with a tripwire*: revisit before any announcement push, since
  squat-risk grows with attention.
- **Windows target** — ADR-0014: until someone asks. Nobody has.
- **Long-standing parks, whys unaged** (V2/V3 spec Out of Scope and
  V1-next, as cited in `docs/intake/2026-08-16-codeatlas-v2-next.md`):
  concurrent-connection cap; the five parser gaps + C++ scope-tracking
  family; annotation-store reviewer machinery (still no reviewer);
  user-adjustable top-40 constant; enrichment internals bundle; CLI
  wrappers (chat / explain / onboard); Northstar → CodeAtlas producer
  skill; share/redaction hygiene bundle. **These are the subject of
  open question 1** — Memnoc wants them on the interview's table.

## Dropped — with whys

- **Editing source from the dashboard** — open code is a reader; nothing
  upstream asked for writes (V3 spec Out of Scope, reasoning unaged).
- **Source in share artifacts, always-on source route, GPG signing** —
  rejected by ADR-0013/0014; the superseding mechanism in `docs/adr/` is
  the recorded path if circumstances ever change.
- **Walk friction items 1–2** — the artifacts-download hunt was
  pre-release-only friction (real release downloads exist now); the
  `-scan` typo drew an honest usage refusal, which is the tool working.
- **Nit-grade judgement calls** (tickets 02/05/06/07's test-plumbing and
  record-wording items) — each recorded in its receipt or ticket; none
  worth a ticket. The spec's one drafting slip ("recognised" where the
  501 taxonomy means "unrecognised") was corrected 2026-08-25 with a
  dated note in the spec's Further Notes — closed, not carried.

## Standing items — not V4, not forgettable

- **Compliance tripwires** (record: `docs/compliance/codeatlas.md`):
  monetisation, a hosted service, bundled commercial support, business
  deployment, or any feature letting AI text leave the machine each
  force a compliance re-round *before* the fact. Candidate 2's
  local-vs-hosted fork is the first live contact with this rule.
- **README screen-recording head slot** — `.scratch/recording-script.md`
  holds the shot script; Memnoc's to record.
- **`.scratch/codeatlas-v1/-v2/-v3/` deletion** — whenever ready; the
  specs, ADRs, and git history are the durable record (this doc carries
  the receipts-only material forward).
- **Illinois primary-source re-verify** — opportunistic, if ilga.gov
  ever answers; accepted on secondary evidence by disposition 2026-08-17.
- **Northstar's Built-entry gate items** — different repository.

## Open questions — for the interview to grill first

1. **The long-standing parks: appetite check.** Memnoc, 2026-08-25:
   "some of them look really interesting and we should see if we have
   appetite for any of them." The interview should walk the eight
   long-standing parks (list above) one pass, promote or re-park each
   with a why — the same discipline as this doc, at higher resolution.
2. **What does "open-source model support" mean?** Local runtime, hosted
   endpoint, or both — the fork every one of candidate 2's blockers
   hangs on. Qwen is the named example, not a decided scope.
3. **Discoverability versus ADR-0013.** Where may a hint live without
   becoming advertisement of an absent route — and does the answer
   supersede any ADR-0013 wording?
4. **What did use teach?** By interview time Memnoc will have real hours
   on the tool, and possibly first strangers. Their friction log opens
   the session — it outranks everything harvested here, because it will
   be the only material that came from the tool being *used* rather
   than built.

## Hand-off

Fresh session, `/adr-with-docs`, this document as the agenda — **when
Memnoc calls the lap**, not before. V4 walks the same road V1–V3 did;
its spec will be a new file — a spec that shipped is a record, never
edited into a new version.
