// Where focus goes when a layer closes, decided in one place.
//
// Two things in this dashboard open over the page and take focus with them —
// the share/export panel and the walkthrough — and both had the same effect
// written out, because the rule is not obvious enough to be re-derived
// correctly twice. A reader who tabbed into the layer and closed it would
// otherwise land on `<body>`, and their next Tab would restart from the top
// of the document.
import { useEffect, useRef } from "react";

/**
 * A ref for the control that opens `open`'s layer, which regains focus when
 * the layer closes and took focus with it.
 *
 * `<body>` is the signal, and it is a precise one: a browser moves focus
 * there when the focused element is removed, so finding it there just after
 * the layer closed means the layer took focus with it. Any other value means
 * the reader moved focus themselves — clicked the search box, pressed the
 * toggle again — and the kind thing is to leave them where they are.
 *
 * The `wasOpen` guard is not decoration: without it this runs on mount, where
 * `open` is false and focus is legitimately on `<body>`, and the layer would
 * seize focus from the page as it loads.
 */
export function useFocusReturn<T extends HTMLElement>(open: boolean) {
  const control = useRef<T | null>(null);
  const wasOpen = useRef(false);

  useEffect(() => {
    if (open) {
      wasOpen.current = true;
      return;
    }
    if (wasOpen.current && document.activeElement === document.body) {
      control.current?.focus();
    }
    wasOpen.current = false;
  }, [open]);

  return control;
}
