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

## Post-harvest field feedback — 2026-08-28

The first external user ran CodeAtlas (relayed by Memnoc, 2026-08-28) —
the first hands on the tool that are not its author's. Two items, both
`user-supplied`:

- **macOS blocked the binary at first run.** Gatekeeper quarantines a
  browser-downloaded unsigned binary; the user needed multiple steps to
  allow it, and allowlisted on personal trust rather than verification —
  which is precisely the reader the attestation exists for. The
  documentation half is **done** (2026-08-28: README Quick start and the
  release-notes template carry the verify-then-dequarantine path). The
  V4 candidate that remains: **Apple Developer ID signing + notarization
  in the release workflow** — a decision, not a ticket: a paid developer
  account, credential management in CI, and it reopens territory
  ADR-0014 settled for GPG on different facts (GPG was rejected for lack
  of a verifier audience; Gatekeeper friction blocks every macOS
  browser-downloader at first contact).
- **`scan` is silent while it works.** Its only output was the final
  `mapped N files` line — one `eprintln!` at the end of the run — so on
  a larger repository the user could not tell whether anything was
  happening. **Fixed ahead of the lap, 2026-08-28, at Memnoc's
  direction**: scan now draws a live `scanning: n/N files` line, each
  count measured at the moment it prints, exactly when stderr is a
  terminal — through a pipe (every script, CI leg, and
  `scripts/release-smoke.sh`'s exact-line assertion) output is
  byte-identical to before. Guarded in both directions and each guard
  proven able to fail: a binary-seam test that no progress leaks into a
  pipe, and a wiring test that a real scan ticks once per file. What
  remains for the interview is only whether other long-running commands
  deserve the same line.
  *Follow-up from Memnoc's own macOS walk, 2026-08-28:* sub-second
  scans erased the counter before an eye could catch it — scan clears
  a few hundred files in well under a second, so the line as first
  shipped was invisible on every ordinary repository. At Memnoc's
  direction the final frame now **stands** instead of clearing:
  `scanning: N/N files` survives above the summary, proof of life at
  any speed. Both new guards tamper-proven (a restored clear tripped
  the standing-frame test; an unconditional newline tripped the
  empty-walk test). Open datum: whether the friend's original "is it
  doing anything" was a genuinely huge repository — which would close
  this item — or a wait somewhere else entirely, which would reopen it
  as a different feature.

From Memnoc's own macOS walk (2026-08-28, `user-supplied`):

- **The launch ritual is too manual.** cd into the repository, type a
  long binary path, run two commands — Memnoc's words: explore "a menu
  that asks the user to provide the path of the repository and that's
  it, no more tasks for them." (`scan` and `serve` always took the path
  as an argument, so the cd was never required — now documented.)
  **Built 2026-08-28 at Memnoc's direction, high priority**, decisions
  made in-session with Memnoc: bare `codeatlas` at a real terminal
  (stdin *and* stderr TTYs) runs an interactive launcher — repository
  path with `~` expansion and honest retry, one open-code yes/no
  (closing the discoverability gap the walk found), then scan → serve →
  the OS URL-opener spawned only after the served port answers, and
  never when the port was already owned by something else. A piped bare
  invocation still prints clap's usage and exits 2, pinned at the
  binary seam. The launcher offers no model flags — it is the no-key
  path, identical in a sealed build — and `docs/SECURITY.md` names the
  one new spawned program (fixed name, loopback URL argument, API key
  stripped) with its enforcing tests. Proven by pty walk: interview →
  standing counter → serve → opener called with exactly the loopback
  URL. Remaining in this neighbourhood for the interview: the
  PATH-install story (an `install` verb, a brew formula) and Apple
  signing.

Open question 4 anticipated this section; it is now collecting.

## Post-harvest field feedback — 2026-09-08

From Memnoc's macOS binary test (`user-supplied`). The run itself was
clean: download, dequarantine, scan, serve, dashboard — no failure. What
it found is one layout defect and one absence.

- **The way out breaks before the caption does.** In the magnify view
  with both side panels open, the `Back to assets` button wraps its
  label onto three lines inside its own border, and the breadcrumb pill
  grows tall around it. Cause, read from the source rather than guessed:
  `.breadcrumb` (`dashboard/src/app/styles.css:1112`) is
  `position: absolute` with only `left` set, so its shrink-to-fit width
  is capped by whatever the canvas has left once the files panel and the
  source panel have taken theirs. Its children are flex items at the
  default `flex-shrink: 1`, and `.back` (`styles.css:1453`) sets no
  `white-space`. So the squeeze lands on the *control* — the one element
  in the trail that has to stay pressable — while `.crumb-note`
  (`styles.css:1154`), the longest and most disposable thing in the row
  ("1 neighbour — what it leans on below, what leans on it above",
  `MapExplorer.tsx:1112`), keeps its full width. The priority is
  inverted: the caption should lose first. Ticket-sized, not a decision
  — hold `.back` at `flex: none; white-space: nowrap`, bound
  `.breadcrumb` to the canvas width, and let `.crumb-note` truncate.
  Where the interview should push: the trail already carries a back
  button, two-to-three crumbs, a note, and up to two `.reveal` controls
  — whether it survives a narrow canvas by truncating or by dropping
  parts outright is a design call, not a CSS one.
- **There is no TUI, and there was never a decision to build one.**
  Memnoc expected one for the whole run and did not find it. Correctly
  so: nothing in V1–V3 specified a full-screen terminal interface, and
  no terminal-UI crate (`ratatui`, `crossterm`, `indicatif`) appears in
  any manifest. What exists is three separate pieces of terminal
  courtesy, each shipped for its own reason — scan's standing
  `scanning: n/N files` counter on a stderr TTY (`scan.rs:53-166`),
  enrich's `enriching: …` plan and tally lines (`enrich.rs:953`), and
  the launcher's question-and-answer prompts. Together they read as
  progress, not as a screen. The open item recorded above — "whether
  other long-running commands deserve the same line" — is the *small*
  version of this question; the field feedback raises the large one: a
  single owned frame for the whole run, scan through serve. That is a
  V4 candidate for the interview, and it is a decision, not a ticket:
  a TUI takes over stderr, and every guarantee currently proven at the
  pipe seam (byte-identical output when stderr is not a terminal) has
  to survive it.
- **Open code opened the file but did not colour it.** Memnoc's words:
  "in open code the ui is not highlighting the code, just opening the
  file." Read against the source, this is **not a defect** — it is
  ADR-0013's stated plain-text fallback doing exactly what it says, and
  the run that produced it was a repository the fallback swallows almost
  whole. Seven grammars highlight
  (`crates/codeatlas/src/highlight.rs:131-183`): `ts` `tsx` `js` `jsx`
  `mjs` `cjs` `rs` `py` `go` `c` `h` `cpp` `cc` `cxx` `hpp` `hh` `hxx`.
  Everything else — HTML, CSS, SCSS, JSON, YAML, shell, TOML, template
  languages — comes back escaped and uncoloured with `language:
  "plain text"`. The tested repository is a static site: its `assets`,
  `templates`, `themes` and `manual` regions are mostly not in that
  list, so the one file that *did* colour was the `.js` one. Two things
  for the interview, and they are different sizes:
  - **The coverage question (a decision).** Which grammars, if any, V4
    vendors beyond the scanner's set. It trades binary size against how
    much of a real repository opens lit, and it is the same zero-egress
    calculus ADR-0013 already ran — no new principle, only new weight.
  - **The asymmetry (a ticket, and the sharpest case).** Markdown is
    *scanned* — `parsers/markdown.rs:26` claims `md` and `markdown`, so
    `.md` files are map nodes and openable — but no markdown grammar
    exists in the highlighter. A file the map invited the reader to open
    is guaranteed to open grey. The `manual` region's 134 files are very
    likely exactly this. Whatever the coverage decision says, the
    scanner's languages and the highlighter's should not disagree
    silently.

  The fallback *is* disclosed — `SourcePanel.tsx:80` renders the
  envelope's language, and `.source-language` (`styles.css:670`) is a
  muted 11px pill in the source head. It told the truth and Memnoc still
  read the result as broken, which is the finding: the pill names a
  language, so "plain text" reads as a label rather than as *no grammar
  shipped for this file*. Worth a sentence that says the second thing.

**Resolution, 2026-09-08, same session.** Memnoc confirmed the launcher
verdict the report left unstated: it ran, fast, and the browser opened
itself — the `open` spawn's first human run on macOS, **pass**. The
interaction was judged "way too clunky", and Memnoc chose the modal
direction on the spot; three shape decisions were put and answered
(modal owns the launcher only, not the whole run; repo picking is
browse-plus-type; the two dashboard fixes ride along). Built same day:
a hand-drawn crossterm modal (no TUI framework — the ADR-0011 call
again), pure state machine with the plain interview kept as the
raw-mode-refused fallback, pty-walked end to end. The breadcrumb
squeeze and the pill wording are fixed as prescribed, each pinned in
`stylesheet-contract.test.ts` / `open-code.test.tsx`. Still for the
interview, sharpened rather than closed: the **whole-run owned frame**
(the modal is its first slice, deliberately not its last), the grammar
coverage decision and the markdown asymmetry above, and one boundary
recorded while building: a persisted **recent-repositories list** was
deliberately not built — it would be CodeAtlas's first file outside the
scanned repository, a retained record of what the reader scanned, and
that is a decision with a documented location and a way to clear it,
not a convenience default.

**Second round, same day — Memnoc's Linux walk of the modal.** Two
datums, both acted on immediately:

- **The passive checkbox lost to the forced question.** Memnoc served a
  repo and looked for Open code in the dashboard — it was off, because
  the modal's `[ ]` row was sailed past where the old interview's
  `[y/N]` question could not be. The author of the feature being the
  one it bit is the strongest evidence discoverability will get. Fixed:
  Enter now lands on a **confirm frame** — the chosen path with
  `OPEN CODE ON/OFF` stated loudly, `o` flips it there, `Enter` goes,
  `Esc` backs out — the forced-decision moment restored without giving
  up the modal.
- **The navigator existed and was not found.** From `~/Downloads`
  (no code beneath it), Memnoc typed a full path from memory — while
  `h`/`l` navigation sat in the footer unread. A footer key nobody
  reads is not an affordance; a visible row is. Fixed: a `../ (up)`
  row now sits in every listing with a parent, and Enter on it climbs
  (navigates, never "chooses" the parent).
- Plus the polish asked for: the frame now colours by meaning through
  the terminal's own 16-colour palette (accent title, highlighted
  cursor row, dim footer, red errors) — the reader's theme, never a
  shipped one.
- **Third round, same walk: Enter had the wrong meaning.** Memnoc
  pressed Enter on `Code/` expecting its projects and got a selection —
  the file-manager convention their fingers know is *Enter opens*. Now
  Enter walks into folders, and only the pinned `. (map this
  directory)` row selects, with the second footer line saying so out
  loud. And per the same round's ask, the frame centres itself and
  sizes to the terminal (inner width to 72 columns, list rows with
  terminal height, resize followed live), with clipping moved to the
  paint layer so text truncates only when the terminal genuinely
  cannot hold it.
- Small datum from the same walk, unfixed and noted: a second launcher
  while one serves fails honestly at the bind (port 4173) after
  scanning — the pre-check correctly refuses to open a browser at the
  other instance's server. Whether the launcher should offer another
  port is a small V4 question.
- **A want, not a bug, shipped in the same run:** the dashboard's third
  theme — Rosé Pine's main variant, per Memnoc's ask — is now the dark
  default (the media query and `systemTheme()` both say `main` for a
  never-chosen dark-OS reader; Moon became an explicit choice, reached
  through the header's new three-way cycle Dawn → Rosé Pine → Moon).
  The token architecture absorbed it as its own comment predicted: "a
  third variant would be one more block and nothing else."
- **Fourth find, same day, in the dashboard: the source panel cut off
  line 1 of a six-line file.** `.source-code` had `overflow-x: auto`
  all along — the scrollbar sat at the bottom of a flex-stretched
  block, present and undiscoverable, invisible under overlay
  scrollbars. The remedy is wrapping, not a better scrollbar: lines
  now soft-wrap with a hanging indent past the painted gutter, so all
  the code is visible always. And per the same report's ask, the
  panel gained a **full-screen toggle** (⤢ beside the close control):
  `position: fixed; inset: 0` at z-index 60 — held below the
  walkthrough's 100 by the existing stacking sweep — per-reading
  state that resets on close. Both guarded and tamper-proven
  (`stylesheet-contract.test.ts`, `open-code.test.tsx`).

## Hand-off

Fresh session, `/adr-with-docs`, this document as the agenda — **when
Memnoc calls the lap**, not before. V4 walks the same road V1–V3 did;
its spec will be a new file — a spec that shipped is a record, never
edited into a new version.
