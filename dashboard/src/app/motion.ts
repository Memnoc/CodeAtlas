// Whether to animate, asked in one place.
//
// `prefers-reduced-motion` is honoured twice over in this dashboard and the
// two halves have to agree: the stylesheet turns its transitions off inside
// `@media (prefers-reduced-motion: reduce)`, and the movements CSS cannot
// express — the canvas viewport's easing, bringing a walkthrough step into
// view — read the same query from here. A second, differently-worded query
// somewhere else is how a reader ends up with half their setting respected.
//
// Read at the moment of the move rather than cached, so changing the system
// setting takes effect without a reload; and wrapped, because a `file://`
// artifact and jsdom can both be missing `matchMedia` entirely, which is not
// worth a broken dashboard over.

/** What the operating system asks for, as far as this browser will say. Not
 * exported: callers want a duration or a behaviour, and the two below are the
 * only shapes anything in this dashboard has ever needed. */
function prefersReducedMotion(): boolean {
  try {
    return (
      globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches ===
      true
    );
  } catch {
    return false;
  }
}

/** `ms`, or zero when the reader has asked their system for less motion. */
export function motionDuration(ms: number): number {
  return prefersReducedMotion() ? 0 : ms;
}

/** How a scroll should be performed, for the APIs that take the choice as a
 * word rather than as a duration. */
export function scrollBehaviour(): ScrollBehavior {
  return prefersReducedMotion() ? "auto" : "smooth";
}
