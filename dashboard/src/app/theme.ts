// The two themes the dashboard ships, Rosé Pine Dawn and Moon. The palettes
// themselves live in styles.css; this module only decides which one is on and
// remembers the reader's choice.
//
// Every browser API touched here is optional. A share artifact is opened by
// double-click from `file://`, where `localStorage` is an opaque origin in
// some browsers and throws `SecurityError` on access rather than returning
// null, and `matchMedia` is absent under jsdom. Neither is worth a broken
// dashboard over: both are wrapped, and the theme falls back to the one the
// stylesheet already applies without any JavaScript at all.

/** The `data-theme` values styles.css knows about. */
export type Theme = "dawn" | "moon";

const STORAGE_KEY = "codeatlas-theme";
const ATTRIBUTE = "data-theme";

/**
 * What bare `:root` resolves to in styles.css. Also the answer when the
 * environment cannot say what the reader prefers — guessing Moon there would
 * override the stylesheet with the same value it already had, only later and
 * visibly.
 */
const FALLBACK: Theme = "dawn";

function isTheme(value: unknown): value is Theme {
  return value === "dawn" || value === "moon";
}

/** The reader's stored choice, or `null` if they have never made one. */
export function storedTheme(): Theme | null {
  try {
    const value = globalThis.localStorage?.getItem(STORAGE_KEY);
    return isTheme(value) ? value : null;
  } catch {
    return null;
  }
}

/** What the operating system asks for, as far as this browser will say. */
export function systemTheme(): Theme {
  try {
    return globalThis.matchMedia?.("(prefers-color-scheme: dark)").matches ===
      true
      ? "moon"
      : FALLBACK;
  } catch {
    return FALLBACK;
  }
}

/** The theme to open with: an explicit choice outranks the system's. */
export function initialTheme(): Theme {
  return storedTheme() ?? systemTheme();
}

/**
 * Puts `theme` on the document root, which is what the palettes key off.
 * Separate from [`persistTheme`] on purpose: the attribute is set on every
 * render, but only a deliberate toggle is worth recording.
 */
export function applyTheme(theme: Theme): void {
  document.documentElement.setAttribute(ATTRIBUTE, theme);
}

/** Records a deliberate choice. Storage refusing is not an error worth raising. */
export function persistTheme(theme: Theme): void {
  try {
    globalThis.localStorage?.setItem(STORAGE_KEY, theme);
  } catch {
    // A read-only or opaque origin still gets the theme, just not the memory.
  }
}

/** The other one. */
export function otherTheme(theme: Theme): Theme {
  return theme === "dawn" ? "moon" : "dawn";
}
