# Ticket 15 — the FILES tab forgets what you were doing

**Status:** ready
**Spec:** docs/specs/2026-08-13-codeatlas-v2.md
**Story:** 22 — the FILES tab's filter and fold state survive tab switches and
sidebar folds, so the panel does not forget what I was doing
**Blocks:** none
**Blocked by:** none — can start immediately

## Problem

The FILES tab owns its filter text and its expansion state, so both die when
the panel unmounts. Switching to another tab and back clears a filter the
reader typed; folding the sidebar to see more canvas and unfolding it does the
same. The reader is punished for looking at anything else.

## What to build

The filter and fold state outlive the tab.

## Acceptance criteria

- [ ] Filter text survives switching away to another tab and back.
- [ ] Expansion/fold state within the panel survives the same round trip.
- [ ] Both survive folding and unfolding the sidebar.
- [ ] The state has one owner — the component that owns the tabs — rather
      than a copy per tab that has to be kept in step.
- [ ] Existing FILES-tab behaviour is otherwise unchanged: opening a node,
      expanding symbols, and the filter's own semantics.
- [ ] jsdom tests cover the tab switch and the sidebar fold, each proven able
      to fail.

## Notes

This is a lifting-state prefactor with a user-visible outcome, which is why it
is a ticket rather than a note. Keep it to hoisting: the moment it turns into
a state-management abstraction for the whole sidebar, it has outgrown the
story it serves.
