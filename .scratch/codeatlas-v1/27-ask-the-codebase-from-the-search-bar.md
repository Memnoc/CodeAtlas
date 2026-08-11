# Ticket 27 — ask the codebase a question from the search bar

**Status:** blocked
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md — **needs an ADR and a story first**
**Story:** none yet. See Notes.
**Blocks:** none
**Blocked by:** an ADR, then a spec story
**Scope:** V1 — decided 2026-08-11, against a recommendation to defer it

## Problem

Requested 2026-08-11: when a map is enriched, let the search bar take a
*question* rather than a name — "how does this function do X", "where does Y
business logic happen" — and answer it from the map.

The appeal is obvious: the map already holds the structure and, once
enriched, the prose. A reader who does not know the codebase does not know
what to search *for*, which is precisely when a name-matching search is least
useful.

## Why this is blocked rather than ready

It is not a dashboard feature with an LLM bolted on; it changes what the
product *is*, in three ways that the existing ADRs answer differently:

1. **The dashboard would have to reach the network.** Today it consumes only
   local files and makes zero external requests — that is story 17, and
   ADR-0006 makes it a compile-time guarantee with an egress suite behind it.
   A question box that calls an API breaks that guarantee for the dashboard,
   and the share artifact inherits the same renderer.
2. **Enrichment today is a batch that fills typed slots** (ADR-0004): bounded
   prompts, structured output, no free text, no output-repair machinery. A
   question is unbounded input and its answer is prose. That is a second,
   differently-shaped path to the model.
3. **It needs credentials at view time, not scan time**, which is the exact
   friction the enrichment-credential work exists to remove. A feature that
   reintroduces per-reader credentials wants deciding alongside that, not
   after it.

None of these is a reason not to build it. They are reasons the decision
belongs in an `/adr` rather than in a ticket.

## Questions the ADR has to settle

- Does the *dashboard* call the model, or does the CLI serving it do so —
  keeping egress in the binary where ADR-0006's guarantee already lives?
- What does the sealed build do here? Refuse, presumably — and the egress
  suite must prove it.
- What does the share artifact do? It is a single file with no server; either
  the feature is absent there or the artifact gains a network path, which
  would be a much larger change to what "share" means.
- What does the model see? Bounded slots are what keeps ADR-0004's promise
  that the model never receives the whole serialized graph. A free question
  needs a retrieval step with a stated bound.
- Is it answerable from the map alone, or does it need file contents? Those
  are very different privacy postures.

## Notes

**Needs a spec story as well as an ADR.** Story 17 currently says the
dashboard makes zero external requests; this feature contradicts that story
as written, so the story changes or a new one carves out an exception. Either
way `/harden` needs something to walk.

Filed now so the idea is not lost; deliberately not `ready`.
