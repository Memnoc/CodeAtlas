// The top bar: what you are looking at, how you want to look at it, and the
// two things you can do to it.
import type { KnowledgeGraph } from "../index.js";
import { downloadMap } from "./export.js";
import type { RegionKind } from "./regions.js";
import { SegmentedControl } from "./SegmentedControl.js";
import { ThemeToggle } from "./ThemeToggle.js";

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
}: {
  map: KnowledgeGraph;
  mode: Mode;
  onMode: (mode: Mode) => void;
  grouping: RegionKind;
  onGrouping: (grouping: RegionKind) => void;
  pathOpen: boolean;
  onTogglePath: () => void;
}) {
  const enriched = map.nodes.filter((n) => n.provenance === "llm").length;
  // Learn holds the tour *and* the flows, so the hint has to speak for both:
  // saying "no tour" over a mode with 148 call flows in it would be a lie.
  const tourLength = map.tour?.length ?? 0;
  const flowCount = map.domain_flows?.length ?? 0;
  const learnHint =
    tourLength > 0
      ? `Walk the ${tourLength}-step guided tour`
      : flowCount > 0
        ? `No tour in this map; ${flowCount} call flows to follow`
        : "This map has no tour and no call flows";

  return (
    <header className="topbar">
      <div className="topbar-identity">
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
        <button
          type="button"
          className="topbar-button"
          title="Download this map as JSON. For a self-contained shareable page, run `codeatlas share`."
          onClick={() => downloadMap(map)}
        >
          Export
        </button>
        <button
          type="button"
          className={`topbar-button${pathOpen ? " topbar-button-on" : ""}`}
          aria-pressed={pathOpen}
          title="Find the shortest chain of relationships between two nodes"
          onClick={onTogglePath}
        >
          Path
        </button>
        <ThemeToggle />
      </div>
    </header>
  );
}
