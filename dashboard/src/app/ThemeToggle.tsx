// The header's theme switch: a cycle through Rosé Pine Dawn → Rosé Pine →
// Rosé Pine Moon. The main variant's full name is just "Rosé Pine" — the
// palette the others are variants of.
import { useLayoutEffect, useState } from "react";
import {
  applyTheme,
  initialTheme,
  nextTheme,
  persistTheme,
  type Theme,
} from "./theme.js";

const GLYPH: Record<Theme, string> = { dawn: "☀", main: "✦", moon: "☾" };

function label(theme: Theme): string {
  switch (theme) {
    case "dawn":
      return "Rosé Pine Dawn";
    case "main":
      return "Rosé Pine";
    case "moon":
      return "Rosé Pine Moon";
  }
}

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(initialTheme);

  // Before paint, not after: an effect that runs late would show one theme
  // and then swap to the other in front of the reader.
  useLayoutEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const next = nextTheme(theme);

  return (
    <button
      type="button"
      className="theme-toggle"
      data-walkthrough="theme"
      aria-label={`Theme: ${label(theme)}. Switch to ${label(next)}.`}
      title={`Switch to ${label(next)}`}
      onClick={() => {
        setTheme(next);
        persistTheme(next);
      }}
    >
      <span className="theme-toggle-glyph" aria-hidden="true">
        {GLYPH[theme]}
      </span>
      {label(theme)}
    </button>
  );
}
