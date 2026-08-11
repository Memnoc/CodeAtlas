# Ticket 28 — Export offers a format without saying what it is for

**Status:** ready
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 8 — export a single self-contained redacted HTML file to share with
someone who has nothing installed
**Blocks:** none
**Blocked by:** none

## Problem

There are two ways a map leaves the dashboard, and the UI offers only the one
most readers do not want.

**Export** downloads the raw knowledge graph as JSON. It is a *machine*
format: its value is that it conforms to the published versioned contract, so
another tool, a script, or a diff can consume it. Handed to a person, it is a
few hundred kilobytes of JSON.

**`codeatlas share`** writes the thing a person wants — one self-contained
HTML file, the same renderer inlined, opens by double-click with no server and
nothing installed, LLM prose redacted against an allowlist over the contract,
and the artifact states what it removed. That is story 8, and the UI never
mentions it outside a `title` tooltip on the Export button.

Asked directly on 2026-08-11, having used the built dashboard: *"I see it
exports a json file, what do I do with it?"* A tooltip is hover-only, absent
on touch, and unreadable by anyone who did not already suspect there was
something to hover over. The question is the evidence that it does not work as
an explanation.

There is also a real hazard the UI does not state: **the JSON is not
redacted.** On an enriched map it carries every generated summary. The share
artifact redacts precisely that, so a reader who reaches for Export because it
is the visible button gets the unredacted one by default.

## Acceptance criteria

- [ ] The share route is visible in the UI as something other than a tooltip,
      and says plainly that it produces a self-contained page needing nothing
      installed.
- [ ] Export says what it is for — the map as data, against the published
      contract — rather than only its file format.
- [ ] The UI states that the exported JSON is unredacted whenever the map
      being viewed carries `llm` provenance, and says nothing about redaction
      when there is no enriched prose to redact.
- [ ] Viewing a share artifact, which cannot run a CLI, does not advertise a
      command the reader has no way to run.
- [ ] The dashboard still runs `codeatlas share` for nobody: it prints or
      copies the command, it does not attempt to execute anything (ADR-0006 —
      the dashboard makes zero external requests and has no shell).
- [ ] No change to what either route emits. This ticket is about what the UI
      says, not what it produces.

## Notes

Deliberately not in scope: making the dashboard *generate* the share
artifact. That would move ADR-0006's allowlist redaction into TypeScript, and
the whole reason `export.ts` does not redact today is that two copies of one
security policy in two languages means the copy that drifts is the one that
leaks. The CLI stays the only thing that writes a share artifact.
