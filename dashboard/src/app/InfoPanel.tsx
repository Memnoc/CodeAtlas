// The right panel's INFO tab: what this project is, where to start reading,
// what everything leans on, and how its regions relate.
import { useMemo, type ReactNode } from "react";
import type { KnowledgeGraph } from "../index.js";
import {
  entryPoints,
  languageCounts,
  mostDependedOn,
  projectCounts,
} from "./insights.js";
import type { Region, RegionLink } from "./regions.js";

export function InfoPanel({
  map,
  regions,
  links,
  onSelectNode,
  onOpenRegion,
}: {
  map: KnowledgeGraph;
  regions: readonly Region[];
  links: readonly RegionLink[];
  onSelectNode: (id: string) => void;
  onOpenRegion: (id: string) => void;
}) {
  // Each of these walks every node or every edge, and the panel re-renders
  // on each keystroke in the search box above it — the query lives in the
  // explorer, so React has no way to know this subtree does not depend on it.
  const counts = useMemo(
    () => projectCounts(map, regions.length),
    [map, regions.length],
  );
  const languages = useMemo(() => languageCounts(map), [map]);
  const starts = useMemo(() => entryPoints(map), [map]);
  const leaned = useMemo(() => mostDependedOn(map), [map]);
  const widest = Math.max(1, ...regions.map((r) => r.files.length));
  const names = new Map(regions.map((r) => [r.id, r.name]));

  return (
    <div className="info-panel">
      <h2 className="info-project">{map.project.name}</h2>
      <p className="info-counts">
        {counts.files} {counts.files === 1 ? "file" : "files"} in{" "}
        {counts.regions} {counts.regions === 1 ? "region" : "regions"},{" "}
        {counts.relationships} relationships between them.
      </p>

      <ul className="language-chips" aria-label="Languages">
        {languages.map(({ language, count }) => (
          <li key={language} className="language-chip">
            {language} <span className="chip-count">{count}</span>
          </li>
        ))}
      </ul>

      <Section
        title="Start here"
        blurb="Calls other code; nothing calls it."
        empty="No entry point — every function here is called by another."
        rows={starts.map((node) => ({
          key: node.id,
          onClick: () => onSelectNode(node.id),
          left: <code className="row-symbol">{node.name}</code>,
          right: <span className="row-path">{node.path}</span>,
        }))}
      />

      <Section
        title="Everything leans on"
        blurb="The files the most other files reach into."
        empty="Nothing is imported more than once."
        rows={leaned.map(({ node, count }) => ({
          key: node.id,
          onClick: () => onSelectNode(node.id),
          left: <code className="row-symbol">{node.path}</code>,
          right: (
            <span className="row-path">
              ← {count} {count === 1 ? "file" : "files"}
            </span>
          ),
        }))}
      />

      <section className="info-section" aria-label="Regions">
        <h3>Regions</h3>
        <ul className="region-list">
          {regions.map((region, i) => (
            <li key={region.id}>
              <button
                type="button"
                className="region-row"
                data-accent={i % 6}
                onClick={() => onOpenRegion(region.id)}
              >
                <span className="region-row-head">
                  <span className="region-dot" aria-hidden="true" />
                  <span className="region-row-name">{region.name}</span>
                  <span className="region-row-count">
                    {region.files.length}{" "}
                    {region.files.length === 1 ? "file" : "files"}
                  </span>
                </span>
                {/* The bar is scaled by the same number the row prints, so
                    two readings of one region can never disagree. */}
                <span className="region-bar" aria-hidden="true">
                  <span
                    className="region-bar-fill"
                    style={{
                      width: `${(region.files.length / widest) * 100}%`,
                    }}
                  />
                </span>
                <span className="region-row-description">
                  {region.description}
                </span>
              </button>
            </li>
          ))}
        </ul>
      </section>

      <section className="info-section" aria-label="How they connect">
        <h3>How they connect</h3>
        {links.length === 0 ? (
          <p className="info-empty">
            No region reaches into another — every relationship stays at home.
          </p>
        ) : (
          <ul className="connect-list">
            {[...links]
              .sort((a, b) => b.count - a.count)
              .map((link) => (
                <li key={`${link.source}--${link.target}`}>
                  <span className="connect-from">
                    {names.get(link.source) ?? link.source}
                  </span>
                  <span className="connect-arrow" aria-hidden="true">
                    →
                  </span>
                  <span className="connect-to">
                    {names.get(link.target) ?? link.target}
                  </span>
                  <span className="connect-count">{link.label}</span>
                </li>
              ))}
          </ul>
        )}
      </section>
    </div>
  );
}

function Section({
  title,
  blurb,
  empty,
  rows,
}: {
  title: string;
  blurb: string;
  empty: string;
  rows: {
    key: string;
    onClick: () => void;
    left: ReactNode;
    right: ReactNode;
  }[];
}) {
  return (
    <section className="info-section" aria-label={title}>
      <h3>{title}</h3>
      <p className="info-blurb">{blurb}</p>
      {rows.length === 0 ? (
        <p className="info-empty">{empty}</p>
      ) : (
        <ul className="info-rows">
          {rows.map((row) => (
            <li key={row.key}>
              <button type="button" className="info-row" onClick={row.onClick}>
                {row.left}
                {row.right}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
