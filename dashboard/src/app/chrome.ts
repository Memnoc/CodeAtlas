// What the reader has folded away to get more canvas.
//
// The map is the thing worth looking at; everything around it is chrome that
// earns its pixels only while it is being used. The side panel is 360px of a
// window that is often 1200, and the region chips wrap to two or three rows on
// a repository with many of them — folded, both of those go to the canvas.
//
// Two rules shape what is here:
//
// - **Folded means unmounted, not hidden.** The interface walkthrough resolves
//   its steps against the elements actually on the page
//   (`resolveWalkthroughSteps`), so a panel that is merely `display: none`
//   would still be found and would then be spotlighted as a hole of zero size.
//   Unmounting makes the walkthrough skip the step, which is what it already
//   does for every control a given page does not have.
// - **Nothing folds away the only way to do something.** The diff overlay
//   toggle lives among the region chips but is not one, so it stays when they
//   go; the search field and the top bar do not fold at all. Folding is for
//   things the reader can see again by unfolding, never for reach.

/** Where the folded state is kept between visits. */
export const CHROME_KEY = "codeatlas-chrome";

/** The two foldable parts of the frame, and whether each is folded away. */
export type Chrome = {
  /** The side panel: tabs, path finder, detail, and whichever of Info, Files
   * or the codebase tour the current mode shows. */
  panel: boolean;
  /** The region chips. The row itself stays — it holds the count, the
   * unfold control and the diff overlay toggle. */
  chips: boolean;
};

export const CHROME_OPEN: Chrome = { panel: false, chips: false };

/**
 * What was folded last time. Anything unreadable, unparseable or the wrong
 * shape reads as nothing folded — the interface a first visit gets, which is
 * the only safe default: a reader who cannot see the panel and does not know
 * it exists has no way to ask for it back.
 */
export function readChrome(): Chrome {
  try {
    const raw = globalThis.localStorage?.getItem(CHROME_KEY);
    if (typeof raw !== "string") {
      return CHROME_OPEN;
    }
    const parsed: unknown = JSON.parse(raw);
    if (typeof parsed !== "object" || parsed === null) {
      return CHROME_OPEN;
    }
    const value = parsed as Partial<Record<keyof Chrome, unknown>>;
    return {
      panel: value.panel === true,
      chips: value.chips === true,
    };
  } catch {
    return CHROME_OPEN;
  }
}

/** Remembers the fold. A storage that refuses costs the memory, not the fold. */
export function writeChrome(chrome: Chrome): void {
  try {
    globalThis.localStorage?.setItem(CHROME_KEY, JSON.stringify(chrome));
  } catch {
    // An opaque `file://` origin — a share artifact opened by double-click —
    // throws here. It still folds; it just starts open next time.
  }
}

/** True when nothing is folded, which is what the Focus control toggles
 * against: pressing it folds everything, pressing it again restores
 * everything, and it reads as pressed while anything is folded. */
export function allOpen(chrome: Chrome): boolean {
  return !chrome.panel && !chrome.chips;
}
