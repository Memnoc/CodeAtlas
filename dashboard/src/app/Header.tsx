// The top bar: what you are looking at, how you want to look at it, and the
// two things you can do to it.
import type { KnowledgeGraph } from "../index.js";
import { allOpen, type Chrome } from "./chrome.js";
import { ExportMenu } from "./ExportMenu.js";
import type { RegionKind } from "./regions.js";
import { SegmentedControl } from "./SegmentedControl.js";
import { ThemeToggle } from "./ThemeToggle.js";
import { WalkthroughLauncher } from "./Walkthrough.js";

/** Overview draws the map; Learn is the guided read of it — the codebase tour
 * and the call flows. Not the *interface* walkthrough, which is a control in
 * the top bar and belongs to no mode. */
export type Mode = "overview" | "learn";

/** The one word each grouping wears, quoted by both the tally and the
 * grouping control so the header can never say one thing where the switch
 * says another. The structural grouping is labelled by its unit — the layer
 * — because "structural" is the provenance kind's word in the map contract,
 * and the count beside this label is a provenance count: "STRUCTURAL ·
 * 0 structural" was two true facts colliding into a contradiction
 * (ticket 14). */
const groupingLabels: Record<RegionKind, string> = {
  domain: "Domain",
  structural: "Layer",
};

export function Header({
  map,
  mode,
  onMode,
  grouping,
  onGrouping,
  pathOpen,
  onTogglePath,
  chrome,
  onChrome,
  shared,
  exportOpen,
  onExportOpen,
  walkthroughOpen,
  onStartWalkthrough,
}: {
  map: KnowledgeGraph;
  mode: Mode;
  onMode: (mode: Mode) => void;
  grouping: RegionKind;
  onGrouping: (grouping: RegionKind) => void;
  pathOpen: boolean;
  onTogglePath: () => void;
  /** What is folded away. The panel and the chips each have a control of
   * their own; this is the one that does both at once, for a reader who wants
   * the map and nothing else. */
  chrome: Chrome;
  onChrome: (chrome: Chrome) => void;
  /** True when this document is a share artifact rather than the served
   * dashboard — see {@link ExportMenu}. */
  shared: boolean;
  /** Owned by the explorer, not by the menu, so `Escape` can close it in the
   * same cascade as everything else that opens (ticket 22). */
  exportOpen: boolean;
  onExportOpen: (open: boolean) => void;
  /** Owned by the explorer for the same reason, and for a second one: it is
   * the explorer that goes inert while the walkthrough runs. */
  walkthroughOpen: boolean;
  onStartWalkthrough: () => void;
}) {
  const enriched = map.nodes.filter((n) => n.provenance === "llm").length;
  // Learn holds the tour *and* the flows, so the hint has to speak for both:
  // saying "no tour" over a mode with 148 call flows in it would be a lie.
  const tourLength = map.tour?.length ?? 0;
  const flowCount = map.domain_flows?.length ?? 0;
  const learnHint =
    tourLength > 0
      ? `Walk the ${tourLength}-step codebase tour`
      : flowCount > 0
        ? `No codebase tour in this map; ${flowCount} call flows to follow`
        : "This map has no codebase tour and no call flows";

  return (
    <header className="topbar">
      <div className="topbar-identity" data-walkthrough="identity">
        <h1 className="project-name">{map.project.name}</h1>
        <span className="provenance-tally" data-testid="provenance-tally">
          <strong>{groupingLabels[grouping]}</strong>
          <span>·</span>
          <span>
            {map.nodes.length - enriched} structural
            {enriched > 0 ? `, ${enriched} enriched` : ""}
          </span>
        </span>
      </div>

      <div className="topbar-modes">
        <SegmentedControl
          name="View"
          walkthroughStep="view"
          value={mode}
          onChange={onMode}
          options={[
            { value: "overview", label: "Overview", hint: "The map itself" },
            {
              value: "learn",
              label: "Learn",
              hint: learnHint,
            },
          ]}
        />
        <SegmentedControl
          name="Grouping"
          walkthroughStep="grouping"
          value={grouping}
          onChange={onGrouping}
          options={[
            {
              value: "domain",
              label: groupingLabels.domain,
              hint: "Group by the call flows the map found",
            },
            {
              value: "structural",
              label: groupingLabels.structural,
              hint: "Group by directory-derived layers",
            },
          ]}
        />
      </div>

      <div className="topbar-actions">
        <ExportMenu
          map={map}
          shared={shared}
          open={exportOpen}
          onOpen={onExportOpen}
        />
        <button
          type="button"
          className={`topbar-button${pathOpen ? " topbar-button-on" : ""}`}
          aria-pressed={pathOpen}
          data-walkthrough="path"
          title="Find the shortest chain of relationships between two nodes"
          onClick={onTogglePath}
        >
          Path
        </button>
        {/* Both folds at once. It reads as pressed while anything is folded,
            so the control that got the reader here is the control that gets
            them back — whichever of the three they used. */}
        <button
          type="button"
          className={`topbar-button${allOpen(chrome) ? "" : " topbar-button-on"}`}
          aria-pressed={!allOpen(chrome)}
          data-walkthrough="focus"
          title={
            allOpen(chrome)
              ? "Fold the panel and the regions away, leaving the map"
              : "Bring the panel and the regions back"
          }
          onClick={() =>
            onChrome(
              allOpen(chrome)
                ? { panel: true, chips: true }
                : { panel: false, chips: false },
            )
          }
        >
          Focus
        </button>
        {/* Named for what it walks. The other walk in this product is the
            *codebase* tour behind the Learn switch, and two things called a
            tour would confuse the reader before either of them explained
            anything. */}
        <WalkthroughLauncher
          open={walkthroughOpen}
          onStart={onStartWalkthrough}
        />
        <ThemeToggle />
      </div>
    </header>
  );
}
