// The header's theme switch: Rosé Pine Dawn ⇄ Moon.
import { useLayoutEffect, useState } from "react";
import {
  applyTheme,
  initialTheme,
  otherTheme,
  persistTheme,
  type Theme,
} from "./theme.js";

const NAME: Record<Theme, string> = { dawn: "Dawn", moon: "Moon" };
const GLYPH: Record<Theme, string> = { dawn: "☀", moon: "☾" };

export function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(initialTheme);

  // Before paint, not after: an effect that runs late would show one theme
  // and then swap to the other in front of the reader.
  useLayoutEffect(() => {
    applyTheme(theme);
  }, [theme]);

  const next = otherTheme(theme);

  return (
    <button
      type="button"
      className="theme-toggle"
      aria-label={`Theme: Rosé Pine ${NAME[theme]}. Switch to ${NAME[next]}.`}
      title={`Switch to Rosé Pine ${NAME[next]}`}
      onClick={() => {
        setTheme(next);
        persistTheme(next);
      }}
    >
      <span className="theme-toggle-glyph" aria-hidden="true">
        {GLYPH[theme]}
      </span>
      {NAME[theme]}
    </button>
  );
}
