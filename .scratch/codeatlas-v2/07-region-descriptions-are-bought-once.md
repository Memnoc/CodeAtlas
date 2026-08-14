# Ticket 07 — region descriptions are bought once

**Status:** done
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 8 — region descriptions are bought once and carried over by content
hash, so a re-scan never re-buys prose that is still valid; 7 — the purchased
half of the region card
**Blocks:** none
**Blocked by:** 06 — there is nowhere to put the prose until the field exists

## What to build

Enrichment gains one slot kind — a layer's description — stored in the
annotation store and carried over by content hash exactly as enriched layer
names are.

## Acceptance criteria

- [x] A new slot kind offers each structural layer's description, and a
      provider's answer lands in the description — never in the name.
      Addressing stays collision-proof across kinds through the existing
      key-prefix rule.
- [x] The slot carries mechanically summarised topology only: the layer ID
      and its file count, matching the layer-name slot's discipline. Never
      the member list, never edges.
- [x] The answer is stored and carried over on the same derivation-input hash
      the layer name uses, so a re-scan that changes nothing re-buys nothing,
      and a layer whose membership changed expires its prose.
- [x] Name and description coexist in the store for the same layer without
      either claiming the other's key.
- [x] A store written before this ticket keeps re-attaching. Whether the
      store version bumps is decided explicitly and recorded: a bump is a
      bill, and is only worth sending when the data would otherwise be wrong.
- [x] The cost estimate counts the new slots, so `--dry-run` still says what
      a run will buy.
- [x] A blank or refused answer never replaces the mechanical text — the
      existing rule, extended to this slot.
- [x] Tests use a scripted provider double. No live provider, no API key, no
      subscription spend — ever, in any test in this repository.

## Notes

No new carry-over mechanism. This is
[ADR-0005](../../docs/adr/0005-full-rescan-with-content-hash-enrichment-carry-over.md)
discipline applied to one more slot; if the work seems to need a new hashing
rule, that is a signal the slot is being modelled wrong.

The prose is bought for a *layer*, and layers are stable — this is one of the
cheapest slot kinds in the system. Eight layers here against 1100+ node
summaries; do not add batching machinery for it.

## Decisions (recorded 2026-08-14)

**The store version does NOT bump — `STORE_VERSION` stays 2.** The new
`layer_descriptions` section is purely additive and `#[serde(default)]`:
a store written before this ticket deserializes with the section empty and
keeps re-attaching every summary, name, flow and tour label it holds
(asserted by
`a_store_written_before_descriptions_existed_keeps_reattaching_what_it_holds`
against a hand-written fixture that genuinely lacks the key), and an older
binary reading a newer store ignores the unknown field. Nothing either
holds is *wrong* — the pre-07 store merely has no descriptions to offer,
and the next `--enrich` buys exactly those, incrementally. A bump is a
bill: it would discard every committed store and charge every repository a
full re-enrichment to learn prose a run could buy for eight slots. That is
the `STORE_VERSION` doc comment's own policy ("purely additive optional
fields do not bump it"), applied for the second time; the doc comment now
names this section as the second instance.

**Shape, as ticket 06 specified it.** The slot kind is
`EnrichmentSlot::LayerDescription(LayerSlot)` — it reuses the name slot's
`LayerSlot { id, member_files }` because the two are the same bounded
question about the same topology, asked for different prose. Key prefix
`layer-description:`, so `layer-name:src` and `layer-description:src` can
never claim each other's answers. The store section is
`layer_descriptions: BTreeMap<layer ID, SemanticAnnotation>`, riding the
SAME `inputs_hash` (sorted member set, `semantic_hashes`) the name uses —
asserted by hash equality between the two written sections. Selection and
application key on the description's OWN provenance, so an enriched name
never blocks a description purchase, and the reverse.

## Guards proven able to fail (2026-08-14)

The RED run of 2026-08-14 (skeleton compiled, behaviour unimplemented)
failed 7 of the new tests for the right reasons: slot never offered
(`a_description_answer_lands…`, count 0≠1), coexistence count 1≠2, plan 5≠6
slots, collision test's description answer landing nowhere, store section
missing from the written JSON, `an_enriched_half…` count 0≠1, and the
prompt payload test on the untaught system prompt. Each guard that RED
could not reach was then mutated, watched fail, and reverted; suites green
after every revert:

- **Answer diverted into the name** (`apply_answers` writing
  `layer.name`): `a_description_answer_lands_in_the_description_and_never_
  in_the_name` failed — `left: "Files under src/", right: "Owns the
  application's whole runtime."` — the description stayed mechanical while
  the name was hijacked.
- **Enriched description overwritten** (provenance guard dropped in
  `apply_answers`): `an_enriched_half_of_a_layer_is_not_reoffered_while_
  the_other_half_is` failed — count `2 ≠ 1`, "MUST NOT APPLY" landed.
- **Blank answer accepted** (`answers.get` in place of `answered`):
  `blank_semantic_answers_keep_the_mechanical_labels` failed — `1 ≠ 0`,
  "blank answers must not count as enrichment".
- **Enriched description re-offered** (`collect_slots` filter removed):
  `enriched_semantic_slots_are_not_reoffered` failed — "an enriched
  semantic slot was re-offered: layer-description:src".
- **Stale prose surviving a membership change** (`reattach` ignoring the
  hash): `a_changed_membership_expires_the_description_exactly_as_it_
  expires_the_name` failed — `"Purchased prose about src." ≠ "Files under
  src/"` — and the integration test failed at the same stage on the same
  words.
- **Pre-07 store discarded** (`#[serde(default)]` removed from
  `layer_descriptions`): `a_store_written_before_descriptions_existed_
  keeps_reattaching_what_it_holds` failed — the whole store stopped
  deserializing and even the node summary reverted to mechanical, which is
  precisely the bill the no-bump decision refuses to send.
- **A new hashing rule** (`save_store` hashing the description text):
  `a_purchased_description_is_stored_on_the_hash_the_name_uses_and_
  reattaches` failed — "the description must ride the name's derivation
  hash", quoting both hashes.
- **Slot payload grown** (a `member_paths` field added):
  `a_layer_description_slot_carries_exactly_the_documented_fields` failed
  naming the extra field.

## Built (2026-08-14)

`crates/codeatlas/src/enrich.rs`: fifth slot kind
`EnrichmentSlot::LayerDescription(LayerSlot)` with key prefix
`layer-description:`; `collect_slots` offers it per structural-provenance
description; `apply_answers` lands answers as
`description = { text, provenance: llm }` and never touches the name;
`AnnotationStore.layer_descriptions` saved from `llm`-provenance
descriptions on the layer's existing membership hash and re-attached by
it; the version-2 doc comment records the second no-bump instance.
`enrich/prompt.rs`: the system prompt teaches the kind; the payload
carries `directory` + `member_files`, pinned exactly. `Plan`/`--dry-run`
count the slots by construction (asserted: one structural layer = two
slots). Docs: `docs/SECURITY.md`'s store paragraph now names layer
descriptions; `CONTEXT.md`'s Enrichment entry gains "descriptions". No
dashboard files touched — ticket 06 landed the rendering and the export
count. The committed `.codeatlas/annotations.json` is byte-identical
(sha256 `a94875be…ca28e26` before and after).

Suites (2026-08-14): cargo test 267/267 (was 258 + 9 new: 7 enrich unit,
1 prompt unit, 1 integration); sealed 228/228 (220 + 8 — the prompt test
is compiled out there); agent-cli 255/255; fmt clean; clippy ×3 clean
(default, sealed, agent-cli).

**Residual from the crosscheck (2026-08-14), accepted without send-back:**
the no-bump decision's record covered reading, not re-enriching — a pre-07
binary that runs `--enrich` against a newer store rebuilds it wholesale
(`save_store`'s documented self-pruning) and silently drops the
`layer_descriptions` section, discarding purchased prose. Bounded: the
store is a committed artifact (ADR-0007), so the drop is visible in the
git diff a reviewer reads before merging, and the next `--enrich` on a
current binary re-buys eight slots at most. Stated here so the consequence
is a decision, not a surprise. The reviewer also noted SECURITY.md's
"What a model receives" Enforced-by list omits the new payload-pin test —
folded into the next pass over that document (ticket 12 or 13, whichever
lands first, both touch SECURITY.md).
