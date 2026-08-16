// The opened file, beside the map it was opened from (ticket 02 of V3,
// ADR-0013). It renders one [`SourceState`] and nothing else — the request,
// the state machine and the Escape that closes this belong to callers, so
// the panel itself is as available to a test as it is to a reader.
//
// Three honesties this panel owes, from the ticket:
//
// - **A symbol lands read, not hunted for.** The lit range is the symbol's
//   own `range` from the contract, every line inside it is marked, and the
//   first lit line is scrolled to on arrival — under the reader's motion
//   preference, through the same `scrollBehaviour()` every other programmed
//   move reads.
// - **A cut is announced.** The server truncates past its cap rather than
//   refusing (ADR-0013); the envelope's flag becomes a visible notice, so a
//   file that ends mid-thought reads as cut, never as complete.
// - **Never an empty panel.** A file the map names but the disk no longer
//   holds arrives as the server's own 404 message, shown as prose — a
//   deleted file is news, not a blank column.
//
// The lines are the server's highlighted HTML (ticket 03): token spans with
// `hl-…` classes the stylesheet colours, entities for anything that was
// markup in the source. Injecting it is safe by the same trust that runs
// the dashboard at all — this HTML comes from the binary that served the
// page itself, and its escaping is proven on its side of the wire (a
// `<script>` in source arrives as entities, `src/highlight.rs`). The server
// closes every span before each newline, so splitting on `\n` still yields
// one self-contained element per line — the lit/landing mechanics of
// ticket 02 carry over unchanged. No highlight library, no parsing here:
// the dashboard's whole contribution is its stylesheet (ADR-0013).
import { useEffect, useRef } from "react";
import { scrollBehaviour } from "./motion.js";
import type { SourceState } from "./source.js";

export function SourcePanel({
  state,
  onDismiss,
}: {
  state: SourceState;
  onDismiss: () => void;
}) {
  // The first lit line, or null when a file opened at its top. Scrolled to
  // when the source arrives — an effect, because the line exists only after
  // the open render; guarded, because jsdom (and nothing else this runs on)
  // lacks `scrollIntoView`, the same guard `Walkthrough.tsx` wears.
  const landing = useRef<HTMLElement | null>(null);
  useEffect(() => {
    if (state.phase !== "open") {
      return;
    }
    const line = landing.current;
    if (line !== null && typeof line.scrollIntoView === "function") {
      // Centred rather than nearest: a function's first line at the very
      // bottom edge of the column shows the signature and none of the body.
      line.scrollIntoView({ block: "center", behavior: scrollBehaviour() });
    }
  }, [state]);

  if (state.phase === "closed") {
    return null;
  }

  const lit = state.phase === "failed" ? null : state.lit;

  return (
    // Marked so the walkthrough accounts for the close control, and
    // deliberately without a step of its own: the column exists only after
    // the reader opened something — see `WALKTHROUGH_TRANSIENT`.
    <section className="source" aria-label="Source" data-walkthrough="source">
      <div className="source-head">
        <p className="source-path">
          {state.phase === "open" ? state.envelope.path : state.path}
          {lit !== null && (
            <span className="source-lit-note">
              {" "}
              · lines {lit.start_line}–{lit.end_line}
            </span>
          )}
        </p>
        {state.phase === "open" && (
          // What the server highlighted as — or its stated "plain text"
          // fallback, which is how a reader tells "uncovered" from
          // "broken" without opening the network tab.
          <span className="source-language">{state.envelope.language}</span>
        )}
        <button
          type="button"
          className="source-dismiss"
          onClick={onDismiss}
          aria-label="Close the source"
          title="Close the source and give its width back to the map (Escape)"
        >
          <span aria-hidden="true">×</span>
        </button>
      </div>

      {state.phase === "opening" && (
        <p className="source-status" role="status">
          Opening {state.path}…
        </p>
      )}

      {state.phase === "failed" && (
        <p className="source-error" role="alert">
          Could not open: {state.message}
        </p>
      )}

      {state.phase === "open" && (
        <>
          {state.envelope.truncated && (
            <p className="source-truncated" role="note">
              Truncated: this file is larger than the server sends whole, so
              this is its beginning — the rest is on disk, not here.
            </p>
          )}
          <pre className="source-code">
            <code>
              {state.envelope.html.split("\n").map((markup, i) => {
                const line = i + 1;
                const isLit =
                  lit !== null &&
                  line >= lit.start_line &&
                  line <= lit.end_line;
                return (
                  <span
                    key={line}
                    className={
                      isLit ? "source-line source-line-lit" : "source-line"
                    }
                    data-line={line}
                    // The trailing newline stands in for the one split off,
                    // so a copied selection keeps its line breaks — exactly
                    // the `{"\n"}` the plain-text lap appended as a child.
                    dangerouslySetInnerHTML={{ __html: `${markup}\n` }}
                    {...(isLit && line === lit.start_line
                      ? {
                          ref: (el: HTMLElement | null) => {
                            landing.current = el;
                          },
                        }
                      : {})}
                  />
                );
              })}
            </code>
          </pre>
        </>
      )}
    </section>
  );
}
