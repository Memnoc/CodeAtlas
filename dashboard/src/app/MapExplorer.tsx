// The dashboard's seam component: give it a map conforming to the published
// contract and it renders the whole explorer — canvas, search, detail panel.
// It consumes only the generated contract types (ADR-0003) and never makes a
// network request.
import { ReactFlow } from "@xyflow/react";
import { useMemo, useState } from "react";
import type { KnowledgeGraph, Node as MapNode } from "../index.js";
import { searchNodes, toFlow } from "./graph.js";
import { nodeTypes } from "./nodes.js";
import "@xyflow/react/dist/style.css";
import "./styles.css";

export function MapExplorer({ map }: { map: KnowledgeGraph }) {
  const { nodes, edges } = useMemo(() => toFlow(map), [map]);
  const [query, setQuery] = useState("");
  const [selectedId, setSelectedId] = useState<string | null>(null);

  const results = useMemo(() => searchNodes(map, query), [map, query]);
  const selected = useMemo(
    () => map.nodes.find((n) => n.id === selectedId) ?? null,
    [map, selectedId],
  );

  return (
    <div className="explorer">
      <aside className="sidebar">
        <header className="sidebar-header">
          <h1>{map.project.name}</h1>
          <p className="contract-version">map contract {map.version}</p>
        </header>
        <input
          type="search"
          aria-label="Search nodes"
          placeholder="Search by name or path…"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        {query.trim() !== "" && (
          <ul className="search-results" aria-label="Search results">
            {results.map((n) => (
              <li key={n.id}>
                <button type="button" onClick={() => setSelectedId(n.id)}>
                  <span className="result-name">{n.name}</span>
                  <span className="result-path">{n.path}</span>
                </button>
              </li>
            ))}
            {results.length === 0 && <li className="no-matches">No matches</li>}
          </ul>
        )}
        {selected && (
          <DetailPanel map={map} node={selected} onSelect={setSelectedId} />
        )}
      </aside>
      <main className="canvas">
        <ReactFlow
          nodes={nodes}
          edges={edges}
          nodeTypes={nodeTypes}
          onNodeClick={(_event, node) => {
            if (node.type === "entity") {
              setSelectedId(node.id);
            }
          }}
          fitView
          minZoom={0.05}
          nodesConnectable={false}
        />
      </main>
    </div>
  );
}

function DetailPanel({
  map,
  node,
  onSelect,
}: {
  map: KnowledgeGraph;
  node: MapNode;
  onSelect: (id: string) => void;
}) {
  const byId = useMemo(
    () => new Map(map.nodes.map((n) => [n.id, n])),
    [map],
  );
  const touching = map.edges.filter(
    (e) => e.source === node.id || e.target === node.id,
  );

  return (
    <section className="detail" aria-label="Node detail">
      <h2>{node.name}</h2>
      <p className="detail-meta">
        <span className="detail-kind">{node.kind}</span>
        <span
          className={`badge badge-${node.provenance}`}
          data-testid="provenance-badge"
        >
          {node.provenance}
        </span>
      </p>
      <p className="detail-path">{node.path}</p>
      {node.range && (
        <p className="detail-range">
          lines {node.range.start_line}–{node.range.end_line}
        </p>
      )}
      <p className="detail-summary">{node.summary}</p>
      <h3>Edges</h3>
      {touching.length === 0 ? (
        <p className="no-edges">No edges</p>
      ) : (
        <ul className="edge-list" aria-label="Edges">
          {touching.map((e) => {
            const outgoing = e.source === node.id;
            const otherId = outgoing ? e.target : e.source;
            const otherName = byId.get(otherId)?.name ?? otherId;
            return (
              <li key={`${e.source}--${e.kind}--${e.target}`}>
                <button type="button" onClick={() => onSelect(otherId)}>
                  {outgoing
                    ? `${e.kind} → ${otherName}`
                    : `← ${e.kind} ${otherName}`}
                </button>
              </li>
            );
          })}
        </ul>
      )}
    </section>
  );
}
