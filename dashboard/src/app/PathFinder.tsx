// The Path control: pick two nodes, see the shortest chain of relationships
// between them, and get every node on it highlighted on the canvas.
import { useMemo, useState } from "react";
import type { KnowledgeGraph, Node as MapNode } from "../index.js";
import { searchNodes } from "./graph.js";
import { shortestPath } from "./paths.js";

/** Matches offered while typing. Enough to choose from, few enough to scan. */
const SUGGESTIONS = 6;

export function PathFinder({
  map,
  from,
  to,
  onPick,
  onSelectNode,
}: {
  map: KnowledgeGraph;
  from: MapNode | null;
  to: MapNode | null;
  onPick: (end: "from" | "to", node: MapNode | null) => void;
  onSelectNode: (id: string) => void;
}) {
  const path = useMemo(
    () =>
      from === null || to === null ? null : shortestPath(map, from.id, to.id),
    [map, from, to],
  );

  return (
    <section className="pathfinder" aria-label="Path finder">
      <EndPicker
        map={map}
        label="From"
        picked={from}
        onPick={(node) => onPick("from", node)}
      />
      <EndPicker
        map={map}
        label="To"
        picked={to}
        onPick={(node) => onPick("to", node)}
      />
      {from !== null && to !== null && (
        <div className="path-result">
          {path === null ? (
            <p className="info-empty">
              No chain of relationships joins these two.
            </p>
          ) : (
            <ol className="path-steps" aria-label="Path steps">
              {path.map((node, i) => (
                <li key={node.id}>
                  <button type="button" onClick={() => onSelectNode(node.id)}>
                    <span className="path-index">{i + 1}</span>
                    <span className="path-name">{node.name}</span>
                    <span className="path-path">{node.path}</span>
                  </button>
                </li>
              ))}
            </ol>
          )}
        </div>
      )}
    </section>
  );
}

function EndPicker({
  map,
  label,
  picked,
  onPick,
}: {
  map: KnowledgeGraph;
  label: string;
  picked: MapNode | null;
  onPick: (node: MapNode | null) => void;
}) {
  const [query, setQuery] = useState("");
  const matches = useMemo(
    () => searchNodes(map, query).slice(0, SUGGESTIONS),
    [map, query],
  );

  if (picked !== null) {
    return (
      <div className="path-end path-end-picked">
        <span className="path-end-label">{label}</span>
        <span className="path-end-name">{picked.name}</span>
        <button
          type="button"
          className="path-clear"
          aria-label={`Clear ${label}`}
          onClick={() => {
            onPick(null);
            setQuery("");
          }}
        >
          ×
        </button>
      </div>
    );
  }

  return (
    <div className="path-end">
      <label className="path-end-label" htmlFor={`path-${label}`}>
        {label}
      </label>
      <input
        id={`path-${label}`}
        type="search"
        placeholder="name or path…"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
      />
      {query.trim() !== "" && (
        <ul className="path-suggestions" aria-label={`${label} suggestions`}>
          {matches.map((node) => (
            <li key={node.id}>
              <button
                type="button"
                onClick={() => {
                  onPick(node);
                  setQuery("");
                }}
              >
                <span className="result-name">{node.name}</span>
                <span className="result-path">{node.path}</span>
              </button>
            </li>
          ))}
          {matches.length === 0 && <li className="no-matches">No matches</li>}
        </ul>
      )}
    </div>
  );
}
