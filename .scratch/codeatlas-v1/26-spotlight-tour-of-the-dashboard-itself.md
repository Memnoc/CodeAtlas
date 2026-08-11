# Ticket 26 — a spotlight tour of the dashboard itself

**Status:** ready
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md — **needs a new story first**
**Story:** none yet. See Notes.
**Blocks:** none
**Blocked by:** a spec story covering it

## Problem

The dashboard has grown a lot of controls — Overview/Learn, Domain/Structural,
region chips, the path finder, export, the tour, the narrative panel — and
nothing explains any of them. A first-time reader has to infer what each one
is for by clicking it.

Requested 2026-08-11: the familiar product-tour experience — a button that
starts a walkthrough, and at each step one part of the UI is highlighted while
the rest dims, with a short explanation of what that part does.

Note this is a tour of **the application**, which is a different thing from
the existing guided tour of **the codebase** (story 6). Two things called
"tour" in one product will confuse both the code and the reader, so naming is
part of the work.

## Acceptance criteria

- [ ] A control starts the walkthrough; it is not shown automatically on
      first load without a way to decline it.
- [ ] Each step highlights one real element in the live UI and dims the rest,
      rather than showing a picture of the UI.
- [ ] Steps carry forward, back, and dismiss. Dismiss is reachable at every
      step and by `Escape`.
- [ ] Focus moves to the step content so a screen reader announces it, and
      the dimmed region is inert to the keyboard while the walkthrough runs.
- [ ] `prefers-reduced-motion` is honoured, as everywhere else in this UI.
- [ ] The highlight follows the element if the layout reflows — a spotlight
      cut at a stale rectangle is worse than no spotlight.
- [ ] It does not collide with the codebase tour: starting one does not leave
      the other half-running, and the two are named distinctly in the UI.
- [ ] Whether it has been seen is remembered locally, and no request leaves
      the page to record it (ADR-0006).

## Notes

**This needs a spec story before it is built.** Nothing in stories 1–17
covers explaining the application to its own user; story 3 covers the
dashboard existing, and story 6 covers touring the *codebase*. `/harden`
walks the numbered story list, so a feature with no story never gets
verified. Raise it in the same `/to-spec` pass as the enrichment-credential
stories.

Worth deciding early whether the step list is hand-written or derived from
the components present, since a hand-written list silently goes stale the
next time the header changes.
