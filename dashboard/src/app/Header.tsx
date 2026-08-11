// The top bar: what you are looking at, how you want to look at it, and the
// two things you can do to it.
import type { KnowledgeGraph } from "../index.js";
import { ExportMenu } from "./ExportMenu.js";
import type { RegionKind } from "./regions.js";
import { SegmentedControl } from "./SegmentedControl.js";
import { ThemeToggle } from "./ThemeToggle.js";
import { WalkthroughLauncher } from "./Walkthrough.js";

/** Overview draws the map; Learn walks the guided tour through it. */
export type Mode = "overview" | "learn";

export function Header({
  map,
  mode,
  onMode,
  grouping,
  onGrouping,
  pathOpen,
  onTogglePath,
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
          <strong>{grouping}</strong>
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
          walkthrough="view"
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
          walkthrough="grouping"
          value={grouping}
          onChange={onGrouping}
          options={[
            {
              value: "domain",
              label: "Domain",
              hint: "Group by the call flows the map found",
            },
            {
              value: "structural",
              label: "Structural",
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
