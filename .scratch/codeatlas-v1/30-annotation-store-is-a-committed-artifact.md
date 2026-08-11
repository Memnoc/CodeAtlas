# Ticket 30 — enrichment arrives with the repository

**Status:** done
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 18 — enrichment someone else already paid for arrives with the
repository, so cloning is all I have to do
**Blocks:** none
**Blocked by:** none

## Problem

Enrichment is a per-developer purchase. `.codeatlas/` is git-ignored, so prose
one person paid for is invisible to everyone else, and a colleague without a
credential sees a structurally complete map with nothing but mechanical labels
on it.

The mechanism to fix this already exists and is one line from working: the
annotation store re-attaches on *every* scan, before the enrichment branch is
even considered, so a plain `codeatlas scan` with no credential and no network
already consumes committed prose. The only blocker is that the file cannot be
committed.

## What to build

One person enriches, commits, pushes. Everyone else clones and runs a plain
`codeatlas scan` — no credential, no network, no flags — and gets the map with
all its prose. The store says what produced that prose, so a reviewer looking
at the diff can see whether it came from Opus 5 through the API, a
subscription CLI, or something else.

## Acceptance criteria

- [x] A scan writes `.codeatlas/.gitignore` that ignores the directory's
      contents and un-ignores the annotation store, so `git check-ignore`
      classifies the map file as ignored and the annotation store as not.
- [x] A `.codeatlas/.gitignore` the user has edited is never clobbered — see
      Notes, this refines rather than contradicts ADR-0007.
- [x] The store records the provider, the model, and the date that produced
      its prose.
- [x] Those fields are additive: a store written before this ticket still
      loads and still re-attaches, without a store-version bump.
- [x] The story's actual claim is tested end to end — enrich a fixture with a
      fake provider, discard the map file, run a plain `scan` with no provider
      selected at all, and assert the prose is back and provenance is `llm`.
- [x] The regenerated map file stays ignored. It is ~790 KB on this repository
      and rebuilt every run; committing it would be pure diff noise.

## Notes

**ADR-0007 says "every scan writes a `.codeatlas/.gitignore`" and does not
settle what happens when one already exists.** This ticket settles it: write
the file when it is absent or byte-identical to the default, and leave a
modified one alone. Clobbering would silently discard a deliberate choice —
someone who decided *not* to publish their prose has made a real decision, and
a tool that overwrites it every scan is a tool people stop trusting with their
repository.

The reconciliation with ticket 14's redaction is recorded in ADR-0007 and does
not need restating in code: the line is the trust boundary, not the prose. A
share artifact goes to someone who does not hold the source; a committed store
goes only to people who already do.

Worth knowing before starting: enrichment has never actually run on this
repository, so there is no `annotations.json` here to look at. The fixture
route through a fake provider is the only way to produce one without spend.

## What the work found

**The mechanism was one line from working and this repository was the thing
blocking it.** Git will not let a nested ignore file re-include anything under
a directory an outer `.gitignore` excluded outright — the one exclusion rule
with no escape — and CodeAtlas's own `.gitignore` said `.codeatlas/`. So the
feature would have shipped, passed its tests against temp fixtures, and done
exactly nothing on the repository that dogfoods it. The root rule is now
`**/.codeatlas/*`: contents ignored, directory reachable, nested negations
honoured, and the `**/` keeps the old pattern's reach for a scan run from a
subdirectory. The same trap is waiting in every repository that followed the
old README's advice, which is why the README now says the opposite and
`docs/SECURITY.md` records it as a limitation. It fails in the safe direction
— prose stays unpublished rather than being published by surprise — and
CodeAtlas deliberately does not read the repository's own ignore rules to
second-guess them.

**`git check-ignore` was worth the extra machinery.** The obvious test reads
`.codeatlas/.gitignore` and looks for `!annotations.json`, which asserts that
a string is in a file rather than that git obeys it. `--no-index` is the part
that matters in the clone test: without it git reports a *tracked* path as
un-ignored no matter what the rules say, and the assertion that the store is
publishable would have been true of any committed file.

**A real clone proves what deleting the map file cannot.** The ticket asked
for the map to be discarded; committing and cloning discards it for the right
reason. In the clone the map is absent *because git never took it* and the
store is present *because git did*, so the two halves of the ignore file are
tested by the same act that the story describes. The plain scan in the clone
selects no backend at all, and a test build has no default provider, so a run
that reached for one would fail rather than quietly spend money.

**The store's provenance is one record, not one per annotation.** ADR-0007
says the store records provider, model and date, and the tempting reading is
per-annotation. It cannot be honest: `save_store` rebuilds the whole store
from the enriched graph on every purchasing run, and a carried-over
annotation's original run is not recoverable from the graph — so
per-annotation provenance would be right for the slots that run bought and
quietly wrong for every slot it inherited. One record for the store says what
the last run to write it was, which stays true.

**The backend names itself, through a defaulted trait method.** Only the
backend knows the model actually used: `--model` is optional and each backend
answers differently to being given none — the API provider pins
`claude-opus-5`, the CLI provider leaves the choice to the subscription and
genuinely never learns which model answered, so it records `null` rather than
a guess. Reconstructing that from the spec string at the selection site would
have been an invention. The method is defaulted because the trait has a dozen
implementors between the test doubles and the offline backends, and none of
them will ever write a store.

**Additive turned out to be free, which is exactly why it needed a test.**
Serde treats a missing `Option` field as `None` without `#[serde(default)]`,
so removing that attribute did not break the old-shaped store — the guard
looked untamperable until it was tampered the other way, by making `load`
require the field. Both the unit test and the integration test then failed.
The integration test also asserts that the roll-back it performs actually
removed something, so it cannot pass by rolling back nothing; that assertion
is what fired first when the field was suppressed at the source.

**A date without a dependency is fifteen lines.** ADR-0006 admits none, and a
calendar crate is a lot of supply chain for one line of a JSON file, so
`civil_date` is Howard Hinnant's `civil_from_days` pinned against dates
checked by hand — the epoch, the day before it, the 2000 century leap that the
naive `%4` rule gets wrong, and both sides of 2024's leap day. A date and not
a timestamp, because this is read by a person in a diff and a second-resolution
clock would churn the file on every run that changed nothing else.

**The `"claude"` spec literal became `claude::SPEC`.** Recording the provider
name needed the string in a second place, and a second literal is a second
thing to drift; `agent_cli` already had a `SPEC` const for the same reason.
That also kept the sealed byte probe's needle set unchanged — but it did move
the recorded *counts*, and re-measuring found the table in `docs/SECURITY.md`
and `scripts/sealed-probe.sh` had already drifted before this ticket (`ureq`
under `network` was 22 and measures 23, which nothing here touched). Both are
re-measured, and both now say what the numbers are: `grep -c` line counts over
a binary, which move with the toolchain. Only the sealed row's zeros are
asserted, and the shape the table is read for — default is the sum of the two
above it — survives: 5 is 3 + 2.

**Left deliberately undone.** CodeAtlas does not warn when a parent
`.gitignore` has neutralised the nested file. It could — the `ignore` crate is
already a dependency — but reading the repository's own rules to second-guess
them is a behaviour with its own failure modes, and the outcome without the
warning is a feature that quietly does nothing rather than one that does
something unwanted. Filed as prose in the README and in `docs/SECURITY.md`'s
limitations instead. The `agent-cli` `identity()` also reports the `cli-exec:`
stand-in by name rather than as `cli:claude`, behind a `cfg(test-provider)`
split, so no released binary carries a name for a program it cannot run.

## What /crosscheck found

**The sentence this ticket was proudest of was wrong, and the fix for it was
wrong too.** The paragraph above says the byte-count table's shape survives
re-measurement — "default is the sum of the two above it" — and the `ureq`
column in the same table disproved it on the day it was written: 23 plus 0 is
23, but default read 25. The review asked for the claim to be scoped back to
the `claude` column, where 5 was 3 + 2. Re-measuring for this amendment
showed that narrower claim is now false as well. On the same machine, the
same toolchain and the same build directory, `f6b4fa4` reads 2 in the
`agent-cli` cell and 25 in default's `ureq`; this amendment reads 3 and 24,
and the only thing between them is a `#[serde(flatten)]` on a struct in the
annotation store. So the sum was not scoped back. It was removed, from
`docs/SECURITY.md` and from `scripts/sealed-probe.sh`, and both now say why
in terms a future reader cannot mistake for arithmetic: `grep -c` counts
lines, a line in a binary is whatever falls between two `0x0a` bytes, and the
linker decides that. Only the sealed row is a claim. The table's real
argument does not need a sum at all — the `network`-only build has no CLI
backend compiled into it and contains `claude` three times regardless, which
is the whole reason the probe's control asks for `cli:claude` instead.

**Three consecutive tickets have shipped a false sentence into
`docs/SECURITY.md`, and all three were the same mistake:** a claim about
numbers written next to the numbers, without reading them. This one was
caught by measuring before writing rather than after.

**Guarantee 5 opened with a claim nothing enforced.** "A scan writes into
`.codeatlas/` under the scanned root and nowhere else" was true of the code —
`scan::save` and `save_store` are the only writers and both build their path
from `OUTPUT_DIR` — but "the only two writers today" is a fact about a
reading, and the document's own preamble promises a committed test beside
every claim. `a_scan_writes_nothing_outside_the_directory_it_owns`
fingerprints every path under the fixture root except `.codeatlas/` — length,
content hash and modification time, `.git` included — scans, and compares.
Three fingerprints and not one because each catches what the others miss: a
same-length edit, a same-content rewrite, and a filesystem whose timestamps
are too coarse to notice. Its control is that the run demonstrably wrote
something, so an unchanged tree is evidence of restraint rather than of a
scan that never ran.

**The defect this ticket discovered had no regression guard, which is the
sharpest thing the review found.** Every other test in `publish.rs` runs in a
temp fixture, and a temp fixture has no outer `.gitignore` — the one rule
that has ever broken this mechanism. Re-tightening the root rule back to
`.codeatlas/` would have un-published the store again with a completely green
suite. `this_repositorys_own_annotation_store_is_publishable` asks `git
check-ignore` about the real repository root instead of a fixture. It is four
lines and it guards the exact thing that was broken.

**Documentation prescribed a narrower fix than the tool gave itself.**
ADR-0007 and the README both told a reader to narrow their rule to
`.codeatlas/*` while this repository's own rule says `**/.codeatlas/*`, whose
`**/` keeps the reach of the pattern it replaced for a scan run from a
subdirectory. Not false, but a reader following the documentation got less
than CodeAtlas gave itself. All three documents now say the same thing and
say what the `**/` is for.

**Guarantee 5 also described one keying where there are two.** Annotations
are keyed by node id and carry a hash of the file's contents; layer, flow and
tour labels are keyed by their own ids and carry an `inputs_hash` over the
derivation inputs they were bought for. "Keyed by node id and a content hash"
was true of a quarter of the store.

**`write_ignore_file` failed in the wrong direction.** It answered "does
anything exist here" by reading the whole file, which reports a
present-but-unreadable file, and a directory, as *absent* — and the write
that follows either clobbers or takes the whole scan down with it. For a
function whose entire purpose is never to overwrite somebody's decision,
failing open into a write is backwards. It now asks `try_exists`, writes only
on a definite "nothing is there", and treats an unclassifiable path the same
as an occupied one.
`a_directory_where_the_ignore_file_belongs_neither_fails_nor_is_replaced`
pins it; against the old code the scan aborted with `cannot write
./.codeatlas/.gitignore: Is a directory`.

**Two assertions were shaped so that they could fail for the wrong reason.**
The store's date was compared against a second reading of the clock, which
fails spuriously on a run straddling UTC midnight — the clock is now read on
both sides of the write and the store's date must be one of the two. And an
absent model was asserted by searching the whole serialized store for the
substring `model`, which is not vacuous today but would fail on any future
field, or any annotation prose, containing those five letters; it now asks
the `produced_by` object whether it has the key, with the presence of
`provider` beside it as the control that the object being asked is the one
that would carry it.

**`ProducedBy` and `ProviderIdentity` differed by one field and were
transcribed across by hand.** `ProducedBy` now holds a whole
`ProviderIdentity` flattened into the same JSON object, so the two cannot
drift and the on-disk shape is unchanged. That refactor is also what moved
two cells of the byte-count table, which is a better argument against reading
shapes into those numbers than any sentence about them.

**Six copies of `materialize` became one.** `fixture_dir`, `copy_tree`,
`materialize`, `git`, `read_json`, `read_map`, `node` and `canned_provider`
now live in `tests/common/mod.rs`, which already existed to hold shared test
policy. What stayed local is what a reader has to see beside the assertion to
know the assertion can fail: `plain_scan`, whose `env_remove` of the provider
variable is the reason no test here can reach a real backend, and
`ignored_by_git`, whose `--no-index` is the difference between asking the
ignore rules and asking whether a file happens to be tracked.
