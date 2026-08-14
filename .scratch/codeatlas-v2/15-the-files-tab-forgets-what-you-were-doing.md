# Ticket 15 — the FILES tab forgets what you were doing

**Status:** done
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

- [x] Filter text survives switching away to another tab and back.
- [x] Expansion/fold state within the panel survives the same round trip.
- [x] Both survive folding and unfolding the sidebar.
- [x] The state has one owner — the component that owns the tabs — rather
      than a copy per tab that has to be kept in step.
- [x] Existing FILES-tab behaviour is otherwise unchanged: opening a node,
      expanding symbols, and the filter's own semantics.
- [x] jsdom tests cover the tab switch and the sidebar fold, each proven able
      to fail.

## Record (2026-08-14)

**Where the state went.** `openId`, `filter` and `expanded` left
`FilesPanel` (which no longer imports `useState`) and became
`filesOpenId` / `filesFilter` / `filesExpanded` in `MapExplorer`, declared
beside the `tab` state whose lifetime they now share. `MapExplorer` is the
owner of the tabs — it holds `tab`, renders the tablist, and is the
component that unmounts the panel both ways (the `tab === "files"` ternary
and the `chrome.panel` fold that unmounts the whole aside). Plain hoisted
`useState`, props down (`openId`/`onOpenId`, `filter`/`onFilter`,
`expanded`/`onExpanded`); no context, store or reducer, per this ticket's
own fence. One owner, one copy — the INFO tab has no copy to keep in step.

**Guards proven able to fail** — red run 2026-08-14 10:28 against the
unmodified code, before the hoist, all four failing at the guard assertion
itself (`4 failed | 9 passed`):

- "keeps the filter across a tab round trip":
  `expect(element).toHaveValue("src")` — received `""`.
- "keeps the folds and the open symbol list across a tab round trip":
  `expected [] to deeply equal [ Array(3) ]`.
- "keeps the filter across folding and unfolding the sidebar":
  `expect(element).toHaveValue("src")` — received `""`.
- "keeps the folds and the open symbol list across folding and unfolding
  the sidebar": `expected [] to deeply equal [ Array(3) ]`.

Green run after the hoist, same date: `tests/files-panel.test.tsx`
13 passed (13). The one-owner criterion is structural, shown by the diff
rather than a test: `FilesPanel` holds no state to lose.

**Behaviour otherwise unchanged.** The nine pre-existing files-panel tests
pass byte-for-byte unmodified — they drive through `<MapExplorer/>`, so
the hoist needed no mount-point edits anywhere. `fold.test.tsx` untouched.

**Suites (2026-08-14).** `npm test` in `dashboard/`: 285 passed (19 files).
`npm run typecheck`: clean. `cargo test`: 270 passed across 15 suites,
0 failed, including `the_share_artifact_stays_under_its_ceiling` — the
artifact weighed 1,560,526 bytes against the 2,097,152-byte ceiling.

## Notes

This is a lifting-state prefactor with a user-visible outcome, which is why it
is a ticket rather than a note. Keep it to hoisting: the moment it turns into
a state-management abstraction for the whole sidebar, it has outgrown the
story it serves.
