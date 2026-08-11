# Ticket 26 — a spotlight tour of the dashboard itself

**Status:** done
**Spec:** docs/specs/2026-08-09-codeatlas-v1.md
**Story:** 20 — a walkthrough that highlights each control in the live
interface and says what it does
**Blocks:** none
**Blocked by:** none — story 20 was added 2026-08-11
**Scope:** V1 — decided 2026-08-11, against a recommendation to defer it

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

- [x] A control starts the walkthrough; it is not shown automatically on
      first load without a way to decline it.
- [x] Each step highlights one real element in the live UI and dims the rest,
      rather than showing a picture of the UI. **The "real element" half is
      asserted: the running walkthrough puts `data-walkthrough-lit` on the
      live control, and the test checks that exactly one element carries it,
      that it is inside `.explorer`, and that it is not inside the dialog —
      which a picture could never satisfy. The dimming itself is one CSS
      `box-shadow`, and jsdom paints nothing; it is unwatched in the same way
      every other visual state in this UI is (spec, Notes on the passes, 1).**
- [x] Steps carry forward, back, and dismiss. Dismiss is reachable at every
      step and by `Escape`.
- [x] Focus moves to the step content so a screen reader announces it, and
      the dimmed region is inert to the keyboard while the walkthrough runs.
- [x] `prefers-reduced-motion` is honoured, as everywhere else in this UI.
      **Both halves of the existing mechanism: the spotlight's transition
      joins the stylesheet's `@media (prefers-reduced-motion: reduce)` block,
      and the one movement CSS cannot express — bringing a step's element
      into view — reads the same query through `motion.ts` and is asserted on
      the argument it passes.**
- [x] The highlight follows the element if the layout reflows — a spotlight
      cut at a stale rectangle is worse than no spotlight. **Not on zeroes:
      the test replaces `ResizeObserver` with one it can fire and drives the
      rect itself, so the two reads genuinely differ. Proven able to fail —
      see the tamper list below.**
- [x] It does not collide with the codebase tour: starting one does not leave
      the other half-running, and the two are named distinctly in the UI.
- [x] Whether it has been seen is remembered locally, and no request leaves
      the page to record it (ADR-0006).

## Notes

**Resolved 2026-08-11: this is story 20.** Nothing in stories 1–17 covered
explaining the application to its own user — story 3 covers the dashboard
existing, story 6 covers touring the *codebase* — and `/harden` walks the
numbered story list, so a feature with no story never gets verified. The
story was added in the same `/to-spec` pass as the enrichment-credential
stories. The spec also records the naming requirement: story 20's feature and
story 6's are named distinctly in the UI, and starting one must not leave the
other half-running.

Worth deciding early whether the step list is hand-written or derived from
the components present, since a hand-written list silently goes stale the
next time the header changes. Carried into the spec's Further Notes as an
open question rather than settled here.

## What the work found

**The names are "Codebase tour" and "Interface walkthrough", and the rename
went both ways.** Calling the new thing something other than a tour was never
going to be enough on its own, because the existing feature was called
"Guided tour" — a name that says a walk is guided without saying what it walks,
which is precisely the ambiguity the spec asked to remove. Story 6's panel is
now headed and labelled **Codebase tour**; story 20's control is **Walkthrough**
and its dialog is **Interface walkthrough**. The word "tour" appears nowhere in
the new feature and the word "walkthrough" nowhere in the old one, and the test
that holds that collects every button whose text matches `/tour|walkthrough/i`
with the Learn panel open and requires the set to be exactly
`{"Start tour", "Walkthrough"}` — so renaming either one back into collision
fails the suite rather than merely reading badly.

Not-half-running is a state problem rather than a naming one, and it needed the
codebase tour's step index lifted out of `TourPanel` into the explorer. A panel
holding its own index cannot be told to go back to the start, so starting the
walkthrough would have left "Step 2 of 3" sitting behind the modal. The index
is now the explorer's and `startWalkthrough` clears it, along with the search
overlay and the export menu — the cascade has one order, and two layers each
claiming to be innermost is how ticket 22's dead zone was built.

**The step list is hand-written prose walked in a list read off the page, and
the staleness objection is answered by tests rather than by derivation.** The
two halves of a step come from different places and pretending otherwise is
what makes the question look binary. Nothing can derive "Domain groups files by
the call flows the map found, Structural by the directories they live in" from
a DOM node: that sentence is knowledge about the product, and it has to be
written. But *which* steps a given page is walked through must not be written
down, because the page differs — a share artifact has no question box, a
repository with no `codeatlas diff` run has no overlay toggle — so every walk is
the declaration filtered against the elements actually present, and a reader is
never told about a control they do not have. Thirteen steps declared, eleven
walked on a plain served map.

That leaves the accusation the ticket actually makes: a hand-written list goes
stale the next time the header changes. That is a test problem, and it is
solved as one, in both directions. Every interactive control inside `.explorer`
must sit within some element carrying `data-walkthrough`, so a button added to
the top bar tomorrow fails the suite until somebody decides which band it
belongs to; and the markers present on a fully-featured render must be exactly
the ids the step list declares, so a marked band with no prose and prose naming
no band both fail too. Both were proven by tampering: a stray `<button>` in
`topbar-actions` failed the first, `data-walkthrough="unspoken"` on the
workspace failed the second. The residual gap, recorded rather than closed, is
that a control added *inside* an already-marked band — a fourth region chip, a
third right-panel tab — is covered by its band's step and does not fail
anything. That is the correct outcome for a chip and the wrong one for
something genuinely new, and no test can tell those apart.

**The Escape layer went in first, which is the opposite end from the answer
panel's.** Ticket 27 put the answer fourth, reasoning that a band the reader is
working through should yield to anything opened on top of it. A walkthrough is
not a band: while it runs the rest of the page is `inert`, so there is nothing
else Escape could sensibly mean, and any layer it did reach would be a control
whose effect the reader cannot see. It is still the explorer's one
document-level listener — the test dispatches `keydown` on `document` itself
rather than on the focused card, which a handler living inside the walkthrough
would never see, so a green assertion there is evidence about *where* the
listener is and not only that one exists.

**`inert` is the right declaration and useless as evidence.** React 19 emits it
as a proper boolean — checked, because a permanent `inert=""` would disable the
whole page — and a real browser acts on it. Neither jsdom nor `@testing-library
/user-event` implements it: `user-event`'s `utils/focus/selector.js` filters on
`disabled` and negative `tabindex` and nothing else, so a test asserting the
attribute would have passed over a fully tabbable background. The walkthrough
therefore also traps Tab inside its card, which is both the belt-and-braces
every dialog implementation carries and the thing a test can drive. Twenty-five
presses, focus checked after each — twenty-five because the chrome holds
thirteen focusable controls before the right panel, so a background that was
merely dim rather than inert would have arrived at the codebase tour's Start
button, which is the specific collision the criterion is about.

**One test passed for the wrong reason and had to be rewritten.** "Starting the
walkthrough puts the search results away" was green with `setSearchDismissed`
deleted, because a pointer press on the launcher is a press outside the search
row and the existing `pointerdown` dismissal closes the overlay anyway. The
guard proved nothing — and the hole it was hiding is real: a keyboard reader who
types a query, tabs to the control and presses Enter produces no `pointerdown`
at all. Both tests now focus the launcher and press Enter, and both fail when
the corresponding line is removed. The same is true of the export menu, which
is why it gets its own test rather than sharing one: the two cannot be open
simultaneously through a pointer, since opening either closes the other.

**The reflow criterion was verifiable here, but only by refusing the
environment's answer.** `getBoundingClientRect` returns zeroes under jsdom and
`ResizeObserver` is a no-op stub in `tests/setup.ts`, so the obvious test —
resize something, compare the spotlight's geometry before and after — compares
`0px 0px 0px 0px` with itself and passes over a component that never measures at
all. The test replaces `ResizeObserver` with one whose callbacks it can fire,
firing only those watching the element under test so React Flow's own observers
are left alone, and drives the rect from the test body: `10px 20px 100px 30px`,
then a fired reflow, then `240px 500px 320px 44px`. Both numbers are asserted,
so a component that measured once and kept the answer fails — which was checked
by building exactly that component. A second test asserts the previous step's
element stops being observed, because an observer left attached across thirteen
steps is a leak that nothing else would notice.

What the walkthrough watches is deliberately short, and it is short because of
`inert`: nothing on the page behind can move under its own steam while a modal
is over it, so the rectangle goes stale for exactly three reasons — the window
resizes, the element resizes, or something scrolls. An observer on the target,
an observer on the document element, `resize`, and capture-phase `scroll` cover
all three. This is the one place in the dashboard where a position cannot come
from the stylesheet, because it is a position on top of whatever the stylesheet
did.

**Reduced motion needed a shared home rather than a second mechanism.**
`MapExplorer` held a private `motionDuration` reading `matchMedia`, and the
stylesheet has its own `@media` block; a third copy inside the walkthrough is
how a reader ends up with half their setting respected. The function moved to
`motion.ts`, gained `prefersReducedMotion` and `scrollBehaviour` beside it, and
the spotlight's transition joined the existing media block rather than starting
a new one. The CSS half is unwatched here as everywhere — jsdom runs no
transitions — but the scroll half is genuinely asserted, on the `behavior`
argument, and it is the half that matters for a step below the fold.

**Twenty-three tests, and each guard was broken on purpose.** Seventeen tampers
were run; every one tripped a test named for what it broke. Measuring the rect
once; dropping the focus trap; not moving focus to the card; leaving the
previous element lit; never disconnecting the observer; always easing the
scroll; removing the last step's way out; deleting the Escape layer; moving the
Escape layer below the step back; not resetting the codebase tour; leaving the
search overlay open; leaving the export menu open; walking the declaration
instead of the page; never recording the offer; a stray header button; a marked
band with no prose; renaming the control back to "Tour".

Two tests were rewritten before that sweep, for two different reasons. The
search-overlay one did not trip at all, described above. The naming one would
have tripped, but was vacuous in the other direction: it asserted that no
button was named `/tour/i` while standing in Overview, where the codebase tour
is not rendered at all, so it passed over a screen with nothing to be confused
with. It now opens Learn first and requires both names to be present and
distinct, which is the property the criterion is about.

**Deliberately not done.** The answer panel is not marked, and the walkthrough
says nothing about it: it exists only after a question has been asked, so a step
about it would be a spotlight on an absent element every time. It is reached
from the search band, which is explained. The walkthrough is present in a share
artifact and this is deliberate — its recipient is exactly the reader story 20
is about, it makes no request, and it silently drops the two steps whose
controls that page does not have. `docs/SECURITY.md` needed no amendment:
nothing in it claims anything about browser-local state, the document's
disclosure section is about what a *scan* writes into the repository, and the
"what a connection can be told" enumeration is unchanged because no route was
added. `README.md`'s self-scan figure was stale before this ticket and is
refreshed — 1224 nodes and 2264 edges in about 150 ms, measured three times on
a release build — because leaving a number that had drifted in both directions
is the sort of quietly false sentence the last three tickets each shipped.
