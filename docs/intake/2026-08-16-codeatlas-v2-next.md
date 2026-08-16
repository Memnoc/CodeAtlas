# CodeAtlas V3 agenda — harvested from the V2 ship

> Produced by `/next` on 2026-08-16. Triage decisions in this doc are
> **Memnoc's**, made in the harvest session: the two headliners were named
> explicitly ("the most important thing is distribution and open code");
> the remaining dispositions were presented in the same pass and accepted
> without correction. This is an agenda for `/adr-with-docs`, not a
> commitment: nothing here is a spec, a glossary entry, or a decision until
> the interview makes it one.

## Where V2 landed

Shipped 2026-08-14. All 26 stories pass on the first harden walk — serve
stories watched live over real TCP with the scripted double, significance
recomputed 349/349 exact from the real map's own edges, enrichment
carry-over proven by a second run buying nothing — and Memnoc walked the
four reader's-walk spots in the browser and accepted, recorded in the
spec's `## Verification` (`docs/specs/2026-08-13-codeatlas-v2.md`). No
fails, no unverifiables. The spec is a record now; V3 gets a new file.

Context the interview should hold (user-supplied, 2026-08-16): nobody but
Memnoc has run CodeAtlas yet; Memnoc's own use has not been exhaustive; a
business deployment is a while away.

## V3 candidates

**1. Distribution** — *the headline, Memnoc's call 2026-08-16, executing
the standing polish-before-distribution sequencing now that the polish lap
has shipped.*
Source: V2 spec Out of Scope ("its own lap after V2 ships"); the V1-next
doc's candidate 5 already sketches the shape (tagged-release GitHub Actions
workflow, static Linux/macOS binaries, dashboard already embedded so one
downloaded file works, release notes saying loudly that no key is needed
for scan/serve/diff/share/schema). Riders from the memnoc.dev Built-entry
gate (user-supplied): licensing, a Known Limitations statement, accurate
claims, provenance — what a listed project owes before it is listed.

**2. Open code** — *the second headliner, Memnoc's call 2026-08-16.*
Source: V2 spec Further Notes, parked for `/next` on 2026-08-14: opening
the selected file or symbol as highlighted source, the way RepoAtlas does.
The parked entry records three blockers the interview must resolve, each
pinned by a test or an ADR: it is a new class of serve surface
(source-over-HTTP) that would falsify the never-serves-your-source
sentence `docs/SECURITY.md` states and the drift test pins; it cannot
exist in share mode under ADR-0011's two-megabyte ceiling; and syntax
highlighting has no path under ADR-0006. **It starts as ADR-0013, not as
a ticket** — the spec says so verbatim.

**3. HTTP conformance nits** — source: ticket 13 residuals, marked V-next
harvest material at crosscheck: the 405 omits the `Allow` header RFC 9110
§15.5.6 makes a MUST; `starts_with("HTTP/")` accepts a four-token request
line; an unrecognised method draws 405 where purist HTTP wants 501. One
small ticket riding behind the headliners.

**4. Ask-route tightening** — source: ticket 08 residuals: carried
`citations` arrays ride per-field unbounded (only `MAX_BODY` bounds them),
and the 400 for a structurally-wrong turn is untested. Rides with any
serve ticket.

**5. Old-binary enrichment data loss** — source: ticket 07 residual: a
pre-V2 binary running `--enrich` against a newer store rebuilds it
wholesale and silently drops purchased layer descriptions. Bounded, but
the only residual shaped like data loss of paid-for prose — and
distribution multiplies old binaries in the wild, which is why it enters
this lap rather than staying parked.

## Parked

- **Concurrent-connection cap** — V2 spec Out of Scope and ticket 12's
  honest limitation bullet in `docs/SECURITY.md`: the read bounds change a
  thread's tenure, not how many threads a patient client can hold. Stays
  "a decision argued on its own"; loopback-only and no external users yet.
- **The five remaining parser gaps** (Go package-file anchoring, Rust
  macro-interior calls, C `static inline`, Python duplicate `def`,
  Markdown self-loop) — V2 spec Out of Scope; C++ was judged the only one
  worth the last lap and nothing has changed the ranking.
- **C++ scope-tracking family** — ticket 10 residuals: unqualified
  same-namespace sibling calls, `using namespace`, namespace aliases; one
  family, parked together with the gaps above.
- **Enrichment internals bundle** — V1-next parked, untouched since:
  `fill_slots` duplicates `build_tour`'s tally byte-for-byte, flow-hash
  granularity re-purchases identically-prompted names, `MAX_TOKENS`
  untested against the real model, no config-file model override. No
  enrichment pain reported since.
- **Annotation-store reviewer machinery** — V2 spec Out of Scope, "parked
  until a reviewer reports pain"; no reviewer has (user-supplied: nobody
  else has run the tool yet).
- **Top-40 constant user-adjustable** — V2 spec Further Notes open
  question: "a knob nobody asked for is speculative generality." Reopens
  only if maps much larger than this repository's appear.
- **CLI wrappers (chat / explain / onboard)** — V1-next parked; the
  capability lives in the dashboard, the command-line surface waits until
  something needs it.
- **Northstar → CodeAtlas producer skill** — V1-next parked twice;
  unblocked is not urgent.
- **Local-model provider** — V1-next parked; the provider trait keeps the
  door open (ADR-0004). Note for the interview: distribution brings users
  without Claude subscriptions, which is the first circumstance that could
  reopen this.
- **Share/redaction hygiene bundle** — V1-next parked: dual schema-walker
  divergence risk, disclosure banner conflating dropped vs replaced,
  `is_inert` duplicated cross-language, version pattern rejecting
  prerelease semver.

## Dropped

- **Small QA debts** — ticket-recorded residuals: jsdom scroll tests prove
  arithmetic not browsers, canvas interactivity ticked on inference, the
  ~581px viewport floor unrecorded, the dismissal-race guard sound but
  untested, CI could skip the 20 s trickle test in feature legs. None
  blocks anything; each is written down in the ticket where it lives.
- **Symbols on the canvas, magnify depth control, semantic search** — V2
  spec Out of Scope, reasoning unaged (314 files, density, a different
  product decision); reaffirmed by omission.
- **Closed-by-ADR items** (server-held sessions, cost-in-currency, layout
  library, dashboard-side share, Domain entity) — stay closed; the
  business deployment that might reopen them is a while away
  (user-supplied), and the superseding mechanism in `docs/adr/` is the
  recorded path if it arrives.

## Open questions — for the interview to grill first

1. **Open code versus its three blockers.** ADR-0013 must decide what
   gives: the SECURITY.md sentence and the drift test that pins it, the
   share-mode story under the 2 MB ceiling, and a highlighting path that
   respects ADR-0006's zero-egress gate. Each blocker is enforced by a
   test or an ADR — none can be waved through.
2. **Sequencing the headliners.** Does open code land before distribution
   (the first public binary carries it) or after (distribute what is
   already hardened)? The answer shapes both laps.
3. **What "ready to distribute" requires.** Nobody but the owner has run
   the tool, and the owner's own use has not been exhaustive
   (user-supplied). What bar does a public binary owe — a dogfooding
   period, a fresh-machine install walk, something else?
4. **The Built-entry gate as release checklist.** Licensing, Known
   Limitations, accurate claims, provenance — which are release blockers
   and which are listing blockers?
5. **Does distribution reopen the local-model provider?** Parked above,
   but it is the one parked item whose blocking circumstance distribution
   itself changes.

## Hand-off

Fresh session, `/adr-with-docs`, this document as the agenda. V3 walks the
same road V1 and V2 did; its spec is a new file — a spec that shipped is a
record, never edited into a new version.
