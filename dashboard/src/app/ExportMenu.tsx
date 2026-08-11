// The two ways a map leaves the dashboard, named and explained side by side.
//
// They are not variants of one thing. Export writes the graph as JSON against
// the published contract — a machine format, whose worth is that another tool
// can read it. `codeatlas share` writes the artifact a *person* can open: one
// self-contained page, no server, nothing installed, LLM prose redacted
// against the allowlist (ADR-0006). Until this menu existed the second route
// appeared only in a `title` tooltip, so the visible button was the one most
// readers did not want — and on an enriched map it is also the unredacted one.
//
// The dashboard cannot run `codeatlas share` and must not pretend to: it has
// no shell and makes no requests. It prints the command and copies it.
import { useEffect, useRef, useState } from "react";
import type { KnowledgeGraph } from "../index.js";
import { downloadMap } from "./export.js";
import { useFocusReturn } from "./focus.js";

/** What the reader has to type. Shown as text, copied on request, never run.
 * Bare, because the subcommand's path argument defaults to the working
 * directory — which is why the panel says where to run it from. */
const SHARE_COMMAND = "codeatlas share";

/** Where `codeatlas share` puts the artifact. Naming it is half the answer to
 * "what do I do with this": a command whose output you cannot find has not
 * told you anything. */
const SHARE_OUTPUT = ".codeatlas/share.html";

/** How many prose slots in this map an LLM wrote — exactly the set the share
 * allowlist redacts (`Node.summary`, `Layer.name`, `DomainFlow.name`,
 * `TourStep.label`), which is what makes it the right number to warn with. */
function enrichedSlots(map: KnowledgeGraph): number {
  return [map.nodes, map.layers ?? [], map.domain_flows ?? [], map.tour ?? []]
    .flat()
    .filter((slot) => slot.provenance === "llm").length;
}

type CopyState = "idle" | "copied" | "failed";

const COPY_NOTE: Record<CopyState, string> = {
  idle: "",
  copied: "Copied.",
  failed: "Could not reach the clipboard — copy the command above by hand.",
};

export function ExportMenu({
  map,
  shared,
  open,
  onOpen,
}: {
  map: KnowledgeGraph;
  /** True when this document *is* a share artifact: its payload is already
   * redacted, and its reader has nothing installed to run a CLI with. */
  shared: boolean;
  open: boolean;
  onOpen: (open: boolean) => void;
}) {
  const wrap = useRef<HTMLDivElement | null>(null);
  // Closing the panel must not drop focus on the floor. The rule about where
  // it goes is shared with the walkthrough — see `focus.ts`.
  const toggle = useFocusReturn<HTMLButtonElement>(open);
  const [copied, setCopied] = useState<CopyState>("idle");

  // Same shape as the search overlay's dismissal (ticket 22): `pointerdown`
  // so the menu is gone before the press lands, consuming nothing so the
  // press still reaches whatever it was aimed at. The ref wraps the toggle
  // too, or opening the menu would immediately close it again.
  useEffect(() => {
    if (!open) {
      return;
    }
    const onPointerDown = (event: PointerEvent) => {
      const target = event.target;
      if (target instanceof Node && wrap.current?.contains(target)) {
        return;
      }
      onOpen(false);
    };
    document.addEventListener("pointerdown", onPointerDown);
    return () => document.removeEventListener("pointerdown", onPointerDown);
  }, [open, onOpen]);

  // "Copied." is about the press that just happened, so it does not survive
  // the menu closing — and the menu closes three ways (the toggle, a click
  // outside, Escape from the explorer's cascade), only one of which this
  // component sees. Keying on `open` catches all three.
  useEffect(() => {
    if (!open) {
      setCopied("idle");
    }
  }, [open]);

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(SHARE_COMMAND);
      setCopied("copied");
    } catch {
      // Insecure origin, denied permission, or no clipboard at all. The
      // command is on screen regardless, so this degrades to reading it.
      setCopied("failed");
    }
  };

  const enriched = enrichedSlots(map);

  return (
    <div className="export-menu-wrap" ref={wrap} data-walkthrough="export">
      <button
        type="button"
        ref={toggle}
        className={`topbar-button${open ? " topbar-button-on" : ""}`}
        aria-expanded={open}
        onClick={() => onOpen(!open)}
      >
        {/* A share artifact has only one route out, so naming the other one
            in the chrome would advertise something absent from the panel. */}
        {shared ? "Export" : "Share / Export"}
      </button>

      {open && (
        // Escape is handled by the explorer's one cascade rather than here,
        // so the layers close innermost-first (ticket 22). Focus on the way
        // back out is this component's job — see the effect above.
        <div
          className="export-menu"
          role="group"
          aria-label="Share or export this map"
        >
          {!shared && (
            <section className="export-route">
              <h2>Share a page</h2>
              <p>
                One self-contained HTML file at <code>{SHARE_OUTPUT}</code>. It
                opens by double-click in any browser, with no server and
                nothing installed. LLM-written prose is redacted from it, and
                the page states what it removed.
              </p>
              {/* The subcommand's path argument defaults to the working
                  directory, and the dashboard has no way to know where the
                  reader's shell is — so the instruction carries the bit the
                  bare command cannot. */}
              <p>Run this from the repository root:</p>
              <div className="export-command">
                <code>{SHARE_COMMAND}</code>
                <button
                  type="button"
                  className="topbar-button"
                  onClick={() => void copy()}
                >
                  Copy
                </button>
              </div>
              <p className="export-note" role="status">
                {COPY_NOTE[copied]}
              </p>
            </section>
          )}

          <section className="export-route">
            <h2>Download the data</h2>
            <p>
              The map as JSON, conforming to map contract v{map.version} — for
              scripts, diffs, and anything else that reads the published
              format.
            </p>
            {/* Keyed on `shared`, not on provenance alone: the allowlist
                keeps `llm` provenance and replaces the prose, so a share
                artifact carries enriched nodes with nothing left to leak. */}
            {!shared && enriched > 0 && (
              <p className="export-warning">
                Not redacted: this file carries the map&rsquo;s {enriched}{" "}
                LLM-written prose {enriched === 1 ? "field" : "fields"} — node
                summaries, layer and flow names, tour narration. The shared
                page above removes them.
              </p>
            )}
            <button
              type="button"
              className="topbar-button"
              onClick={() => downloadMap(map)}
            >
              Download JSON
            </button>
          </section>
        </div>
      )}
    </div>
  );
}
