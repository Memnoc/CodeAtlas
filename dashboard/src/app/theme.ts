// The three themes the dashboard ships: Rosé Pine (the main variant, and
// the dark default), Rosé Pine Moon (the softer dark, an explicit choice),
// and Rosé Pine Dawn (light). The palettes themselves live in styles.css;
// this module only decides which one is on and remembers the reader's
// choice.
//
// Every browser API touched here is optional. A share artifact is opened by
// double-click from `file://`, where `localStorage` is an opaque origin in
// some browsers and throws `SecurityError` on access rather than returning
// null, and `matchMedia` is absent under jsdom. Neither is worth a broken
// dashboard over: both are wrapped, and the theme falls back to the one the
// stylesheet already applies without any JavaScript at all.

/** The `data-theme` values styles.css knows about. */
export type Theme = "dawn" | "main" | "moon";

const STORAGE_KEY = "codeatlas-theme";
const ATTRIBUTE = "data-theme";

/**
 * What bare `:root` resolves to in styles.css. Also the answer when the
 * environment cannot say what the reader prefers — guessing dark there would
 * override the stylesheet with a value it never chose.
 */
const FALLBACK: Theme = "dawn";

function isTheme(value: unknown): value is Theme {
  return value === "dawn" || value === "main" || value === "moon";
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

/**
 * What the operating system asks for, as far as this browser will say. A
 * dark preference means the main variant — Moon is only ever a deliberate
 * choice, matching the stylesheet's own media query.
 */
export function systemTheme(): Theme {
  try {
    return globalThis.matchMedia?.("(prefers-color-scheme: dark)").matches ===
      true
      ? "main"
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

/** The cycle the header button walks: Dawn → Rosé Pine → Moon → Dawn. */
export function nextTheme(theme: Theme): Theme {
  switch (theme) {
    case "dawn":
      return "main";
    case "main":
      return "moon";
    case "moon":
      return "dawn";
  }
}
