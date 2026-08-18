# CodeAtlas V3 — open code, then hand it to strangers

> Produced by `/to-spec` on 2026-08-16 from the `/adr-with-docs` interview of
> the same day. Decisions are Memnoc's, recorded in
> [ADR-0013](../adr/0013-open-code-is-a-flag-gated-serve-route-highlighted-by-the-vendored-grammars.md)
> and
> [ADR-0014](../adr/0014-distribution-is-attested-github-releases-sealed-beside-default.md);
> the agenda was `docs/intake/2026-08-16-codeatlas-v2-next.md`. All terms are
> `CONTEXT.md`'s.

## Problem Statement

A reader drills to the exact file the map says matters, reads its enriched
summary, asks the map a question about it — and then the trail goes cold.
The map can name `resolver.rs` but never show it: the natural last step of
every exploration, "let me see the code," dead-ends into a context switch
to an editor, carrying the filename by hand.

And the only person who has ever taken that walk is the person who built
the tool. Running CodeAtlas today means cloning the repository and owning a
Rust toolchain plus npm — a wall that filters out exactly the newcomer the
dashboard was designed for. There is no artifact to download, no tag, no
release: the polish lap shipped, and the standing decision that
distribution comes next is now due.

Distribution also converts two quiet facts into real hazards. A released
binary lives for years: today's store format tolerance — an older binary
that rewrites a newer annotation store silently drops purchased prose —
is theoretical while every binary is Memnoc's, and data loss the day it
isn't. And the serve surface still cuts three small corners of HTTP that
nobody noticed on loopback but a stranger's tooling will.

## Solution

A reader who starts `serve --open-code` clicks a file or symbol and reads
its source right there — highlighted, opened at the symbol's own lines,
the map still on screen. A reader who starts plain `serve` gets a server
that still cannot serve source, and an auditor can watch the tests that
prove it.

Anyone on Linux or macOS downloads one file, runs `codeatlas scan . &&
codeatlas serve .`, and has a served map in minutes — no toolchain, no
key, no account. The download's origin is machine-verifiable, the sealed
build sits beside the standard one for the security reviewer, and the
release notes say plainly what needs no credential, what needs Claude, and
how the thing was built. Old binaries stop being a threat to purchased
prose before the first one ships.

## User Stories

**Open code**

1. As a reader, I want opening the selected file's source beside the map,
   so that the map's last step no longer dead-ends into my editor.
2. As a reader, I want opening a symbol to land scrolled and lit at its
   own lines, so that I read the function rather than hunt for it.
3. As a reader, I want source in the grammar-covered languages
   highlighted and everything else readable as plain text, so that
   highlighting never gates what I can open.
4. As a reader, I want a file past the size cap opened truncated with a
   visible notice, so that a huge file opens usefully instead of refusing.
5. As the serving binary, I want the source route to not exist without
   `--open-code`, so that a plain `serve` still never serves source
   ([ADR-0013](../adr/0013-open-code-is-a-flag-gated-serve-route-highlighted-by-the-vendored-grammars.md)).
6. As the serving binary, I want only files that are nodes in the map to
   be servable, so that no request can walk the filesystem.
7. As the dashboard, I want the capabilities route to say whether open
   code is on, so that the affordance is absent rather than broken — and
   absent in every share artifact for the same reason.
8. As a reader, I want a mapped file that has since been deleted to draw
   an honest 404, so that a stale map never fabricates source.
9. As an auditor, I want the source route named in `docs/SECURITY.md` by
   the existing drift gate, and the model boundary unchanged — ask still
   sends no file contents — so that the posture stays tested, not
   documented.
10. As the repository owner, I want share artifacts to carry no source
    and no open-code affordance, so that the redaction trust boundary
    holds for recipients who do not hold the code.

**Distribution**

11. As a newcomer on a fresh machine, I want to download one file for my
    platform and reach a served map with no toolchain and no key, so that
    trying CodeAtlas costs minutes, not an afternoon.
12. As the release workflow, I want a version tag to build the default
    and sealed binaries for all four targets with a checksums file and
    build-provenance attestation, so that a release is evidence, never a
    hand-built upload
    ([ADR-0014](../adr/0014-distribution-is-attested-github-releases-sealed-beside-default.md)).
13. As the release workflow, I want the full CI gates re-run and the
    sealed probe run against the release artifacts themselves before
    anything publishes, so that no artifact ships that CI would not pass.
14. As an auditor, I want to download the sealed binary and verify its
    attestation, so that "exfiltration is a compile error" is a claim I
    can hold in my hand.
15. As the repository owner, I want the first public tag gated by a
    fresh-machine walk, so that "ready to distribute" is a verified story
    rather than a feeling.
16. As a newcomer, I want the release notes and README to state what
    needs no key, what needs Claude, and how this software was built, so
    that every claim survives me acting on it.
17. As the repository owner, I want every released binary to preserve
    annotation-store sections it does not understand, so that an old
    binary can never silently drop purchased prose.

**The protocol honesties**

18. As a reader's HTTP client, I want every 405 to carry the `Allow`
    header, so that refusal follows RFC 9110's MUST.
19. As the serving binary, I want a request line accepted only as exactly
    three tokens and an unrecognised method drawing 501, so that parsing
    keeps the promises HTTP makes.
20. As the serving binary, I want carried citations bounded per field and
    the structurally-wrong-turn 400 pinned by a test, so that ask input is
    bounded in every dimension a client controls.

## Implementation Decisions

**Open code.** The shape is
[ADR-0013](../adr/0013-open-code-is-a-flag-gated-serve-route-highlighted-by-the-vendored-grammars.md):
flag-gated route (the `--ask` pattern — without the flag the route does
not exist), map membership as the allowlist, symbol-level opening via the
contract's existing `range`, server-side highlighting by the vendored
tree-sitter grammars with plain-text fallback, a JSON envelope carrying
the highlighted HTML plus path, language and a truncation flag, and a
named size cap whose tripping is disclosed rather than refused. Spec-level
clarifications on top of the ADR: the route joins the serve registry, so
both route drift gates force the `docs/SECURITY.md` naming — the
mechanism V2 built now works *for* this lap; the capabilities response
gains a second boolean beside `ask`, which is also how the share artifact
inherits the absence; source is read live from disk per request, exactly
as the map and the diff overlay are — a changed file serves its current
contents, a missing one 404s; the dashboard offers opening wherever a
node is already selected (drill view, magnify, the panels), and renders
the returned HTML with its own styles. Highlighting adds one crate on the
existing grammar family, no new grammars and no dashboard bundle growth —
the share ceiling is not pressed.

**The map contract is untouched.** Symbol ranges have been in the
contract since V1; no field is added or changed this lap — the first lap
for which that is true, and worth saying because every prior lap moved it.

**Distribution.** The posture is
[ADR-0014](../adr/0014-distribution-is-attested-github-releases-sealed-beside-default.md):
tag-triggered GitHub Releases starting at `v0.1.0`, default and sealed
binaries for Linux x86_64-musl, Linux aarch64-musl, macOS arm64 and macOS
x86_64, SHA-256 checksums plus GitHub build-provenance attestation, no
GPG, crates.io deferred with the name left unregistered. Spec-level
decisions: the release workflow is its own workflow beside CI, triggered
by `v*` tags and offering a dry-run entry point that builds and verifies
everything while publishing nothing — that dry run is the new testing
seam; Linux binaries are fully static (musl), macOS binaries build on
native runners per architecture; artifacts are named
`codeatlas-<tag>-<target>` with a `-sealed` variant beside each; the
binary's Cargo version and the tag move together, while the map contract
keeps its own version, deliberately separate. The README Quick start
leads with download-and-run and demotes build-from-source to second; the
release notes carry the provenance paragraph — built AI-assisted under
the Northstar pipeline, annotation-store prose self-discloses provider,
model and date — and the no-key sentence for scan/serve/diff/share.

**The store becomes forward-compatible first.** Reading and rewriting an
annotation store preserves top-level sections the binary does not
understand; the store version stays at 2. Sequenced before the release
ticket, because the first distributed binary is the last moment this fix
is free — no shipped binary can be patched retroactively.

**The protocol honesties.** The 405 gains `Allow` naming exactly the
methods served at that path, which deliberately supersedes V2 ticket 13's
byte-identical-405s constraint — that constraint pinned a shape, and this
lap changes the shape on purpose. The request line is accepted only as
exactly three tokens; a syntactically valid request whose method is
recognised by HTTP but not served here draws 501 rather than the 405 that
today claims a method-of-this-path problem. Carried citations get a
per-field bound clamped like every other carried field — the history is
the dashboard's bookkeeping, so clamping over refusing follows
[ADR-0012](../adr/0012-a-conversation-is-client-carried-bounded-input.md)'s
reasoning — and the existing 400 for structurally-wrong turns gets the
test it never had.

## Testing Decisions

Six seams: the five V2 named, one of them idle, plus the first new seam
since the list was written.

1. **The serve HTTP surface** (existing). Real TCP against the real
   binary. Home of: the source route present exactly with the flag and
   absent without it, allowlist 404s for unmapped paths and deleted
   files, the envelope and its truncation disclosure, the capabilities
   boolean on both shapes, `Allow` on every 405, the three-token request
   line, the 501, the citations bound observable on the wire and the
   structurally-wrong-turn 400. No provider is involved on the source
   route; the scripted double appears only where ask does. Prior art: the
   V2 serve suite and the `--ask` route-existence tests.
2. **The rule beside the wire** (existing pattern). Highlighting
   unit-tested at the source module: every vendored grammar yields spans,
   an uncovered language falls back to plain text, the cap trips the
   flag. The HTTP tests prove the plumbing; these prove the rule — the
   same division ask's slice tests already use.
3. **The share artifact tests** (existing). The artifact contains no
   source, no source route reference and no open-code affordance; the
   two-megabyte ceiling test keeps riding.
4. **The jsdom component seam** (existing). Gesture→state: opening from
   a selected file and from a symbol, the truncation notice rendered,
   the affordance absent when capabilities says off. Geometry goes to the
   stylesheet contract only if the view needs pinning.
5. **The enrichment store fixtures** (existing). A store carrying an
   unknown top-level section survives read → rewrite with the section
   byte-preserved — proven able to fail by a tamper before its criterion
   is ticked.
6. **The release pipeline** (new). No `cargo test` exercises a
   tag-triggered workflow, so the seam is the workflow's dry-run entry
   point (or a throwaway prerelease tag): all eight artifacts built,
   checksums verified, attestation produced and verified, the sealed
   probe run against the built artifacts. The real `v0.1.0` is cut only
   after `/harden` walks story 15's fresh-machine walk — the human
   verification layer, exactly as V2's reader's walk was.

The map contract seam sits idle this lap — no contract change to test. A
good test here asserts external behaviour — what the wire returns, what
the artifact contains, what the workflow published — never internals.
Every guard added must be proven able to fail before its criterion is
ticked; that rule has earned itself in this repository beyond argument.

## Out of Scope

- **Feeding opened source into ask's slice** — out; a different decision
  with its own bounds story, stated as the boundary in ADR-0013.
- **Editing source from the dashboard** — out; open code is a reader, not
  an editor, and nothing upstream asked for writes.
- **Source in share artifacts** — rejected in ADR-0013; the trust
  boundary, before the ceiling is even reached.
- **An always-on source route** — rejected in ADR-0013; loopback ports
  are readable by any local user, file permissions are not.
- **A Windows target** — deferred in ADR-0014, until someone asks.
- **crates.io publishing** — deferred in ADR-0014 to its own decision;
  the name stays unregistered.
- **GPG signing** — rejected in ADR-0014; attestation covers origin
  without the key-management burden.
- **A local-model provider** — parked, demand-driven; release notes say
  honestly what needs Claude and what needs nothing.
- **A concurrent-connection cap for serve** — still its own argument, as
  V1 ticket 38 and the V2 spec both recorded.
- **The five remaining parser gaps and the C++ scope-tracking family** —
  parked; nothing re-ranked them.
- **Annotation-store reviewer machinery** — parked until a reviewer
  reports pain; no reviewer exists yet.
- **A user-adjustable top-40 constant** — parked; a knob nobody asked for
  is still speculative generality.
- **Northstar's Built-entry gate items** — different repository, filed
  separately.

## Further Notes

- Source material: `docs/intake/2026-08-16-codeatlas-v2-next.md`, ADRs
  0013–0014, `CONTEXT.md` (Open code is its 26th term). Two stale
  premises were corrected during the interview and are recorded in
  ADR-0013's Context and the intake doc: highlighting always had a
  zero-egress path through the already-vendored grammars, and the
  license gate item was already closed (MIT, tracked 2026-08-13).
- Open question, non-blocking: the exact highlight crate pairing
  (`tree-sitter-highlight` against the pinned tree-sitter version) is
  verified at ticket time; if the versions disagree, the fallback is
  driving the grammars' own highlight queries directly — still vendored,
  still zero egress, same decision.
- Open question, non-blocking: aarch64 Linux builds on a native arm
  runner versus cross-compilation is a ticket-time choice; the C grammar
  objects make a native runner the likely answer.
- The fresh-machine walk needs a target repository that is not CodeAtlas;
  picked at walk time.
- Sequencing inside the lap, per the interview: open-code stories first,
  the store forward-compat story before the release ticket, the release
  ticket last, `v0.1.0` cut only after the harden walk.

## Verification

Walked 2026-08-16 by the harden session, against main at `5dd8152` — all
nine tickets `done`, the release dry run green
(github.com/Memnoc/CodeAtlas/actions/runs/31963531524). Method notes
follow the table; execution was preferred throughout — real TCP against
release binaries, the dry run's own downloaded artifacts, a real share
artifact, the committed store on a repo copy. Fresh backstops this
session: full default Rust suite (15 result blocks, 0 failures),
dashboard 307/307, tsc clean.

| # | Story | Verdict | Method |
|---|-------|---------|--------|
| 1 | open file source beside the map | pass | wire: live envelope watched (edit served); beside-map at the jsdom seam |
| 2 | symbol lands scrolled and lit | pass | jsdom seam (lands lit 3–9, scrolled); wire: symbol id draws 404, file nodes only |
| 3 | seven languages highlighted, rest plain | pass | watched on the wire: all seven named + `plain text` fallback stated; `<script>` arrives as entities |
| 4 | past-cap file truncated with notice | pass | wire: 624,000-byte file arrives `truncated:true` AND highlighted; notice at the jsdom seam |
| 5 | source route absent without the flag | pass | watched: flag-off response byte-identical to an unregistered miss |
| 6 | only map nodes servable | pass | watched: on-disk-unmapped, symbol id, traversal id → 404; mapped → 200 |
| 7 | capabilities says open code; absent in share | pass | both booleans watched in three server shapes; share absence structural (no wire chunk) + jsdom |
| 8 | deleted file draws honest 404 | pass | watched: names the file and the remedy |
| 9 | SECURITY.md names the route; ask sends no file contents | pass | drift gates executed 5/5 both directions; ask prompt bounds by committed tests (seam execution) |
| 10 | share carries no source, no affordance | pass | real artifact: zero occurrences of every REGISTRY route; probe + ceiling 14/14; note: the button label rides as dead string data in the shared component — the control never renders (asserted against a lying server, zero fetches) |
| 11 | newcomer downloads one file → served map | pass | dry-run artifact on this machine: static ELF (`ldd`: statically linked), no key in env, stranger repo mapped and served; the literal fresh machine is story 15's layer |
| 12 | tag builds all targets, checksums, attestation | pass | seam 6 executed: the green dry run built 8 binaries + checksums, attested and verified all 9 subjects in-run |
| 13 | CI gates + sealed probe before publish | pass | run job graph: all gates green, sealed probe green per target, publish leg skipped (tag-only by `if`) |
| 14 | auditor verifies the sealed binary | pass | checksums OK locally; attestation fetched by the downloaded file's sha256 digest — names this repo, `refs/heads/main`, `release.yml`; full `gh attestation verify` green in-run (local gh predates the subcommand — an auditor needs gh ≥ 2.49) |
| 15 | first tag gated by fresh-machine walk | **pass** | walked by Memnoc 2026-08-18 — addendum below |
| 16 | release notes/README say no-key, needs-Claude, how-built | pass | the dry run's rendered notes.md read: no-key section, measured artifact sizes, verify instructions, provenance paragraph, zero unfilled slots; README claims adversarially cross-checked in ticket 08 review + provenance fix `3022fe6` |
| 17 | released binary preserves unknown store sections | pass | seam 5 executed (preservation/tamper tests 5/5); committed store byte-identical through a plain scan on a repo copy (sha256 compared); binary↔commit lineage held by attestation |
| 18 | every 405 carries Allow | pass | watched: `GET, HEAD` everywhere; `GET, HEAD, POST` exactly where ask is registered |
| 19 | three tokens or nothing; unrecognised → 501 | pass | watched: `FROB` → 501, four-token line → 400, three-token control 200 |
| 20 | citations bounded; wrong-turn 400 pinned | pass | watched: malformed turn → 400 with self-explaining error, provider unconsulted; over-count/over-length citations → 200, never refused; clamp steering pinned by wire tests in the fresh suite run |

Between-the-slices checks, all watched this session: truncation composes
with highlighting on one response (01×03); Allow composes with flag-gated
routes — the flag-off source path is a miss, never a 405 (01×05); the
capabilities booleans held to the routes they describe in three server
shapes (01×05×06-ticket); HEAD mirrors GET on the source route (01×05);
the committed store round-trips a plain scan byte-identical (07); the
walkthrough-steps rename left dashboard suite and typecheck green (09's
fix); the rendered release notes consume ticket 08's template with every
slot measured (08×09).

Unverifiable at the 2026-08-16 walk: story 15 only — the fresh-machine
walk is the deliberate human gate on `v0.1.0`, exactly as V2's reader's
walk was. Shipping therefore waits on that walk (or Memnoc's recorded
acceptance in its place). One tooling note: local `gh` here is too old
for `attestation verify`; the story passes on in-run verification plus a
digest-level API check, and a stranger auditing with current gh
reproduces the full check.

### Addendum 2026-08-18 — story 15 walked by Memnoc

The human walk happened on 2026-08-17/18 and story 15 moves to **pass**,
conditions stated exactly:

- **Walker and machine**: Memnoc, on their own Linux x86_64 machine — not
  a factory-fresh one. The no-toolchain claim rides the statically linked
  musl binary (`ldd`: not a dynamic executable, proven in-workflow and
  locally) and the walk never invoking a toolchain; a literally fresh
  machine remains open to anyone who wants the purer form.
- **Artifact source**: the green dry run's `binaries-x86_64-unknown-linux-musl`
  (run 31963531524), since the walk gates the release that would otherwise
  be the download page. Noted friction that vanishes post-tag: workflow
  artifacts need a GitHub login and live at the run's Summary page —
  the real newcomer gets plain release downloads.
- **Target**: `craftinginterpreters` — 754 files, 4 regions, 1,047
  structural nodes, 498 relationships; multi-language (C, Java, Dart,
  Markdown, HTML). Not CodeAtlas, per the spec's rule.
- **Watched**: download → `chmod +x` → `scan` → `serve` →
  map in the browser at `127.0.0.1:4173`; drill and selection; then with
  `--open-code`: a symbol (`initValueArray`, `c/value.c`) opened beside
  the map, scrolled and lit at exactly its own lines 13–17, C
  highlighted; Memnoc's words: "a very neat user experience".
- **Friction log, all of it**: (1) the artifacts download hid on the run's
  Summary page — pre-release-only friction; (2) `-scan` typed for `scan` —
  the CLI refused with usage, honestly; (3) with plain `serve`, Memnoc
  looked for open code and nothing hinted it exists behind a flag —
  exactly ADR-0013's absent-not-advertised design, recorded here as a
  discoverability datum for `/next` to weigh.

With this walk and the compliance record's closed remediation gate
(`5e9f692`), every story passes and nothing further gates the tag:
**`v0.1.0` is the next act, and it is Memnoc's push.**
